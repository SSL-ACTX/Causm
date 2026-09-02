use crate::vm::error::TemporalError;
use crate::vm::state::{SpeculationContext, Vm};
use causm_core::SpeculationCommitMode;

#[allow(non_snake_case)]
impl Vm {
    pub(crate) fn Speculate(
        &mut self,
        branch_id: &str,
        _max_ms: u64,
        fallback_target: usize,
    ) -> Result<(), TemporalError> {
        let current_timeline = self.get_branch_mut(branch_id)?.clone();
        let history_index = self.causal_history.len();
        self.speculation_stack.push(SpeculationContext {
            speculation_start_state: current_timeline,
            history_start_index: history_index,
            fallback_target,
            commit_vars: std::collections::HashSet::new(),
            in_commit_block: false,
            commit_executed: false,
            collapse_happened: false,
        });
        Ok(())
    }

    pub(crate) fn EndSpeculate(
        &mut self,
        branch_id: &str,
        _max_ms: u64,
        _fallback_target: usize,
    ) -> Result<(), TemporalError> {
        let context =
            self.speculation_stack
                .pop()
                .ok_or(TemporalError::EvalError(
                    "EndSpeculate without Speculate".into(),
                ))?;

        if !context.commit_executed
            && self.speculative_commit_mode == SpeculationCommitMode::Selective
        {
            // Rollback if selective mode and no commit
            let branch = self.get_branch_mut(branch_id)?;
            let current_pc = branch.pc;
            let current_instrs = branch.instructions.clone();
            let current_loop_depth = branch.loop_depth;
            let current_break = branch.break_requested;

            *branch = context.speculation_start_state;

            branch.pc = current_pc;
            branch.instructions = current_instrs;
            branch.loop_depth = current_loop_depth;
            branch.break_requested = current_break;
        }
        Ok(())
    }

    pub(crate) fn Collapse(&mut self, branch_id: &str) -> Result<(), TemporalError> {
        let context =
            self.speculation_stack
                .pop()
                .ok_or(TemporalError::EvalError(
                    "Collapse outside speculation".into(),
                ))?;
        let fallback_target = context.fallback_target;
        let start_state = context.speculation_start_state;

        let branch = self.get_branch_mut(branch_id)?;
        let saved_instructions = branch.instructions.clone();
        *branch = start_state;
        branch.instructions = saved_instructions;
        branch.pc = fallback_target;
        Ok(())
    }

    pub(crate) fn SpeculationMode(
        &mut self,
        _branch_id: &str,
        mode: SpeculationCommitMode,
    ) -> Result<(), TemporalError> {
        self.speculative_commit_mode = mode;
        Ok(())
    }

    pub(crate) fn Commit(
        &mut self,
        _branch_id: &str,
        _vars: Vec<String>,
    ) -> Result<(), TemporalError> {
        if let Some(ctx) = self.speculation_stack.last_mut() {
            ctx.commit_executed = true;
        }
        Ok(())
    }
}
