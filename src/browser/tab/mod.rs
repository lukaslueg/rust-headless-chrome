use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use failure::{Error, Fail};
use log::*;
use serde;

use element::Element;
use point::Point;

use crate::browser::Transport;
use crate::protocol::page::methods::Navigate;
use crate::protocol::target::TargetId;
use crate::protocol::target::TargetInfo;
use crate::protocol::Event;
use crate::protocol::{dom, input, page, profiler, target};
use crate::{protocol, util};

use super::transport::SessionId;
use crate::protocol::dom::Node;
use std::time::Duration;

pub mod element;
mod keys;
mod point;

/// A handle to a single page. Exposes methods for simulating user actions (clicking,
/// typing), and also for getting information about the DOM and other parts of the page.
#[derive(Debug)]
pub struct Tab {
    target_id: TargetId,
    transport: Arc<Transport>,
    session_id: SessionId,
    navigating: Arc<AtomicBool>,
    target_info: Arc<Mutex<TargetInfo>>,
}

#[derive(Debug, Fail)]
#[fail(display = "No element found for selector: {}", selector)]
pub struct NoElementFound {
    selector: String,
}

#[derive(Debug, Fail)]
#[fail(display = "Navigate failed: {}", error_text)]
pub struct NavigationFailed {
    error_text: String,
}

impl Tab {
    pub fn new(target_info: TargetInfo, transport: Arc<Transport>) -> Result<Self, Error> {
        let target_id = target_info.target_id.clone();

        let session_id = transport
            .call_method_on_browser(target::methods::AttachToTarget {
                target_id: &target_id,
                flatten: None,
            })?
            .session_id
            .into();

        debug!("New tab attached with session ID: {:?}", session_id);

        let target_info_mutex = Arc::new(Mutex::new(target_info));

        let tab = Self {
            target_id,
            transport,
            session_id,
            navigating: Arc::new(AtomicBool::new(false)),
            target_info: target_info_mutex,
        };

        tab.call_method(page::methods::Enable {})?;
        tab.call_method(page::methods::SetLifecycleEventsEnabled { enabled: true })?;

        tab.start_event_handler_thread();

        Ok(tab)
    }

    pub fn update_target_info(&self, target_info: TargetInfo) {
        let mut info = self.target_info.lock().unwrap();
        *info = target_info;
    }

    pub fn get_target_id(&self) -> &TargetId {
        &self.target_id
    }

    pub fn get_url(&self) -> String {
        let info = self.target_info.lock().unwrap();
        info.url.clone()
    }

    fn start_event_handler_thread(&self) {
        let incoming_events_rx = self
            .transport
            .listen_to_target_events(self.session_id.clone());
        let navigating = Arc::clone(&self.navigating);

        std::thread::spawn(move || {
            for event in incoming_events_rx {
                trace!("{:?}", &event);
                if let Event::Lifecycle(lifecycle_event) = event {
                    //                        if lifecycle_event.params.frame_id == main_frame_id {
                    match lifecycle_event.params.name.as_ref() {
                        "networkAlmostIdle" => {
                            navigating.store(false, Ordering::SeqCst);
                        }
                        "init" => {
                            navigating.store(true, Ordering::SeqCst);
                        }
                        _ => {}
                    }
                }
            }
        });
    }

    pub fn call_method<C>(&self, method: C) -> Result<C::ReturnObject, Error>
    where
        C: protocol::Method + serde::Serialize + std::fmt::Debug,
    {
        debug!("Calling method: {:?}", method);
        let result = self
            .transport
            .call_method_on_target(self.session_id.clone(), method);
        let mut result_string = format!("{:?}", result);
        result_string.truncate(70);
        debug!("Got result: {:?}", result_string);
        result
    }

    pub fn wait_until_navigated(&self) -> Result<&Self, Error> {
        trace!("waiting to start navigating");
        // wait for navigating to go to true
        let navigating = Arc::clone(&self.navigating);
        util::Wait::default().until(|| {
            if navigating.load(Ordering::SeqCst) {
                Some(true)
            } else {
                None
            }
        })?;
        debug!("A tab started navigating");

        util::Wait::default().until(|| {
            if navigating.load(Ordering::SeqCst) {
                None
            } else {
                Some(true)
            }
        })?;
        debug!("A tab finished navigating");

        Ok(self)
    }

    pub fn navigate_to(&self, url: &str) -> Result<&Self, Error> {
        let return_object = self.call_method(Navigate { url })?;
        if let Some(error_text) = return_object.error_text {
            return Err(NavigationFailed { error_text }.into());
        }

        info!("Navigating a tab to {}", url);

        Ok(self)
    }

    pub fn wait_for_element<'a>(&'a self, selector: &'a str) -> Result<Element<'a>, Error> {
        self.wait_for_element_with_custom_timeout(selector, std::time::Duration::from_secs(15))
    }

    pub fn wait_for_element_with_custom_timeout<'a>(
        &'a self,
        selector: &'a str,
        timeout: std::time::Duration,
    ) -> Result<Element<'a>, Error> {
        debug!("Waiting for element with selector: {}", selector);
        util::Wait::with_timeout(timeout)
            .until(|| {
                if let Ok(element) = self.find_element(selector) {
                    Some(element)
                } else {
                    None
                }
            })
            .map_err(|e| e.into())
    }

    pub fn wait_for_elements<'a>(&'a self, selector: &'a str) -> Result<Vec<Element<'a>>, Error> {
        debug!("Waiting for element with selector: {}", selector);
        util::Wait::with_timeout(Duration::from_secs(15))
            .until(|| {
                if let Ok(elements) = self.find_elements(selector) {
                    Some(elements)
                } else {
                    None
                }
            })
            .map_err(|e| e.into())
    }

    pub fn find_element<'a>(&'a self, selector: &'a str) -> Result<Element<'a>, Error> {
        trace!("Looking up element via selector: {}", selector);

        let node_id = {
            let root_node_id = self.get_document()?.node_id;

            self.call_method(dom::methods::QuerySelector {
                node_id: root_node_id,
                selector,
            })?
            .node_id
        };

        Element::new(&self, node_id, selector)
    }

    pub fn get_document(&self) -> Result<Node, Error> {
        Ok(self
            .call_method(dom::methods::GetDocument {
                depth: Some(0),
                pierce: Some(false),
            })?
            .root)
    }

    pub fn find_elements<'a>(&'a self, selector: &'a str) -> Result<Vec<Element<'a>>, Error> {
        trace!("Looking up elements via selector: {}", selector);

        let node_ids = {
            let root_node_id = self.get_document()?.node_id;

            self.call_method(dom::methods::QuerySelectorAll {
                node_id: root_node_id,
                selector,
            })?
            .node_ids
        };

        if node_ids.is_empty() {
            return Err(NoElementFound {
                selector: selector.to_string(),
            }
            .into());
        }

        let mut elements = vec![];

        for node_id in &node_ids {
            elements.push(Element::new(&self, *node_id, selector)?)
        }

        Ok(elements)
    }

    pub fn describe_node(&self, node_id: dom::NodeId) -> Result<dom::Node, Error> {
        let node = self
            .call_method(dom::methods::DescribeNode {
                node_id: Some(node_id),
                backend_node_id: None,
                depth: Some(100),
            })?
            .node;
        Ok(node)
    }

    pub fn type_str(&self, string_to_type: &str) -> Result<&Self, Error> {
        for c in string_to_type.split("") {
            // split call above will have empty string at start and end which we won't type
            if c == "" {
                continue;
            }
            self.press_key(c)?;
        }
        Ok(self)
    }

    pub fn press_key(&self, key: &str) -> Result<&Self, Error> {
        let definition = keys::get_key_definition(key)?;

        // See https://github.com/GoogleChrome/puppeteer/blob/62da2366c65b335751896afbb0206f23c61436f1/lib/Input.js#L114-L115
        let text = definition.text.or_else(|| {
            if definition.key.len() == 1 {
                Some(definition.key)
            } else {
                None
            }
        });

        // See https://github.com/GoogleChrome/puppeteer/blob/62da2366c65b335751896afbb0206f23c61436f1/lib/Input.js#L52
        let key_down_event_type = if text.is_some() {
            "keyDown"
        } else {
            "rawKeyDown"
        };

        let key = Some(definition.key);
        let code = Some(definition.code);

        self.call_method(input::methods::DispatchKeyEvent {
            event_type: key_down_event_type,
            key,
            text,
            code: Some(definition.code),
            windows_virtual_key_code: definition.key_code,
            native_virtual_key_code: definition.key_code,
        })?;
        self.call_method(input::methods::DispatchKeyEvent {
            event_type: "keyUp",
            key,
            text,
            code,
            windows_virtual_key_code: definition.key_code,
            native_virtual_key_code: definition.key_code,
        })?;
        Ok(self)
    }

    pub fn click_point(&self, point: Point) -> Result<&Self, Error> {
        trace!("Clicking point: {:?}", point);
        if point.x == 0.0 && point.y == 0.0 {
            warn!("Midpoint of element shouldn't be 0,0. Something is probably wrong.")
        }

        self.call_method(input::methods::DispatchMouseEvent {
            event_type: "mouseMoved",
            x: point.x,
            y: point.y,
            ..Default::default()
        })?;
        self.call_method(input::methods::DispatchMouseEvent {
            event_type: "mousePressed",
            x: point.x,
            y: point.y,
            button: Some("left"),
            click_count: Some(1),
        })?;
        self.call_method(input::methods::DispatchMouseEvent {
            event_type: "mouseReleased",
            x: point.x,
            y: point.y,
            button: Some("left"),
            click_count: Some(1),
        })?;
        Ok(self)
    }

    /// Capture a screenshot of the current page.
    ///
    /// If `clip` is given, the screenshot is taken of the specified region only.
    /// `Element::get_box_model()` can be used to get regions of certains elements
    /// on the page; there is also `Element::capture_screenhot()` as a shorthand.
    ///
    /// If `from_surface` is true, the screenshot is taken from the surface rather than
    /// the view.
    ///
    /// ```rust,no_run
    /// # use failure::Error;
    /// # fn main() -> Result<(), Error> {
    /// #
    /// use headless_chrome::{protocol::page::ScreenshotFormat, Browser, LaunchOptionsBuilder};
    /// let browser = Browser::new(LaunchOptionsBuilder::default().build().unwrap())?;
    /// let tab = browser.wait_for_initial_tab()?;
    /// let viewport = tab.navigate_to("https://en.wikipedia.org/wiki/WebKit")?
    ///     .wait_for_element("#mw-content-text > div > table.infobox.vevent")?
    ///     .get_box_model()?
    ///     .margin_viewport();
    ///  let png_data = tab.capture_screenshot(ScreenshotFormat::PNG, Some(viewport), true)?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn capture_screenshot(
        &self,
        format: page::ScreenshotFormat,
        clip: Option<page::Viewport>,
        from_surface: bool,
    ) -> Result<Vec<u8>, Error> {
        let (format, quality) = match format {
            page::ScreenshotFormat::JPEG(quality) => {
                (page::InternalScreenshotFormat::JPEG, quality)
            }
            page::ScreenshotFormat::PNG => (page::InternalScreenshotFormat::PNG, None),
        };
        let data = self
            .call_method(page::methods::CaptureScreenshot {
                format,
                clip,
                quality,
                from_surface,
            })?
            .data;
        base64::decode(&data).map_err(Into::into)
    }

    /// Reloads given page optionally ignoring the cache
    ///
    /// If `ignore_cache` is true, the browser cache is ignored (as if the user pressed Shift+F5).
    /// If `script_to_evaluate` is given, the script will be injected into all frames of the
    /// inspected page after reload. Argument will be ignored if reloading dataURL origin.
    pub fn reload(
        &self,
        ignore_cache: bool,
        script_to_evaluate: Option<&str>,
    ) -> Result<&Self, Error> {
        self.call_method(page::methods::Reload {
            ignore_cache,
            script_to_evaluate,
        })?;
        Ok(self)
    }

    /// Enables the profiler
    pub fn enable_profiler(&self) -> Result<&Self, Error> {
        self.call_method(profiler::methods::Enable {})?;

        Ok(self)
    }

    /// Disables the profiler
    pub fn disable_profiler(&self) -> Result<&Self, Error> {
        self.call_method(profiler::methods::Disable {})?;

        Ok(self)
    }

    /// Starts tracking which lines of JS have been executed
    ///
    /// Will return error unless `enable_profiler` has been called.
    ///
    /// Equivalent to hitting the record button in the "coverage" tab in Chrome DevTools.
    /// See the file `tests/coverage.rs` for an example.
    ///
    /// By default we enable the 'detailed' flag on StartPreciseCoverage, which enables block-level
    /// granularity, and also enable 'call_count' (which when disabled always sets count to 1 or 0).
    ///
    pub fn start_js_coverage(&self) -> Result<&Self, Error> {
        self.call_method(profiler::methods::StartPreciseCoverage {
            call_count: Some(true),
            detailed: Some(true),
        })?;
        Ok(self)
    }

    /// Stops tracking which lines of JS have been executed
    /// If you're finished with the profiler, don't forget to call `disable_profiler`.
    pub fn stop_js_coverage(&self) -> Result<&Self, Error> {
        self.call_method(profiler::methods::StopPreciseCoverage {})?;
        Ok(self)
    }

    /// Collect coverage data for the current isolate, and resets execution counters.
    ///
    /// Precise code coverage needs to have started (see `start_js_coverage`).
    ///
    /// Will only send information about code that's been executed since this method was last
    /// called, or (if this is the first time) since calling `start_js_coverage`.
    /// Another way of thinking about it is: every time you call this, the call counts for
    /// FunctionRanges are reset after returning.
    ///
    /// The format of the data is a little unintuitive, see here for details:
    /// https://chromedevtools.github.io/devtools-protocol/tot/Profiler#type-ScriptCoverage
    pub fn take_precise_js_coverage(&self) -> Result<Vec<profiler::ScriptCoverage>, Error> {
        let script_coverages = self
            .call_method(profiler::methods::TakePreciseCoverage {})?
            .result;
        Ok(script_coverages)
    }
}
