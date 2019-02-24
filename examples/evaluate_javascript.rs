use headless_chrome::{Browser, LaunchOptionsBuilder};

fn main() -> Result<(), failure::Error> {
    let options = LaunchOptionsBuilder::default()
        .build()
        .expect("Couldn't find appropriate Chrome binary.");
    let browser = Browser::new(options)?;
    let tab = browser.wait_for_initial_tab()?;
    let r: Option<String> = tab.evaluate_value("navigator.userAgent")?;
    //let r = tab.compile_script("1+1", "http://www.wikipedia.org", false)?;
    dbg!(&r);
    Ok(())
}
