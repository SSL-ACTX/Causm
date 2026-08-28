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
                    protocol_version: request.protocol_version,
                    compiler_version: request.compiler_version,
                    target_arch: request.target_arch,
                    target_os: request.target_os,
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
    pub protocol_version: String,
    pub compiler_version: String,
    pub target_arch: String,
    pub target_os: String,
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

    pub fn is_target_linux(&self) -> bool {
        self.target_os == "linux"
    }

    pub fn is_target_arch(&self, arch: &str) -> bool {
        self.target_arch == arch
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
    pub fn error(
        message: impl Into<String>,
        span: Option<causm_core::Span>,
    ) -> Self {
        PluginError::Diagnostics(vec![PluginDiagnostic {
            level: DiagnosticLevel::Error,
            message: message.into(),
            span,
        }])
    }

    pub fn warning(
        message: impl Into<String>,
        span: Option<causm_core::Span>,
    ) -> Self {
        PluginError::Diagnostics(vec![PluginDiagnostic {
            level: DiagnosticLevel::Warning,
            message: message.into(),
            span,
        }])
    }

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

/// Fluent builder for constructing `PluginDiagnostic` instances.
pub struct DiagnosticBuilder {
    diag: PluginDiagnostic,
}

impl DiagnosticBuilder {
    pub fn new(level: DiagnosticLevel, message: impl Into<String>) -> Self {
        Self {
            diag: PluginDiagnostic {
                level,
                message: message.into(),
                span: None,
            },
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(DiagnosticLevel::Error, message)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(DiagnosticLevel::Warning, message)
    }

    pub fn note(message: impl Into<String>) -> Self {
        Self::new(DiagnosticLevel::Note, message)
    }

    pub fn with_span(mut self, span: &causm_core::Span) -> Self {
        self.diag.span = Some(span.clone());
        self
    }

    pub fn build(self) -> PluginDiagnostic {
        self.diag
    }
}

/// Read-only AST Visitor trait with default traversal logic.
pub trait AstVisitor {
    fn visit_program(&mut self, program: &causm_core::Program, ctx: &PluginContext) {
        for tb in &program.timelines {
            self.visit_timeline(tb, ctx);
        }
    }

    fn visit_timeline(
        &mut self,
        tb: &causm_core::TimelineBlock,
        ctx: &PluginContext,
    ) {
        for stmt in &tb.statements {
            self.visit_spanned_statement(stmt, ctx);
        }
    }

    fn visit_spanned_statement(
        &mut self,
        stmt: &causm_core::SpannedStatement,
        ctx: &PluginContext,
    ) {
        self.visit_statement(&stmt.stmt, &stmt.span, &stmt.attributes, ctx);
    }

    fn visit_statement(
        &mut self,
        stmt: &causm_core::Statement,
        span: &causm_core::Span,
        attrs: &[causm_core::Attribute],
        ctx: &PluginContext,
    ) {
        match stmt {
            causm_core::Statement::Isolate(isolate) => {
                self.visit_isolate(isolate, span, attrs, ctx);
            }
            causm_core::Statement::RoutineDef {
                name,
                params,
                return_type,
                taking_ms,
                state_constraint,
                required_capabilities,
                body,
            } => {
                self.visit_routine(
                    name,
                    params,
                    return_type.as_ref(),
                    *taking_ms,
                    state_constraint.as_ref(),
                    required_capabilities,
                    body,
                    span,
                    attrs,
                    ctx,
                );
            }
            causm_core::Statement::Expression(expr) => {
                self.visit_expression(expr, span, ctx);
            }
            _ => {}
        }
    }

    fn visit_isolate(
        &mut self,
        isolate: &causm_core::IsolateBlock,
        _span: &causm_core::Span,
        _attrs: &[causm_core::Attribute],
        ctx: &PluginContext,
    ) {
        for inner in &isolate.body {
            self.visit_spanned_statement(inner, ctx);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn visit_routine(
        &mut self,
        _name: &str,
        _params: &[causm_core::ParamDecl],
        _return_type: Option<&causm_core::TypeName>,
        _taking_ms: Option<u64>,
        _state_constraint: Option<&(String, String)>,
        _required_capabilities: &[causm_core::Capability],
        body: &[causm_core::SpannedStatement],
        _span: &causm_core::Span,
        _attrs: &[causm_core::Attribute],
        ctx: &PluginContext,
    ) {
        for stmt in body {
            self.visit_spanned_statement(stmt, ctx);
        }
    }

    fn visit_expression(
        &mut self,
        _expr: &causm_core::Expression,
        _span: &causm_core::Span,
        _ctx: &PluginContext,
    ) {
    }
}

pub mod prelude {
    pub use super::core::*;
    pub use super::{
        causm_plugin, AstVisitor, DiagnosticBuilder, DiagnosticLevel, PluginContext,
        PluginDiagnostic, PluginError, PluginRequest, PluginResponse, PluginStatus,
    };
}
