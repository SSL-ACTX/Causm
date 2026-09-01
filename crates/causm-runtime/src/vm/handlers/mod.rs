pub mod actor;
pub mod arithmetic;
pub mod control_flow;
pub mod entropic;
pub mod ffi;
pub mod loops;
pub mod memory;
pub mod misc;
pub mod speculation;
pub mod temporal;


use crate::vm::context::VmContext;
use crate::vm::error::TemporalError;

pub(crate) fn dispatch_instruction(
    ctx: &mut VmContext<'_>,
    instr: causm_ir::Instruction,
) -> Result<(), TemporalError> {
    #[allow(non_snake_case)]
    macro_rules! dispatch_instruction {
        ($($name:ident $({ $($field:ident: $type:ty),* })?),*) => {
            match instr {
                $(
                    causm_ir::Instruction::$name $({ $($field),* })? => {
                        ctx.vm.$name(ctx.branch_id, $($($field),*)?)
                    }
                )*
            }
        };
    }

    causm_ir::instructions!(dispatch_instruction)
}

#[cfg(test)]
mod tests {
    use super::dispatch_instruction;
    use crate::vm::context::VmContext;
    use crate::vm::state::Vm;
    use causm_core::value::Payload;
    use causm_core::BinaryOperator;
    use causm_ir::{Instruction, Reg};

    #[test]
    fn test_vm_handler_dispatch_loadint_writes_register() -> anyhow::Result<()> {
        let mut vm = Vm::new();
        {
            let mut ctx = VmContext::new(&mut vm, "main");
            dispatch_instruction(
                &mut ctx,
                Instruction::LoadInt {
                    dest: Reg(3),
                    value: 17,
                },
            )?;
        }

        assert_eq!(vm.peek_reg("main", 3)?, Payload::Integer(17));
        Ok(())
    }

    #[test]
    fn test_vm_handler_dispatch_binaryop_computes_result() -> anyhow::Result<()> {
        let mut vm = Vm::new();
        {
            let mut ctx = VmContext::new(&mut vm, "main");
            dispatch_instruction(
                &mut ctx,
                Instruction::LoadInt {
                    dest: Reg(1),
                    value: 9,
                },
            )?;
            dispatch_instruction(
                &mut ctx,
                Instruction::LoadInt {
                    dest: Reg(2),
                    value: 4,
                },
            )?;
            dispatch_instruction(
                &mut ctx,
                Instruction::BinaryOp {
                    dest: Reg(7),
                    op: BinaryOperator::Sub,
                    left: Reg(1),
                    right: Reg(2),
                },
            )?;
        }

        assert_eq!(vm.peek_reg("main", 7)?, Payload::Integer(5));
        Ok(())
    }

    #[test]
    fn test_vm_handler_dispatch_openchan_wires_actor_channel() -> anyhow::Result<()>
    {
        let mut vm = Vm::new();
        {
            let mut ctx = VmContext::new(&mut vm, "main");
            dispatch_instruction(
                &mut ctx,
                Instruction::OpenChan {
                    name: "events".to_string(),
                    capacity: 4,
                    decay_after_ms: Some(20),
                },
            )?;
        }

        assert!(vm.channels.contains_key("events"));
        assert_eq!(vm.channel_decay_limits.get("events"), Some(&20));
        Ok(())
    }
}
