use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_core::value::Payload;
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
        self._execute_capability(branch_id, &cap)?;
        Ok(())
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
        self._execute_capability(branch_id, &cap)?;
        Ok(())
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

    pub(crate) fn Syscall(
        &mut self,
        branch_id: &str,
        dest: Reg,
        target: causm_core::SyscallTarget,
        args: Vec<Reg>,
        duration_ms: Option<u64>,
    ) -> Result<(), TemporalError> {
        let cost = duration_ms.unwrap_or(1);
        let arg_vals: Vec<_> = args
            .iter()
            .filter_map(|r| self.peek_reg(branch_id, r.0).ok())
            .collect();

        let ret_val = match target {
            causm_core::SyscallTarget::Symbol(ref sym) => {
                if sym == "sys_write" {
                    if let Some(Payload::String(ref s)) = arg_vals.get(1) {
                        use std::io::Write;
                        let fd = match arg_vals.first() {
                            Some(Payload::Integer(i)) => *i,
                            _ => 1,
                        };
                        if fd == 2 {
                            let _ = std::io::stderr().write_all(s.as_bytes());
                            let _ = std::io::stderr().flush();
                        } else {
                            let _ = std::io::stdout().write_all(s.as_bytes());
                            let _ = std::io::stdout().flush();
                        }
                    }
                }
                arg_vals.len() as i64
            }
            causm_core::SyscallTarget::Number(n) => {
                // x86_64: 1 = write, AArch64: 64 = write
                if n == 1 || n == 64 {
                    if let Some(Payload::String(ref s)) = arg_vals.get(1) {
                        use std::io::Write;
                        let fd = match arg_vals.first() {
                            Some(Payload::Integer(i)) => *i,
                            _ => 1,
                        };
                        if fd == 2 {
                            let _ = std::io::stderr().write_all(s.as_bytes());
                            let _ = std::io::stderr().flush();
                        } else {
                            let _ = std::io::stdout().write_all(s.as_bytes());
                            let _ = std::io::stdout().flush();
                        }
                    }
                }
                n
            }
        };

        self.insert_reg(
            branch_id,
            dest.0,
            causm_core::value::EntropicState::Valid(Payload::Integer(ret_val)),
        )?;
        let branch = self.get_branch_mut(branch_id)?;
        branch.local_clock += cost;
        branch.consume_budget(cost)?;
        Ok(())
    }
}
