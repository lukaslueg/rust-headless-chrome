use serde::Deserialize;

// TODO: use these aliases in other parts of the protocol module
// From experimentation, it seems the protocol's integers are i32s.
#[allow(dead_code)]
type JsInt = i32;
// For when we specifically want to guard against negative numbers.
type JsUInt = u32;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Coverage data for a source range.
pub struct CoverageRange {
    /// JavaScript script source offset for the range start.
    pub start_offset: JsUInt,
    /// JavaScript script source offset for the range end.
    pub end_offset: JsUInt,
    /// Collected execution count of the source range.
    pub count: JsUInt,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Coverage data for a JavaScript function.
pub struct FunctionCoverage {
    pub function_name: String,
    /// Source ranges inside the function with coverage data.
    pub ranges: Vec<CoverageRange>,
}

/// JS line coverage information for a single script
/// See https://chromedevtools.github.io/devtools-protocol/tot/Profiler#type-ScriptCoverage
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCoverage {
    pub script_id: String,
    /// Either the name or URL of a script loaded by the page
    pub url: String,
    /// Functions contained in the script that has coverage data
    pub functions: Vec<FunctionCoverage>,
}

pub mod methods {
    use crate::protocol::Method;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Enable {}
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct EnableReturnObject {}
    impl Method for Enable {
        const NAME: &'static str = "Profiler.enable";
        type ReturnObject = EnableReturnObject;
    }

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Disable {}
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct DisableReturnObject {}
    impl Method for Disable {
        const NAME: &'static str = "Profiler.disable";
        type ReturnObject = DisableReturnObject;
    }

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct StartPreciseCoverage {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub call_count: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub detailed: Option<bool>,
    }
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StartPreciseCoverageReturnObject {}
    impl Method for StartPreciseCoverage {
        const NAME: &'static str = "Profiler.startPreciseCoverage";
        type ReturnObject = StartPreciseCoverageReturnObject;
    }

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct StopPreciseCoverage {}
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct StopPreciseCoverageReturnObject {}
    impl Method for StopPreciseCoverage {
        const NAME: &'static str = "Profiler.stopPreciseCoverage";
        type ReturnObject = StopPreciseCoverageReturnObject;
    }

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct TakePreciseCoverage {}
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TakePreciseCoverageReturnObject {
        pub result: Vec<super::ScriptCoverage>,
    }
    impl Method for TakePreciseCoverage {
        const NAME: &'static str = "Profiler.takePreciseCoverage";
        type ReturnObject = TakePreciseCoverageReturnObject;
    }
}
