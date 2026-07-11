use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_core::value::{EntropicState, Payload};
use causm_ir::Reg;

#[allow(non_snake_case)]
impl Vm {
    pub(crate) fn LoadInt(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: i64,
    ) -> Result<(), TemporalError> {
        self.insert_reg(
            branch_id,
            dest.0,
            EntropicState::Valid(Payload::Integer(value)),
        )
    }

    pub(crate) fn LoadFloat(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: u64,
    ) -> Result<(), TemporalError> {
        self.insert_reg(
            branch_id,
            dest.0,
            EntropicState::Valid(Payload::Float(value)),
        )
    }

    pub(crate) fn LoadBool(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: bool,
    ) -> Result<(), TemporalError> {
        self.insert_reg(
            branch_id,
            dest.0,
            EntropicState::Valid(Payload::Bool(value)),
        )
    }

    pub(crate) fn LoadString(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: String,
    ) -> Result<(), TemporalError> {
        self.insert_reg(
            branch_id,
            dest.0,
            EntropicState::Valid(Payload::String(value)),
        )
    }

    pub(crate) fn LoadNull(
        &mut self,
        branch_id: &str,
        dest: Reg,
    ) -> Result<(), TemporalError> {
        self.insert_reg(branch_id, dest.0, EntropicState::Valid(Payload::Null))
    }

    pub(crate) fn Move(
        &mut self,
        branch_id: &str,
        dest: Reg,
        src: Reg,
    ) -> Result<(), TemporalError> {
        let state = self.peek_state(branch_id, src.0)?;
        if matches!(state, EntropicState::Consumed) {
            return Err(TemporalError::MemoryFault(
                causm_core::value::MemoryError::AlreadyConsumed,
            ));
        }
        self.insert_reg(branch_id, dest.0, state)
    }

    pub(crate) fn BinaryOp(
        &mut self,
        branch_id: &str,
        dest: Reg,
        op: causm_core::BinaryOperator,
        left: Reg,
        right: Reg,
    ) -> Result<(), TemporalError> {
        let l_val = self.peek_reg(branch_id, left.0)?;
        let r_val = self.peek_reg(branch_id, right.0)?;
        let result = self.evaluate_binary_operation(l_val, r_val, &op)?;
        self.insert_reg(branch_id, dest.0, EntropicState::Valid(result))
    }

    pub(crate) fn UnaryOp(
        &mut self,
        branch_id: &str,
        dest: Reg,
        op: causm_core::UnaryOperator,
        src: Reg,
    ) -> Result<(), TemporalError> {
        let val = self.peek_reg(branch_id, src.0)?;
        let result = self.evaluate_unary_operation(val, &op)?;
        self.insert_reg(branch_id, dest.0, EntropicState::Valid(result))
    }
}
