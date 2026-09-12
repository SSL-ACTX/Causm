use cranelift::codegen::ir::{MemFlagsData, StackSlot};
use cranelift::prelude::*;

pub fn copy_memory(
    builder: &mut FunctionBuilder,
    src_ptr: Value,
    dest_ptr: Value,
    size: usize,
) {
    let mut curr_offset = 0;
    while curr_offset < size {
        let bytes_left = size - curr_offset;
        let (cl_ty, chunk_size) = if bytes_left >= 8 {
            (types::I64, 8)
        } else if bytes_left >= 4 {
            (types::I32, 4)
        } else if bytes_left >= 2 {
            (types::I16, 2)
        } else {
            (types::I8, 1)
        };

        let val = builder.ins().load(
            cl_ty,
            MemFlagsData::new(),
            src_ptr,
            curr_offset as i32,
        );
        builder
            .ins()
            .store(MemFlagsData::new(), val, dest_ptr, curr_offset as i32);
        curr_offset += chunk_size;
    }
}

pub fn stack_store_with_ptr_ty(
    builder: &mut FunctionBuilder,
    val: Value,
    slot: StackSlot,
    offset: i32,
    ptr_ty: Type,
) {
    let addr = builder.ins().stack_addr(ptr_ty, slot, offset);
    builder.ins().store(MemFlagsData::new(), val, addr, 0);
}

pub fn copy_to_stack(
    builder: &mut FunctionBuilder,
    src_ptr: Value,
    slot: StackSlot,
    slot_offset: i32,
    size: usize,
    ptr_ty: Type,
) {
    let mut curr_offset = 0;
    while curr_offset < size {
        let bytes_left = size - curr_offset;
        let (cl_ty, chunk_size) = if bytes_left >= 8 {
            (types::I64, 8)
        } else if bytes_left >= 4 {
            (types::I32, 4)
        } else if bytes_left >= 2 {
            (types::I16, 2)
        } else {
            (types::I8, 1)
        };

        let val = builder.ins().load(
            cl_ty,
            MemFlagsData::new(),
            src_ptr,
            curr_offset as i32,
        );
        stack_store_with_ptr_ty(
            builder,
            val,
            slot,
            slot_offset + curr_offset as i32,
            ptr_ty,
        );
        curr_offset += chunk_size;
    }
}
