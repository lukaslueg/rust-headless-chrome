use serde::Deserialize;

pub type ScriptId = String;
pub type ExecutionContextId = u64;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPreview {
    pub name: String,
    #[serde(rename = "type")]
    pub object_type: String,
    pub value: Option<String>,
    pub value_preview: Option<Box<PropertyPreview>>,
    pub subtype: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPreview {
    #[serde(rename = "type")]
    pub object_type: String,
    pub subtype: Option<String>,
    pub description: Option<String>,
    pub overflow: bool,
    pub properties: Vec<PropertyPreview>,
}

pub enum RemoteObject4 {
    Object {
        subtype: methods::RemoteObjectSubKind,
        class_name: String,
    },
    Function,
    Undefined,
    Str,
    Number,
    Boolean,
    Symbol,
    BigInt,
}

pub struct RemoteObject3 {
    pub kind: RemoteObject4,
    pub value: Option<serde_json::Value>,
    pub unserializable_value: Option<String>,
    pub preview: Option<ObjectPreview>,
    pub description: Option<String>,
}

impl RemoteObject3 {
    pub fn into_value<T: serde::de::DeserializeOwned>(
        self,
    ) -> Result<Option<T>, serde_json::error::Error> {
        //self.value.map(|v| serde_json::from_value(v)).or(Ok(None))
        match self.value {
            Some(v) => Ok(Some(serde_json::from_value(v)?)),
            None => Ok(None),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RemoteObject {
    #[serde(rename = "type")]
    pub object_type: String,
    pub subtype: Option<String>,
    pub description: Option<String>,
    pub class_name: Option<String>,
    pub value: Option<serde_json::Value>,
    pub unserializable_value: Option<String>,
    pub preview: Option<ObjectPreview>,
}

/// Stack entry for runtime errors and assertions.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CallFrame {
    /// JavaScript function name
    pub function_name: String,
    /// JavaScript script id
    pub script_id: ScriptId,
    /// JavaScript script name or url
    pub url: String,
    /// JavaScript script line number (0-based)
    pub line_number: u64,
    /// JavaScript script column number (0-based)
    pub column_number: u64,
}

/// Call frames for assertions or error messages.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StackTrace {
    /// String label of this stack trace. For async traces this may be a name
    /// of the function that initiated the async call
    description: Option<String>,
    /// JavaScript function name
    call_frames: Vec<CallFrame>,
    /// Asynchronous JavaScript stack trace that preceded this stack, if
    /// available
    parent: Option<Box<StackTrace>>,
}

/// Detailed information about exception (or error) that was thrown during
/// script compilation or execution.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionDetails {
    /// Exception id
    pub exception_id: u64,
    /// Exception text, which should be used together with exception object when
    /// available
    pub text: String,
    /// Line number of the exception location (0-based)
    pub line_number: u64,
    /// Column number of the exception location (0-based)
    pub column_number: u64,
    /// Script ID of the exception location
    pub script_id: Option<ScriptId>,
    /// URL of the exception location, to be used when the script was not reported
    pub url: Option<String>,
    /// JavaScript stack trace if available
    pub stack_trace: Option<StackTrace>,
    /// Exception object if available
    pub exception: Option<RemoteObject>,
    /// Identifier of the context where exception happened
    pub execution: Option<ExecutionContextId>,
}

impl std::fmt::Display for ExceptionDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        f.pad(&self.text)
    }
}

impl std::error::Error for ExceptionDetails {}

pub mod methods {
    use crate::cdtp::Method;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RuntimeResult<T> {
        pub result: T,
        pub exception_details: Option<super::ExceptionDetails>,
    }

    impl<T> RuntimeResult<T> {
        pub fn into_result(self) -> Result<T, super::ExceptionDetails> {
            match self.exception_details {
                Some(e) => Err(e),
                None => Ok(self.result),
            }
        }
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "lowercase")]
    pub enum RemoteObjectSubKind {
        Array,
        Null,
        Node,
        Regexp,
        Date,
        Map,
        Set,
        Weakmap,
        Weakset,
        #[serde(rename = "iterator")]
        Iter,
        Generator,
        Error,
        Proxy,
        Promise,
        TypedArray,
        ArrayBuffer,
        Dataview,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "lowercase")]
    pub enum RemoteObjectKind {
        Object,
        Function,
        Undefined,
        #[serde(rename = "string")]
        Str,
        Number,
        Boolean,
        Symbol,
        BigInt,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct RemoteObject2 {
        #[serde(rename = "type")]
        pub object_type: RemoteObjectKind,
        pub subtype: Option<RemoteObjectSubKind>,
        pub description: Option<String>,
        pub class_name: Option<String>,
        pub value: Option<serde_json::Value>,
        pub unserializable_value: Option<String>,
        pub preview: Option<super::ObjectPreview>,
    }

    impl Into<super::RemoteObject3> for RemoteObject2 {
        fn into(self) -> super::RemoteObject3 {
            match self.object_type {
                RemoteObjectKind::Object => super::RemoteObject3 {
                    kind: super::RemoteObject4::Object {
                        subtype: self.subtype.unwrap(),
                        class_name: self.class_name.unwrap(),
                    },
                    value: self.value,
                    unserializable_value: self.unserializable_value,
                    preview: self.preview,
                    description: self.description,
                },
                RemoteObjectKind::Function => super::RemoteObject3 {
                    kind: super::RemoteObject4::Function,
                    value: self.value,
                    unserializable_value: self.unserializable_value,
                    preview: self.preview,
                    description: self.description,
                },
                RemoteObjectKind::Undefined => super::RemoteObject3 {
                    kind: super::RemoteObject4::Undefined,
                    value: self.value,
                    unserializable_value: self.unserializable_value,
                    preview: self.preview,
                    description: self.description,
                },
                RemoteObjectKind::Str => super::RemoteObject3 {
                    kind: super::RemoteObject4::Str,
                    value: self.value,
                    unserializable_value: self.unserializable_value,
                    preview: self.preview,
                    description: self.description,
                },
                RemoteObjectKind::Number => super::RemoteObject3 {
                    kind: super::RemoteObject4::Number,
                    value: self.value,
                    unserializable_value: self.unserializable_value,
                    preview: self.preview,
                    description: self.description,
                },
                RemoteObjectKind::Boolean => super::RemoteObject3 {
                    kind: super::RemoteObject4::Boolean,
                    value: self.value,
                    unserializable_value: self.unserializable_value,
                    preview: self.preview,
                    description: self.description,
                },
                RemoteObjectKind::Symbol => super::RemoteObject3 {
                    kind: super::RemoteObject4::Symbol,
                    value: self.value,
                    unserializable_value: self.unserializable_value,
                    preview: self.preview,
                    description: self.description,
                },
                RemoteObjectKind::BigInt => super::RemoteObject3 {
                    kind: super::RemoteObject4::BigInt,
                    value: self.value,
                    unserializable_value: self.unserializable_value,
                    preview: self.preview,
                    description: self.description,
                },
            }
        }
    }

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct CallFunctionOn<'a> {
        pub object_id: &'a str,
        pub function_declaration: &'a str,
        pub return_by_value: bool,
        pub generate_preview: bool,
        pub silent: bool,
    }
    impl<'a> Method for CallFunctionOn<'a> {
        const NAME: &'static str = "Runtime.callFunctionOn";
        type ReturnObject = RuntimeResult<super::RemoteObject>;
    }

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Evaluate<'a> {
        pub expression: &'a str,
        pub return_by_value: bool,
        pub silent: bool,
    }
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct EvaluateReturnObject {
        pub result: RemoteObject2,
        pub exception_details: Option<super::ExceptionDetails>,
    }
    impl<'a> Method for Evaluate<'a> {
        const NAME: &'static str = "Runtime.evaluate";
        type ReturnObject = EvaluateReturnObject;
    }

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct CompileScript<'a> {
        pub expression: &'a str,
        #[serde(rename = "sourceURL")]
        pub source_url: &'a str,
        pub persist_script: bool,
    }
    impl<'a> Method for CompileScript<'a> {
        const NAME: &'static str = "Runtime.compileScript";
        type ReturnObject = RuntimeResult<super::ScriptId>;
    }
}
