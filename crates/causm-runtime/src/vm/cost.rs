use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_core::Statement;

impl Vm {
    pub fn estimate_block_cost(
        &self,
        block: &[causm_core::SpannedStatement],
    ) -> u64 {
        block
            .iter()
            .map(|stmt| self.estimate_statement_cost(&stmt.stmt))
            .sum()
    }

    pub fn estimate_statement_cost(&self, stmt: &Statement) -> u64 {
        stmt.estimate_cost(|b| self.estimate_block_cost(b))
    }
}
