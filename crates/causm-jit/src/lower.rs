use causm_core::{BinaryOperator, UnaryOperator};
use causm_ir::{Instruction, Reg};
use cranelift::prelude::*;
use cranelift_module::Module;
use std::collections::HashMap;

pub struct LoweringContext<'b, 'a, M: Module> {
    pub builder: &'b mut FunctionBuilder<'a>,
    pub module: &'b mut M,
    pub vars: &'b mut HashMap<u32, Variable>,
}

impl<'b, 'a, M: Module> LoweringContext<'b, 'a, M> {
    pub fn get_or_declare_var(&mut self, reg: Reg, ty: types::Type) -> Variable {
        if let Some(&var) = self.vars.get(&reg.0) {
            var
        } else {
            let var = self.builder.declare_var(ty);
            self.vars.insert(reg.0, var);
            var
        }
    }

    pub fn get_var_value(&mut self, reg: Reg) -> Value {
        let var = self.get_or_declare_var(reg, types::I64);
        self.builder.use_var(var)
    }

    pub fn set_var_value(&mut self, reg: Reg, val: Value) {
        let var = self.get_or_declare_var(reg, types::I64);
        self.builder.def_var(var, val);
    }
}

pub fn lower_instruction<M: Module>(
    ctx: &mut LoweringContext<M>,
    instr: &Instruction,
    blocks: &HashMap<usize, Block>,
    aggregates: &mut crate::tuples::AggregateContext,
) {
    match instr {
        Instruction::ConstInt { dest, value }
        | Instruction::LoadInt { dest, value } => {
            let val = ctx.builder.ins().iconst(types::I64, *value);
            ctx.set_var_value(*dest, val);
        }
        Instruction::ConstBool { dest, value }
        | Instruction::LoadBool { dest, value } => {
            let val = ctx
                .builder
                .ins()
                .iconst(types::I64, if *value { 1 } else { 0 });
            ctx.set_var_value(*dest, val);
        }
        Instruction::ConstNull { dest } | Instruction::LoadNull { dest } => {
            let val = ctx.builder.ins().iconst(types::I64, 0);
            ctx.set_var_value(*dest, val);
        }
        Instruction::Move { dest, src } => {
            let val = ctx.get_var_value(*src);
            ctx.set_var_value(*dest, val);
        }
        Instruction::BinaryOp {
            dest,
            op,
            left,
            right,
        } => {
            let l = ctx.get_var_value(*left);
            let r = ctx.get_var_value(*right);
            let res = match op {
                BinaryOperator::Add => ctx.builder.ins().iadd(l, r),
                BinaryOperator::Sub => ctx.builder.ins().isub(l, r),
                BinaryOperator::Mul => ctx.builder.ins().imul(l, r),
                BinaryOperator::Div => ctx.builder.ins().sdiv(l, r),
                BinaryOperator::Rem => ctx.builder.ins().srem(l, r),
                BinaryOperator::Eq => {
                    let cmp = ctx.builder.ins().icmp(IntCC::Equal, l, r);
                    ctx.builder.ins().uextend(types::I64, cmp)
                }
                BinaryOperator::Neq => {
                    let cmp = ctx.builder.ins().icmp(IntCC::NotEqual, l, r);
                    ctx.builder.ins().uextend(types::I64, cmp)
                }
                BinaryOperator::Lt => {
                    let cmp = ctx.builder.ins().icmp(IntCC::SignedLessThan, l, r);
                    ctx.builder.ins().uextend(types::I64, cmp)
                }
                BinaryOperator::Le => {
                    let cmp =
                        ctx.builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r);
                    ctx.builder.ins().uextend(types::I64, cmp)
                }
                BinaryOperator::Gt => {
                    let cmp = ctx.builder.ins().icmp(IntCC::SignedGreaterThan, l, r);
                    ctx.builder.ins().uextend(types::I64, cmp)
                }
                BinaryOperator::Ge => {
                    let cmp = ctx.builder.ins().icmp(
                        IntCC::SignedGreaterThanOrEqual,
                        l,
                        r,
                    );
                    ctx.builder.ins().uextend(types::I64, cmp)
                }
                BinaryOperator::BitwiseAnd | BinaryOperator::LogicalAnd => {
                    ctx.builder.ins().band(l, r)
                }
                BinaryOperator::BitwiseOr | BinaryOperator::LogicalOr => {
                    ctx.builder.ins().bor(l, r)
                }
                BinaryOperator::BitwiseXor => ctx.builder.ins().bxor(l, r),
                BinaryOperator::Shl => ctx.builder.ins().ishl(l, r),
                BinaryOperator::Shr => ctx.builder.ins().sshr(l, r),
                _ => ctx.builder.ins().iconst(types::I64, 0),
            };
            ctx.set_var_value(*dest, res);
        }
        Instruction::UnaryOp { dest, op, src } => {
            let val = ctx.get_var_value(*src);
            let res = match op {
                UnaryOperator::Neg => ctx.builder.ins().ineg(val),
                UnaryOperator::Not => {
                    let zero = ctx.builder.ins().iconst(types::I64, 0);
                    let cmp = ctx.builder.ins().icmp(IntCC::Equal, val, zero);
                    ctx.builder.ins().uextend(types::I64, cmp)
                }
                UnaryOperator::BitwiseNot => ctx.builder.ins().bnot(val),
            };
            ctx.set_var_value(*dest, res);
        }
        Instruction::Jump { target } => {
            if let Some(&target_block) = blocks.get(target) {
                ctx.builder.ins().jump(target_block, &[]);
            }
        }
        Instruction::JumpIf { cond, target } => {
            if let Some(&target_block) = blocks.get(target) {
                let cond_val = ctx.get_var_value(*cond);
                let cond_b1 =
                    ctx.builder.ins().icmp_imm_s(IntCC::NotEqual, cond_val, 0);
                let fallthrough = ctx.builder.create_block();
                ctx.builder
                    .ins()
                    .brif(cond_b1, target_block, &[], fallthrough, &[]);
                ctx.builder.switch_to_block(fallthrough);
            }
        }
        Instruction::JumpIfNot { cond, target } => {
            if let Some(&target_block) = blocks.get(target) {
                let cond_val = ctx.get_var_value(*cond);
                let cond_b1 =
                    ctx.builder.ins().icmp_imm_s(IntCC::Equal, cond_val, 0);
                let fallthrough = ctx.builder.create_block();
                ctx.builder
                    .ins()
                    .brif(cond_b1, target_block, &[], fallthrough, &[]);
                ctx.builder.switch_to_block(fallthrough);
            }
        }
        Instruction::Return { src } => {
            let ret_val = if let Some(r) = src {
                ctx.get_var_value(*r)
            } else {
                ctx.builder.ins().iconst(types::I64, 0)
            };
            ctx.builder.ins().return_(&[ret_val]);
        }
        Instruction::ArrayLit { dest, elements } => {
            let cap_val =
                ctx.builder.ins().iconst(types::I64, elements.len() as i64);
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
            let func_id = ctx
                .module
                .declare_function(
                    "causm_array_new",
                    cranelift_module::Linkage::Import,
                    &sig,
                )
                .unwrap();
            let local_func =
                ctx.module.declare_func_in_func(func_id, ctx.builder.func);
            let call = ctx.builder.ins().call(local_func, &[cap_val]);
            let mut arr_val = ctx.builder.inst_results(call)[0];

            let mut push_sig = ctx.module.make_signature();
            push_sig.params.push(AbiParam::new(types::I64));
            push_sig.params.push(AbiParam::new(types::I64));
            push_sig.returns.push(AbiParam::new(types::I64));
            let push_id = ctx
                .module
                .declare_function(
                    "causm_array_push",
                    cranelift_module::Linkage::Import,
                    &push_sig,
                )
                .unwrap();
            let local_push =
                ctx.module.declare_func_in_func(push_id, ctx.builder.func);

            for elem in elements {
                let el_val = ctx.get_var_value(*elem);
                let p_call = ctx.builder.ins().call(local_push, &[arr_val, el_val]);
                arr_val = ctx.builder.inst_results(p_call)[0];
            }
            ctx.set_var_value(*dest, arr_val);
        }
        Instruction::ArrayLen { dest, src } => {
            let arr_val = ctx.get_var_value(*src);
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
            let func_id = ctx
                .module
                .declare_function(
                    "causm_array_len",
                    cranelift_module::Linkage::Import,
                    &sig,
                )
                .unwrap();
            let local_func =
                ctx.module.declare_func_in_func(func_id, ctx.builder.func);
            let call = ctx.builder.ins().call(local_func, &[arr_val]);
            let len_val = ctx.builder.inst_results(call)[0];
            ctx.set_var_value(*dest, len_val);
        }
        Instruction::IndexAccess {
            dest,
            target,
            index,
        } => {
            let arr_val = ctx.get_var_value(*target);
            let idx_val = ctx.get_var_value(*index);
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
            let func_id = ctx
                .module
                .declare_function(
                    "causm_array_get",
                    cranelift_module::Linkage::Import,
                    &sig,
                )
                .unwrap();
            let local_func =
                ctx.module.declare_func_in_func(func_id, ctx.builder.func);
            let call = ctx.builder.ins().call(local_func, &[arr_val, idx_val]);
            let elem_val = ctx.builder.inst_results(call)[0];
            ctx.set_var_value(*dest, elem_val);
        }
        Instruction::TupleLit { dest, elems } => {
            aggregates.lower_tuple_lit(ctx, *dest, elems);
        }
        Instruction::TupleAccess { dest, tuple, index } => {
            aggregates.lower_tuple_access(ctx, *dest, *tuple, *index);
        }
        _ => {}
    }
}
