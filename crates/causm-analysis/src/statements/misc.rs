use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use crate::expression::{analyze_expression, analyze_expression_nonconsuming};
use causm_core::*;

#[allow(non_snake_case, unused_variables)]
impl EntropicAnalyzer {
    pub(crate) fn Print(&mut self, expr: &Expression) -> Result<(), SemanticError> {
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

    pub(crate) fn ChannelOpen(
        &mut self,
        _name: &str,
        _capacity: &usize,
    ) -> Result<(), SemanticError> {
        Ok(())
    }

    pub(crate) fn ChannelSend(
        &mut self,
        chan_id: &str,
        value_id: &str,
    ) -> Result<(), SemanticError> {
        if !self.capability_stack.is_empty()
            && !self.is_capability_allowed("Chan.Outbound")
        {
            // Specifically check for this channel ID
            let key = format!("Chan.Outbound[id={}]", chan_id);
            if !self.is_capability_allowed(&key) {
                return Err(self.annotate(SemanticErrorKind::MissingCapability(
                    format!("Chan.Outbound(id={})", chan_id),
                )));
            }
        }
        self.mark_consumed(value_id)
    }

    pub(crate) fn NetworkRequest(
        &mut self,
        _domain: &str,
    ) -> Result<(), SemanticError> {
        if !self.capability_stack.is_empty()
            && !self.is_capability_allowed("System.NetworkFetch")
        {
            return Err(self.annotate(SemanticErrorKind::MissingCapability(
                "System.NetworkFetch".to_string(),
            )));
        }
        Ok(())
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
}
