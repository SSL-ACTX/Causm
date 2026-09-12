use causm_core::{BinaryOperator, UnaryOperator};
use causm_ir::cfg::BlockId;
use causm_ir::ssa::{SsaCFG, SsaInstruction, SsaReg, SsaTerminator};
use cranelift::codegen::ir::{
    BlockArg, MemFlagsData, StackSlotData, StackSlotKind, TrapCode,
};
use cranelift::prelude::*;
use cranelift_module::Module;
use std::collections::{HashMap, HashSet};

pub struct SsaCodegenContext<'b, 'a, M: Module> {
    pub builder: &'b mut FunctionBuilder<'a>,
    pub module: &'b mut M,
    pub cl_blocks: HashMap<BlockId, Block>,
    pub values: HashMap<SsaReg, Value>,
    pub unpacked_tuples: HashMap<SsaReg, Vec<Value>>,
}

impl<'b, 'a, M: Module> SsaCodegenContext<'b, 'a, M> {
    pub fn get_val(&mut self, reg: SsaReg) -> Value {
        if let Some(&val) = self.values.get(&reg) {
            val
        } else {
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.values.insert(reg, zero);
            zero
        }
    }
}

pub fn compile_ssa_cfg<M: Module>(
    builder: &mut FunctionBuilder,
    module: &mut M,
    ssa_cfg: &SsaCFG,
    param_count: usize,
) {
    let mut cl_blocks = HashMap::new();
    for &id in ssa_cfg.blocks.keys() {
        cl_blocks.insert(id, builder.create_block());
    }

    let entry_cl_block = cl_blocks[&ssa_cfg.entry_block];
    builder.append_block_params_for_function_params(entry_cl_block);

    let mut values = HashMap::new();
    let unpacked_tuples = HashMap::new();

    // Map routine entry parameters to SsaReg { reg: i, version: 0 }
    for i in 0..param_count {
        let param_val = builder.block_params(entry_cl_block)[i];
        values.insert(
            SsaReg {
                reg: i as u32,
                version: 0,
            },
            param_val,
        );
    }

    // Pre-declare block parameters for all phi nodes in every block
    for (&block_id, block) in &ssa_cfg.blocks {
        let cl_block = cl_blocks[&block_id];
        for phi in &block.phi_nodes {
            let phi_param = builder.append_block_param(cl_block, types::I64);
            values.insert(phi.dest, phi_param);
        }
    }

    let mut ctx = SsaCodegenContext {
        builder,
        module,
        cl_blocks,
        values,
        unpacked_tuples,
    };

    // Traverse blocks in Reverse Post-Order (RPO) starting from entry_block
    let mut rpo = Vec::new();
    let mut visited = HashSet::new();
    dfs_rpo(ssa_cfg.entry_block, ssa_cfg, &mut visited, &mut rpo);
    rpo.reverse();

    // Also include any detached/unreachable blocks to satisfy Cranelift completeness
    for &id in ssa_cfg.blocks.keys() {
        if !visited.contains(&id) {
            rpo.push(id);
        }
    }

    for block_id in rpo {
        let block = &ssa_cfg.blocks[&block_id];
        let cl_block = ctx.cl_blocks[&block_id];
        ctx.builder.switch_to_block(cl_block);

        for instr in &block.instructions {
            lower_ssa_instruction(&mut ctx, instr);
        }

        lower_ssa_terminator(&mut ctx, block_id, &block.terminator, ssa_cfg);
    }
}

fn dfs_rpo(
    node: BlockId,
    cfg: &SsaCFG,
    visited: &mut HashSet<BlockId>,
    post_order: &mut Vec<BlockId>,
) {
    visited.insert(node);
    if let Some(block) = cfg.blocks.get(&node) {
        for succ in get_terminator_successors(&block.terminator) {
            if !visited.contains(&succ) {
                dfs_rpo(succ, cfg, visited, post_order);
            }
        }
    }
    post_order.push(node);
}

fn get_terminator_successors(term: &SsaTerminator) -> Vec<BlockId> {
    match term {
        SsaTerminator::Jump { target } => vec![*target],
        SsaTerminator::Branch {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        SsaTerminator::MatchEntropy {
            valid_block,
            decayed_block,
            pending_block,
            consumed_block,
            ..
        } => [
            *valid_block,
            *decayed_block,
            *pending_block,
            *consumed_block,
        ]
        .into_iter()
        .flatten()
        .collect(),
        SsaTerminator::Select {
            cases,
            timeout_block,
            ..
        } => {
            let mut succs: Vec<BlockId> =
                cases.iter().map(|c| c.target as BlockId).collect();
            if let Some(tb) = timeout_block {
                succs.push(*tb);
            }
            succs
        }
        SsaTerminator::Return { .. } | SsaTerminator::Unreachable => vec![],
    }
}

fn get_phi_block_args<M: Module>(
    ctx: &mut SsaCodegenContext<M>,
    from_block: BlockId,
    target_block: BlockId,
    ssa_cfg: &SsaCFG,
) -> Vec<BlockArg> {
    let mut args = Vec::new();
    if let Some(target_b) = ssa_cfg.blocks.get(&target_block) {
        for phi in &target_b.phi_nodes {
            let mut matched_val = None;
            for (pred_id, src_reg) in &phi.incoming {
                if *pred_id == from_block {
                    matched_val = Some(ctx.get_val(*src_reg));
                    break;
                }
            }
            let val = matched_val.unwrap_or_else(|| {
                // Fallback for unreachable / undefined paths
                ctx.builder.ins().iconst(types::I64, 0)
            });
            args.push(BlockArg::Value(val));
        }
    }
    args
}

fn lower_ssa_instruction<M: Module>(
    ctx: &mut SsaCodegenContext<M>,
    instr: &SsaInstruction,
) {
    match instr {
        SsaInstruction::ConstInt { dest, value }
        | SsaInstruction::LoadInt { dest, value } => {
            let val = ctx.builder.ins().iconst(types::I64, *value);
            ctx.values.insert(*dest, val);
        }
        SsaInstruction::ConstBool { dest, value }
        | SsaInstruction::LoadBool { dest, value } => {
            let val = ctx
                .builder
                .ins()
                .iconst(types::I64, if *value { 1 } else { 0 });
            ctx.values.insert(*dest, val);
        }
        SsaInstruction::ConstNull { dest } | SsaInstruction::LoadNull { dest } => {
            let val = ctx.builder.ins().iconst(types::I64, 0);
            ctx.values.insert(*dest, val);
        }
        SsaInstruction::Move { dest, src } | SsaInstruction::Clone { dest, src } => {
            let val = ctx.get_val(*src);
            ctx.values.insert(*dest, val);
        }
        SsaInstruction::ConstString { dest, value }
        | SsaInstruction::LoadString { dest, value } => {
            let leaked = Box::leak(value.clone().into_boxed_str());
            let data_ptr = leaked.as_ptr();
            let len = leaked.len();
            let ptr_val = ctx
                .builder
                .ins()
                .iconst(types::I64, data_ptr as usize as i64);
            let len_val = ctx.builder.ins().iconst(types::I64, len as i64);

            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
            let func_id = ctx
                .module
                .declare_function(
                    "causm_str_new",
                    cranelift_module::Linkage::Import,
                    &sig,
                )
                .unwrap();
            let local_func =
                ctx.module.declare_func_in_func(func_id, ctx.builder.func);
            let call = ctx.builder.ins().call(local_func, &[ptr_val, len_val]);
            let str_val = ctx.builder.inst_results(call)[0];
            ctx.values.insert(*dest, str_val);
        }
        SsaInstruction::ArraySlice {
            dest,
            target,
            start,
            end,
            inclusive,
        } => {
            let s_val = ctx.get_val(*target);
            let start_val = if let Some(st) = start {
                ctx.get_val(*st)
            } else {
                ctx.builder.ins().iconst(types::I64, 0)
            };
            let end_val = if let Some(en) = end {
                let v = ctx.get_val(*en);
                if *inclusive {
                    ctx.builder.ins().iadd_imm_s(v, 1)
                } else {
                    v
                }
            } else {
                ctx.builder.ins().iconst(types::I64, isize::MAX as i64)
            };
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
            let func_id = ctx
                .module
                .declare_function(
                    "causm_str_slice",
                    cranelift_module::Linkage::Import,
                    &sig,
                )
                .unwrap();
            let local_func =
                ctx.module.declare_func_in_func(func_id, ctx.builder.func);
            let call = ctx
                .builder
                .ins()
                .call(local_func, &[s_val, start_val, end_val]);
            let sliced = ctx.builder.inst_results(call)[0];
            ctx.values.insert(*dest, sliced);
        }
        SsaInstruction::BinaryOp {
            dest,
            op,
            left,
            right,
        } => {
            let l = ctx.get_val(*left);
            let r = ctx.get_val(*right);
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
            ctx.values.insert(*dest, res);
        }
        SsaInstruction::UnaryOp { dest, op, src } => {
            let val = ctx.get_val(*src);
            let res = match op {
                UnaryOperator::Neg => ctx.builder.ins().ineg(val),
                UnaryOperator::Not => {
                    let zero = ctx.builder.ins().iconst(types::I64, 0);
                    let cmp = ctx.builder.ins().icmp(IntCC::Equal, val, zero);
                    ctx.builder.ins().uextend(types::I64, cmp)
                }
                UnaryOperator::BitwiseNot => ctx.builder.ins().bnot(val),
            };
            ctx.values.insert(*dest, res);
        }
        SsaInstruction::ConditionalSelect {
            dest,
            cond,
            true_val,
            false_val,
        } => {
            let c = ctx.get_val(*cond);
            let t = ctx.get_val(*true_val);
            let f = ctx.get_val(*false_val);
            let cond_b1 = ctx.builder.ins().icmp_imm_s(IntCC::NotEqual, c, 0);
            let res = ctx.builder.ins().select(cond_b1, t, f);
            ctx.values.insert(*dest, res);
        }
        SsaInstruction::ArrayLit { dest, elements } => {
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
                let el_val = ctx.get_val(*elem);
                let p_call = ctx.builder.ins().call(local_push, &[arr_val, el_val]);
                arr_val = ctx.builder.inst_results(p_call)[0];
            }
            ctx.values.insert(*dest, arr_val);
        }
        SsaInstruction::ArrayLen { dest, src } => {
            let arr_val = ctx.get_val(*src);
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
            ctx.values.insert(*dest, len_val);
        }
        SsaInstruction::IndexAccess {
            dest,
            target,
            index,
        } => {
            let arr_val = ctx.get_val(*target);
            let idx_val = ctx.get_val(*index);
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
            ctx.values.insert(*dest, elem_val);
        }
        SsaInstruction::TupleLit { dest, elems } => {
            let size = (elems.len() * 8) as u32;
            let slot = ctx.builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                size.max(8),
                3, // 8-byte alignment
            ));
            let ptr_ty = ctx.module.target_config().pointer_type();
            let mut flat_vals = Vec::with_capacity(elems.len());

            for (i, elem) in elems.iter().enumerate() {
                let val = ctx.get_val(*elem);
                flat_vals.push(val);
                let offset = (i * 8) as i32;
                let addr = ctx.builder.ins().stack_addr(ptr_ty, slot, offset);
                ctx.builder.ins().store(MemFlagsData::new(), val, addr, 0);
            }

            let addr = ctx.builder.ins().stack_addr(ptr_ty, slot, 0);
            ctx.values.insert(*dest, addr);
            ctx.unpacked_tuples.insert(*dest, flat_vals);
        }
        SsaInstruction::TupleAccess { dest, tuple, index } => {
            if let Some(flat_vals) = ctx.unpacked_tuples.get(tuple) {
                if *index < flat_vals.len() {
                    let val = flat_vals[*index];
                    ctx.values.insert(*dest, val);
                    return;
                }
            }
            let tuple_addr = ctx.get_val(*tuple);
            let offset = (*index * 8) as i32;
            let val = ctx.builder.ins().load(
                types::I64,
                MemFlagsData::new(),
                tuple_addr,
                offset,
            );
            ctx.values.insert(*dest, val);
        }
        SsaInstruction::Call {
            routine,
            args,
            dest,
        } => {
            let mut sig = ctx.module.make_signature();
            for _ in args {
                sig.params.push(AbiParam::new(types::I64));
            }
            sig.returns.push(AbiParam::new(types::I64));

            let func_id = ctx
                .module
                .declare_function(routine, cranelift_module::Linkage::Import, &sig)
                .unwrap();
            let local_func =
                ctx.module.declare_func_in_func(func_id, ctx.builder.func);
            let cl_args: Vec<Value> = args.iter().map(|a| ctx.get_val(*a)).collect();
            let call = ctx.builder.ins().call(local_func, &cl_args);
            let ret_val = ctx.builder.inst_results(call)[0];
            ctx.values.insert(*dest, ret_val);
        }
        SsaInstruction::YieldPad => {
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(types::I64));
            let func_id = ctx
                .module
                .declare_function(
                    "causm_spin_pad",
                    cranelift_module::Linkage::Import,
                    &sig,
                )
                .unwrap();
            let local_func =
                ctx.module.declare_func_in_func(func_id, ctx.builder.func);
            let pad_cycles = ctx.builder.ins().iconst(types::I64, 100);
            ctx.builder.ins().call(local_func, &[pad_cycles]);
        }
        _ => {}
    }
}

fn lower_ssa_terminator<M: Module>(
    ctx: &mut SsaCodegenContext<M>,
    current_block_id: BlockId,
    term: &SsaTerminator,
    ssa_cfg: &SsaCFG,
) {
    match term {
        SsaTerminator::Jump { target } => {
            let args = get_phi_block_args(ctx, current_block_id, *target, ssa_cfg);
            let dest_block = ctx.cl_blocks[target];
            ctx.builder.ins().jump(dest_block, &args);
        }
        SsaTerminator::Branch {
            cond,
            then_block,
            else_block,
        } => {
            let c = ctx.get_val(*cond);
            let cond_b1 = ctx.builder.ins().icmp_imm_s(IntCC::NotEqual, c, 0);

            let then_args =
                get_phi_block_args(ctx, current_block_id, *then_block, ssa_cfg);
            let else_args =
                get_phi_block_args(ctx, current_block_id, *else_block, ssa_cfg);

            let t_block = ctx.cl_blocks[then_block];
            let e_block = ctx.cl_blocks[else_block];

            ctx.builder
                .ins()
                .brif(cond_b1, t_block, &then_args, e_block, &else_args);
        }
        SsaTerminator::Return { src } => {
            let ret_val = match src {
                Some(r) => ctx.get_val(*r),
                None => ctx.builder.ins().iconst(types::I64, 0),
            };
            ctx.builder.ins().return_(&[ret_val]);
        }
        SsaTerminator::MatchEntropy {
            target,
            valid_block,
            decayed_block,
            pending_block,
            consumed_block,
        } => {
            let s_val = ctx.get_val(*target);
            let mut switch = cranelift::frontend::Switch::new();

            let cases = [
                (0u64, valid_block),
                (2u64, decayed_block),
                (3u64, pending_block),
                (4u64, consumed_block),
            ];

            let default_cl_block = ctx.builder.create_block();

            for (tag, target_opt) in cases {
                if let Some(target_id) = target_opt {
                    let target_cl = ctx.cl_blocks[target_id];
                    let args = get_phi_block_args(
                        ctx,
                        current_block_id,
                        *target_id,
                        ssa_cfg,
                    );
                    if args.is_empty() {
                        switch.set_entry(tag.into(), target_cl);
                    } else {
                        let trampoline = ctx.builder.create_block();
                        switch.set_entry(tag.into(), trampoline);
                        // Emit jump with args in trampoline later
                        let curr = ctx.builder.current_block().unwrap();
                        ctx.builder.switch_to_block(trampoline);
                        ctx.builder.ins().jump(target_cl, &args);
                        ctx.builder.switch_to_block(curr);
                    }
                }
            }

            switch.emit(ctx.builder, s_val, default_cl_block);
            ctx.builder.switch_to_block(default_cl_block);
            ctx.builder.ins().trap(TrapCode::unwrap_user(1));
        }
        SsaTerminator::Unreachable => {
            ctx.builder.ins().trap(TrapCode::unwrap_user(1));
        }
        _ => {
            let zero = ctx.builder.ins().iconst(types::I64, 0);
            ctx.builder.ins().return_(&[zero]);
        }
    }
}
