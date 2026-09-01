use crate::vm::state::Vm;
use causm_core::value::Payload;
use causm_ir::Instruction;
use crate::vm::error::TemporalError;

pub struct VmContext<'a> {
    pub vm: &'a mut Vm,
    pub branch_id: &'a str,
}

impl<'a> VmContext<'a> {
    pub fn new(vm: &'a mut Vm, branch_id: &'a str) -> Self {
        Self { vm, branch_id }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepAction {
    Continue,
    Jump(usize),
    Yield(Option<Payload>),
    Return(Option<Payload>),
    SuspendActor,
    Terminate,
}

pub trait InstructionHandler<I = Instruction> {
    fn execute(
        ctx: &mut VmContext<'_>,
        instruction: I,
    ) -> Result<StepAction, TemporalError>;
}
