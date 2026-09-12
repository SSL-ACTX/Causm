use causm_ir::Reg;
use cranelift::codegen::ir::{MemFlagsData, StackSlotData, StackSlotKind};
use cranelift::prelude::*;
use cranelift_module::Module;
use std::collections::HashMap;

use crate::lower::LoweringContext;

pub struct AggregateContext {
    pub unpacked_values: HashMap<u32, Vec<Value>>,
}

impl Default for AggregateContext {
    fn default() -> Self {
        Self::new()
    }
}

impl AggregateContext {
    pub fn new() -> Self {
        Self {
            unpacked_values: HashMap::new(),
        }
    }

    pub fn lower_tuple_lit<M: Module>(
        &mut self,
        ctx: &mut LoweringContext<M>,
        dest: Reg,
        elems: &[Reg],
    ) {
        let size = (elems.len() * 8) as u32;
        let slot = ctx.builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            size.max(8),
            3, // 8-byte alignment (2^3)
        ));

        let ptr_ty = ctx.module.target_config().pointer_type();
        let mut flat_vals = Vec::with_capacity(elems.len());

        for (i, elem) in elems.iter().enumerate() {
            let val = ctx.get_var_value(*elem);
            flat_vals.push(val);
            let offset = (i * 8) as i32;
            let addr = ctx.builder.ins().stack_addr(ptr_ty, slot, offset);
            ctx.builder.ins().store(MemFlagsData::new(), val, addr, 0);
        }

        let addr = ctx.builder.ins().stack_addr(ptr_ty, slot, 0);
        ctx.set_var_value(dest, addr);
        self.unpacked_values.insert(dest.0, flat_vals);
    }

    pub fn lower_tuple_access<M: Module>(
        &mut self,
        ctx: &mut LoweringContext<M>,
        dest: Reg,
        tuple: Reg,
        index: usize,
    ) {
        // Fast path: direct register access from unpacked virtual values (0-cycle)
        if let Some(flat_vals) = self.unpacked_values.get(&tuple.0) {
            if index < flat_vals.len() {
                let val = flat_vals[index];
                ctx.set_var_value(dest, val);
                return;
            }
        }

        // Fallback: memory load from stack/heap aggregate pointer
        let tuple_addr = ctx.get_var_value(tuple);
        let offset = (index * 8) as i32;
        let val = ctx.builder.ins().load(
            types::I64,
            MemFlagsData::new(),
            tuple_addr,
            offset,
        );
        ctx.set_var_value(dest, val);
    }
}
