pub mod cache;
pub mod context;
pub mod intrinsics;
pub mod lower;
pub mod memory;
pub mod ssa_lower;
pub mod storage;
pub mod strings;
pub mod timing;
pub mod tuples;

pub use context::{CausmJit, JitError};

#[cfg(test)]
mod tests {
    use super::*;
    use causm_core::types::Type;
    use causm_core::{BinaryOperator, ParamMode};
    use causm_ir::{Instruction, IrRoutine, Reg};

    #[test]
    fn test_jit_arithmetic_add_and_mul_execution() {
        let mut jit = CausmJit::new().expect("JIT initialization failed");

        // routine compute(a: i64, b: i64) -> i64 { return (a + b) * 2; }
        let routine = IrRoutine {
            params: vec![
                (ParamMode::Consume, "a".to_string(), Type::Integer),
                (ParamMode::Consume, "b".to_string(), Type::Integer),
            ],
            return_type: Type::Integer,
            taking_ms: None,
            foreign_binding: None,
            instructions: vec![
                Instruction::BinaryOp {
                    dest: Reg(2),
                    op: BinaryOperator::Add,
                    left: Reg(0),
                    right: Reg(1),
                },
                Instruction::ConstInt {
                    dest: Reg(3),
                    value: 2,
                },
                Instruction::BinaryOp {
                    dest: Reg(4),
                    op: BinaryOperator::Mul,
                    left: Reg(2),
                    right: Reg(3),
                },
                Instruction::Return { src: Some(Reg(4)) },
            ],
            spans: vec![None, None, None, None],
        };

        let func_ptr = jit
            .compile_routine("compute", &routine)
            .expect("Routine compilation failed");

        let func: extern "C" fn(i64, i64) -> i64 =
            unsafe { std::mem::transmute(func_ptr) };

        assert_eq!(func(10, 20), 60);
        assert_eq!(func(-5, 15), 20);
    }

    #[test]
    fn test_jit_memory_array_push_and_index_access() {
        let mut jit = CausmJit::new().expect("JIT initialization failed");

        // routine make_and_read() -> i64 { let arr = [100, 200]; return arr[1]; }
        let routine = IrRoutine {
            params: vec![],
            return_type: Type::Integer,
            taking_ms: None,
            foreign_binding: None,
            instructions: vec![
                Instruction::ConstInt {
                    dest: Reg(0),
                    value: 100,
                },
                Instruction::ConstInt {
                    dest: Reg(1),
                    value: 200,
                },
                Instruction::ArrayLit {
                    dest: Reg(2),
                    elements: vec![Reg(0), Reg(1)],
                },
                Instruction::ConstInt {
                    dest: Reg(3),
                    value: 1,
                },
                Instruction::IndexAccess {
                    dest: Reg(4),
                    target: Reg(2),
                    index: Reg(3),
                },
                Instruction::Return { src: Some(Reg(4)) },
            ],
            spans: vec![None, None, None, None, None, None],
        };

        let func_ptr = jit
            .compile_routine("make_and_read", &routine)
            .expect("Compilation failed");

        let func: extern "C" fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };

        assert_eq!(func(), 200);
    }

    #[test]
    fn test_jit_tuples_stack_slot_allocation_and_extraction() {
        let mut jit = CausmJit::new().expect("JIT initialization failed");

        // routine tuple_test() -> i64 { let tup = (42, 99); return tup.0; }
        let routine = IrRoutine {
            params: vec![],
            return_type: Type::Integer,
            taking_ms: None,
            foreign_binding: None,
            instructions: vec![
                Instruction::ConstInt {
                    dest: Reg(0),
                    value: 42,
                },
                Instruction::ConstInt {
                    dest: Reg(1),
                    value: 99,
                },
                Instruction::TupleLit {
                    dest: Reg(2),
                    elems: vec![Reg(0), Reg(1)],
                },
                Instruction::TupleAccess {
                    dest: Reg(3),
                    tuple: Reg(2),
                    index: 0,
                },
                Instruction::Return { src: Some(Reg(3)) },
            ],
            spans: vec![None, None, None, None, None],
        };

        let func_ptr = jit
            .compile_routine("tuple_test", &routine)
            .expect("Compilation failed");

        let func: extern "C" fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };

        assert_eq!(func(), 42);
    }

    #[test]
    fn test_jit_timing_read_tsc_progression() {
        let tsc1 = timing::read_tsc();
        timing::spin_pad(1000);
        let tsc2 = timing::read_tsc();
        assert!(tsc2 >= tsc1);
    }

    #[test]
    fn test_ssa_branch_phi_merge_jit_execution() {
        let mut jit = CausmJit::new().expect("JIT initialization failed");

        // routine select_branch(cond: i64) -> i64 {
        //   if cond > 0 { return 200; } else { return 100; }
        // }
        // pc 0: JumpIf R0 -> pc 3
        // pc 1: LoadInt R1 = 100
        // pc 2: Jump -> pc 4
        // pc 3: LoadInt R1 = 200
        // pc 4: Return R1
        let routine = IrRoutine {
            params: vec![(ParamMode::Consume, "cond".to_string(), Type::Integer)],
            return_type: Type::Integer,
            taking_ms: None,
            foreign_binding: None,
            instructions: vec![
                Instruction::JumpIf {
                    cond: Reg(0),
                    target: 3,
                },
                Instruction::LoadInt {
                    dest: Reg(1),
                    value: 100,
                },
                Instruction::Jump { target: 4 },
                Instruction::LoadInt {
                    dest: Reg(1),
                    value: 200,
                },
                Instruction::Return { src: Some(Reg(1)) },
            ],
            spans: vec![None, None, None, None, None],
        };

        let func_ptr = jit
            .compile_routine("select_branch", &routine)
            .expect("Compilation failed");

        let func: extern "C" fn(i64) -> i64 =
            unsafe { std::mem::transmute(func_ptr) };

        assert_eq!(func(1), 200);
        assert_eq!(func(0), 100);
    }

    #[test]
    fn test_ssa_loop_counter_accumulator_jit_execution() {
        let mut jit = CausmJit::new().expect("JIT initialization failed");

        // routine sum_loop(n: i64) -> i64:
        // i = 0 (R1)
        // acc = 0 (R2)
        // loop_header: (pc 2)
        // cond = i < n (R3 = R1 < R0)
        // JumpIfNot cond -> end (pc 9)
        // acc = acc + i (R2 = R2 + R1)
        // i = i + 1 (R1 = R1 + 1)
        // Jump -> loop_header (pc 2)
        // end: (pc 9)
        // Return acc (R2)
        let routine = IrRoutine {
            params: vec![(ParamMode::Consume, "n".to_string(), Type::Integer)],
            return_type: Type::Integer,
            taking_ms: None,
            foreign_binding: None,
            instructions: vec![
                Instruction::ConstInt {
                    dest: Reg(1),
                    value: 0,
                },
                Instruction::ConstInt {
                    dest: Reg(2),
                    value: 0,
                },
                // pc 2: header
                Instruction::BinaryOp {
                    dest: Reg(3),
                    op: BinaryOperator::Lt,
                    left: Reg(1),
                    right: Reg(0),
                },
                Instruction::JumpIfNot {
                    cond: Reg(3),
                    target: 8,
                },
                Instruction::BinaryOp {
                    dest: Reg(2),
                    op: BinaryOperator::Add,
                    left: Reg(2),
                    right: Reg(1),
                },
                Instruction::ConstInt {
                    dest: Reg(4),
                    value: 1,
                },
                Instruction::BinaryOp {
                    dest: Reg(1),
                    op: BinaryOperator::Add,
                    left: Reg(1),
                    right: Reg(4),
                },
                Instruction::Jump { target: 2 },
                // pc 8: end
                Instruction::Return { src: Some(Reg(2)) },
            ],
            spans: vec![None, None, None, None, None, None, None, None, None],
        };

        let func_ptr = jit
            .compile_routine("sum_loop", &routine)
            .expect("Compilation failed");

        let func: extern "C" fn(i64) -> i64 =
            unsafe { std::mem::transmute(func_ptr) };

        // sum(0..10) = 45
        assert_eq!(func(10), 45);
        // sum(0..100) = 4950
        assert_eq!(func(100), 4950);
        // sum(0..0) = 0
        assert_eq!(func(0), 0);
    }

    #[test]
    fn test_jit_string_literal_and_slicing() {
        let mut jit = CausmJit::new().expect("JIT initialization failed");

        // routine test_str() -> CausmString {
        //   let s = "Hello, Causm!";
        //   let sub = s[7..12]; // "Causm"
        //   return sub;
        // }
        let routine = IrRoutine {
            params: vec![],
            return_type: Type::String,
            taking_ms: None,
            foreign_binding: None,
            instructions: vec![
                Instruction::ConstString {
                    dest: Reg(0),
                    value: "Hello, Causm!".to_string(),
                },
                Instruction::ConstInt {
                    dest: Reg(1),
                    value: 7,
                },
                Instruction::ConstInt {
                    dest: Reg(2),
                    value: 12,
                },
                Instruction::ArraySlice {
                    dest: Reg(3),
                    target: Reg(0),
                    start: Some(Reg(1)),
                    end: Some(Reg(2)),
                    inclusive: false,
                },
                Instruction::Return { src: Some(Reg(3)) },
            ],
            spans: vec![None, None, None, None, None],
        };

        let func_ptr = jit
            .compile_routine("test_str", &routine)
            .expect("Compilation failed");

        let func: extern "C" fn() -> *const crate::strings::CausmString =
            unsafe { std::mem::transmute(func_ptr) };

        let res_ptr = func();
        let res_str = unsafe { &*res_ptr };
        assert_eq!(res_str.as_str(), "Causm");
    }

    #[test]
    fn test_ssa_direct_cfg_compilation() {
        use causm_ir::cfg::BlockId;
        use causm_ir::ssa::{
            SsaBasicBlock, SsaCFG, SsaInstruction, SsaPhiNode, SsaReg, SsaTerminator,
        };
        use std::collections::HashMap;

        let mut jit = CausmJit::new().expect("JIT initialization failed");

        let mut blocks: HashMap<BlockId, SsaBasicBlock> = HashMap::new();

        // Block 0 (entry): R0_1 = 40; jump Block 1
        blocks.insert(
            0,
            SsaBasicBlock {
                id: 0,
                phi_nodes: vec![],
                instructions: vec![SsaInstruction::ConstInt {
                    dest: SsaReg { reg: 0, version: 1 },
                    value: 40,
                }],
                terminator: SsaTerminator::Jump { target: 1 },
            },
        );

        // Block 1: phi R0_2 = [(Block 0, R0_1)]; R1_1 = 2; R2_1 = R0_2 + R1_1; return R2_1
        blocks.insert(
            1,
            SsaBasicBlock {
                id: 1,
                phi_nodes: vec![SsaPhiNode {
                    dest: SsaReg { reg: 0, version: 2 },
                    original_reg: Reg(0),
                    incoming: vec![(0, SsaReg { reg: 0, version: 1 })],
                }],
                instructions: vec![
                    SsaInstruction::ConstInt {
                        dest: SsaReg { reg: 1, version: 1 },
                        value: 2,
                    },
                    SsaInstruction::BinaryOp {
                        dest: SsaReg { reg: 2, version: 1 },
                        op: BinaryOperator::Add,
                        left: SsaReg { reg: 0, version: 2 },
                        right: SsaReg { reg: 1, version: 1 },
                    },
                ],
                terminator: SsaTerminator::Return {
                    src: Some(SsaReg { reg: 2, version: 1 }),
                },
            },
        );

        let ssa_cfg = SsaCFG {
            entry_block: 0,
            blocks,
            original_pc_to_block_id: HashMap::new(),
        };

        let func_ptr = jit
            .compile_ssa("direct_ssa", &ssa_cfg, 0)
            .expect("Direct SSA compilation failed");

        let func: extern "C" fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
        assert_eq!(func(), 42);
    }

    #[test]
    fn test_jit_cache_hit_identical_routine() {
        let mut jit = CausmJit::new().expect("JIT initialization failed");

        let routine = IrRoutine {
            params: vec![(ParamMode::Consume, "x".to_string(), Type::Integer)],
            return_type: Type::Integer,
            taking_ms: None,
            foreign_binding: None,
            instructions: vec![
                Instruction::ConstInt {
                    dest: Reg(1),
                    value: 777,
                },
                Instruction::Return { src: Some(Reg(1)) },
            ],
            spans: vec![None, None],
        };

        let ptr1 = jit.compile_routine("cached_rt", &routine).unwrap();
        let ptr2 = jit.compile_routine("cached_rt", &routine).unwrap();

        // Exact pointer match from cache
        assert_eq!(ptr1, ptr2);

        let func: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(ptr2) };
        assert_eq!(func(0), 777);
    }
}
