use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_core::Capability;
use causm_ir::Reg;

#[allow(non_snake_case)]
impl Vm {
    pub(crate) fn Print(
        &mut self,
        branch_id: &str,
        src: Reg,
    ) -> Result<(), TemporalError> {
        let val = self.peek_reg(branch_id, src.0)?;
        let message = val.to_string();
        let cap = Capability {
            path: "System.Log".to_string(),
            parameters: [("message".to_string(), message)].into(),
        };
        self._execute_capability(branch_id, &cap)
    }

    pub(crate) fn Debug(
        &mut self,
        branch_id: &str,
        src: Reg,
    ) -> Result<(), TemporalError> {
        let val = self.peek_reg(branch_id, src.0)?;
        println!("[debug] {}", val);
        Ok(())
    }

    pub(crate) fn Capability(
        &mut self,
        branch_id: &str,
        cap: causm_core::Capability,
    ) -> Result<(), TemporalError> {
        self._execute_capability(branch_id, &cap)
    }

    pub(crate) fn NetworkRequest(
        &mut self,
        branch_id: &str,
        _domain: String,
    ) -> Result<(), TemporalError> {
        if !self.capability_handlers.contains_key("System.NetworkFetch") {
            return Err(TemporalError::MissingCapability(
                "System.NetworkFetch".to_string(),
            ));
        }
        let branch = self.get_branch_mut(branch_id)?;
        branch.local_clock += 5;
        branch.consume_budget(5)?;
        Ok(())
    }
}
