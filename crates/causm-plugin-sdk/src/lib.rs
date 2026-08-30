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

/// Mutable AST Transformer trait (AstFolder) for rewriting AST nodes safely.
pub trait AstFolder {
    fn fold_program(
        &mut self,
        mut program: causm_core::Program,
        ctx: &PluginContext,
    ) -> Result<causm_core::Program, PluginError> {
        let mut folded_timelines = Vec::with_capacity(program.timelines.len());
        for tb in program.timelines {
            folded_timelines.push(self.fold_timeline(tb, ctx)?);
        }
        program.timelines = folded_timelines;
        Ok(program)
    }

    fn fold_timeline(
        &mut self,
        mut tb: causm_core::TimelineBlock,
        ctx: &PluginContext,
    ) -> Result<causm_core::TimelineBlock, PluginError> {
        let mut folded_statements = Vec::with_capacity(tb.statements.len());
        for stmt in tb.statements {
            folded_statements.push(self.fold_spanned_statement(stmt, ctx)?);
        }
        tb.statements = folded_statements;
        Ok(tb)
    }

    fn fold_spanned_statement(
        &mut self,
        mut stmt: causm_core::SpannedStatement,
        ctx: &PluginContext,
    ) -> Result<causm_core::SpannedStatement, PluginError> {
        stmt.stmt =
            self.fold_statement(stmt.stmt, &stmt.span, &stmt.attributes, ctx)?;
        Ok(stmt)
    }

    fn fold_statement(
        &mut self,
        stmt: causm_core::Statement,
        span: &causm_core::Span,
        _attrs: &[causm_core::Attribute],
        ctx: &PluginContext,
    ) -> Result<causm_core::Statement, PluginError> {
        match stmt {
            causm_core::Statement::Assignment {
                target,
                mutable,
                var_type,
                lifetime,
                expr,
            } => {
                let folded_expr = self.fold_expression(expr, span, ctx)?;
                Ok(causm_core::Statement::Assignment {
                    target,
                    mutable,
                    var_type,
                    lifetime,
                    expr: folded_expr,
                })
            }
            causm_core::Statement::Isolate(mut isolate) => {
                let mut folded_body = Vec::with_capacity(isolate.body.len());
                for inner in isolate.body {
                    folded_body.push(self.fold_spanned_statement(inner, ctx)?);
                }
                isolate.body = folded_body;
                Ok(causm_core::Statement::Isolate(isolate))
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
                let mut folded_body = Vec::with_capacity(body.len());
                for inner in body {
                    folded_body.push(self.fold_spanned_statement(inner, ctx)?);
                }
                Ok(causm_core::Statement::RoutineDef {
                    name,
                    params,
                    return_type,
                    taking_ms,
                    state_constraint,
                    required_capabilities,
                    body: folded_body,
                })
            }
            causm_core::Statement::Expression(expr) => {
                let folded_expr = self.fold_expression(expr, span, ctx)?;
                Ok(causm_core::Statement::Expression(folded_expr))
            }
            causm_core::Statement::Return(opt_expr) => {
                let folded = match opt_expr {
                    Some(e) => Some(self.fold_expression(e, span, ctx)?),
                    None => None,
                };
                Ok(causm_core::Statement::Return(folded))
            }
            other => Ok(other),
        }
    }

    fn fold_expression(
        &mut self,
        expr: causm_core::Expression,
        _span: &causm_core::Span,
        _ctx: &PluginContext,
    ) -> Result<causm_core::Expression, PluginError> {
        Ok(expr)
    }
}

pub mod prelude {
    pub use super::core::*;
    pub use super::{
        causm_plugin, AstFolder, AstVisitor, DiagnosticBuilder, DiagnosticLevel,
        PluginContext, PluginDiagnostic, PluginError, PluginRequest, PluginResponse,
        PluginStatus,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use causm_core::*;

    struct ConstantAdderFolder {
        add_val: i64,
    }

    impl AstFolder for ConstantAdderFolder {
        fn fold_expression(
            &mut self,
            expr: Expression,
            _span: &Span,
            _ctx: &PluginContext,
        ) -> Result<Expression, PluginError> {
            match expr {
                Expression::Integer(n) => Ok(Expression::Integer(n + self.add_val)),
                other => Ok(other),
            }
        }
    }

    #[test]
    fn test_ast_folder_expression_transformation() {
        let program = Program {
            timelines: vec![TimelineBlock {
                time: TimeCoordinate::Global(0),
                no_z3: false,
                entropy_mode: None,
                statements: vec![SpannedStatement::new(
                    Statement::Assignment {
                        target: "count".to_string(),
                        mutable: true,
                        var_type: None,
                        lifetime: None,
                        expr: Expression::Integer(10),
                    },
                    Span { start: 0, end: 10 },
                )],
            }],
        };

        let ctx = PluginContext {
            protocol_version: "0.1.0".to_string(),
            compiler_version: "0.1.0".to_string(),
            target_arch: "x86_64".to_string(),
            target_os: "linux".to_string(),
            file_path: "main.csm".to_string(),
            options: std::collections::HashMap::new(),
        };

        let mut folder = ConstantAdderFolder { add_val: 32 };
        let folded_program = folder
            .fold_program(program, &ctx)
            .expect("folding should succeed");

        if let Statement::Assignment { expr, .. } =
            &folded_program.timelines[0].statements[0].stmt
        {
            assert_eq!(*expr, Expression::Integer(42));
        } else {
            panic!("Expected assignment statement");
        }
    }
}
