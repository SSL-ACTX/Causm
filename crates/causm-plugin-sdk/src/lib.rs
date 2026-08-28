pub use causm_core as core;
pub use causm_plugin_sdk_macros::causm_plugin;
pub use causm_plugins::{
    DiagnosticLevel, PluginDiagnostic, PluginRequest, PluginResponse, PluginStatus,
};

pub mod abi {
    use crate::{PluginContext, PluginError};
    use causm_core::Program;
    use causm_plugins::{PluginRequest, PluginResponse};

    pub fn alloc(len: u32) -> *mut u8 {
        let mut buf = Vec::with_capacity(len as usize);
        let ptr = buf.as_mut_ptr();
        std::mem::forget(buf);
        ptr
    }

    /// # Safety
    /// Caller must ensure `ptr` points to a valid buffer allocated with `alloc` of length `len`.
    pub unsafe fn dealloc(ptr: *mut u8, len: u32) {
        if !ptr.is_null() && len > 0 {
            let _ = Vec::from_raw_parts(ptr, 0, len as usize);
        }
    }

    /// # Safety
    /// Caller must ensure `ptr` points to a valid readable byte slice of length `len`.
    pub unsafe fn dispatch<F>(ptr: *mut u8, len: u32, transform_fn: F) -> u64
    where
        F: FnOnce(Program, &PluginContext) -> Result<Program, PluginError>,
    {
        let in_slice = std::slice::from_raw_parts(ptr, len as usize);
        let req: Result<PluginRequest, _> = bincode::deserialize(in_slice);

        let response = match req {
            Ok(request) => {
                let ctx = PluginContext {
                    file_path: request.file_path,
                    options: request.options,
                };
                match transform_fn(request.ast, &ctx) {
                    Ok(modified_ast) => {
                        PluginResponse::success(Some(modified_ast), vec![])
                    }
                    Err(plugin_err) => match plugin_err {
                        PluginError::Diagnostics(diags) => {
                            PluginResponse::error("Diagnostics encountered", diags)
                        }
                        PluginError::Message(msg) => {
                            PluginResponse::error(msg, vec![])
                        }
                    },
                }
            }
            Err(e) => PluginResponse::error(
                format!("Failed to deserialize request: {}", e),
                vec![],
            ),
        };

        let out_bytes = bincode::serialize(&response).unwrap_or_default();
        let out_len = out_bytes.len() as u32;
        let out_ptr = alloc(out_len);
        unsafe {
            std::ptr::copy_nonoverlapping(
                out_bytes.as_ptr(),
                out_ptr,
                out_len as usize,
            );
        }

        ((out_ptr as u64) << 32) | (out_len as u64)
    }
}

pub struct PluginContext {
    pub file_path: String,
    pub options: std::collections::HashMap<String, String>,
}

impl PluginContext {
    pub fn get_option_string(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn get_option_bool(&self, key: &str) -> Option<bool> {
        self.options.get(key).and_then(|v| v.parse::<bool>().ok())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin diagnostics: {0:?}")]
    Diagnostics(Vec<PluginDiagnostic>),
    #[error("{0}")]
    Message(String),
}

impl PluginError {
    pub fn diagnostic(
        level: DiagnosticLevel,
        message: impl Into<String>,
        span: Option<causm_core::Span>,
    ) -> Self {
        PluginError::Diagnostics(vec![PluginDiagnostic {
            level,
            message: message.into(),
            span,
        }])
    }
}

pub mod prelude {
    pub use super::core::*;
    pub use super::{
        causm_plugin, DiagnosticLevel, PluginContext, PluginDiagnostic, PluginError,
        PluginRequest, PluginResponse, PluginStatus,
    };
}
