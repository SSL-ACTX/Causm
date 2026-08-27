use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use crate::expression::{analyze_expression, analyze_expression_nonconsuming};
use causm_core::*;

#[allow(non_snake_case, unused_variables)]
impl EntropicAnalyzer {
    pub(crate) fn Print(
        &mut self,
        args: &[Expression],
    ) -> Result<(), SemanticError> {
        for arg in args {
            analyze_expression_nonconsuming(self, arg)?;
        }
        if !self.capability_stack.is_empty()
            && !self.is_capability_allowed("System.Log")
        {
            return Err(self.annotate(SemanticErrorKind::MissingCapability(
                "System.Log".to_string(),
            )));
        }
        Ok(())
    }

    pub(crate) fn ForeignBlock(
        &mut self,
        lib_name: &str,
        _abi: &str,
        routines: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        if !self.capability_stack.is_empty()
            && !self.is_capability_allowed("System.FFI")
            && !self.is_capability_allowed("System.WASI")
        {
            return Err(self.annotate(SemanticErrorKind::MissingCapability(
                "System.FFI".to_string(),
            )));
        }
        if lib_name.starts_with('/')
            && !lib_name.starts_with("/lib")
            && !lib_name.starts_with("/usr/lib")
            && !lib_name.starts_with("/system/lib")
            && !lib_name.starts_with("/data/data/com.termux")
        {
            return Err(self.annotate(SemanticErrorKind::ForbiddenLibraryPath(
                lib_name.to_string(),
            )));
        }
        for r in routines {
            self.analyze_statement(r)?;
        }
        Ok(())
    }

    pub(crate) fn Debug(&mut self, expr: &Expression) -> Result<(), SemanticError> {
        analyze_expression_nonconsuming(self, expr)?;
        if !self.capability_stack.is_empty()
            && !self.is_capability_allowed("System.Log")
        {
            return Err(self.annotate(SemanticErrorKind::MissingCapability(
                "System.Log".to_string(),
            )));
        }
        Ok(())
    }

    pub(crate) fn Send(
        &mut self,
        value_id: &str,
        _target_branch: &str,
    ) -> Result<(), SemanticError> {
        self.mark_consumed(value_id)
    }

    pub(crate) fn Capability(
        &mut self,
        cap: &Capability,
    ) -> Result<(), SemanticError> {
        if !self.is_capability_allowed(&cap.path) {
            return Err(self
                .annotate(SemanticErrorKind::MissingCapability(cap.path.clone())));
        }
        Ok(())
    }

    pub(crate) fn Expression(
        &mut self,
        expr: &Expression,
    ) -> Result<(), SemanticError> {
        analyze_expression(self, expr)
    }

    pub(crate) fn Import(
        &mut self,
        _path: &String,
        _alias: &Option<String>,
    ) -> Result<(), SemanticError> {
        Ok(())
    }

    pub(crate) fn FromImport(
        &mut self,
        _path: &String,
        _symbols: &[(String, Option<String>)],
    ) -> Result<(), SemanticError> {
        Ok(())
    }
}
