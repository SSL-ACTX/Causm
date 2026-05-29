use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_core::Capability;
use causm_frontend::ir::Reg;

#[allow(non_snake_case, dead_code)]
impl Vm {
    pub(crate) fn GetTsc(
        &mut self,
        branch_id: &str,
        dest: Reg,
    ) -> Result<(), TemporalError> {
        let tsc = crate::vm::jit::hw_timing::read_tsc();
        self.insert_reg(
            branch_id,
            dest.0,
            causm_core::value::EntropicState::Valid(
                causm_core::value::Payload::Integer(tsc as i64),
            ),
        )
    }

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
        println!("[DEBUG] {}: {:?}", branch_id, val);
        let message = format!("{:?}", val);
        let cap = Capability {
            path: "System.Log".to_string(),
            parameters: [("message".to_string(), message)].into(),
        };
        self._execute_capability(branch_id, &cap)
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
