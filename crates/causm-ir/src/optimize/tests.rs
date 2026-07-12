#[cfg(test)]
mod tests {
    use crate::cfg::CFG;
    use crate::optimize::coalescing::BlockCoalescingPass;
    use crate::optimize::constant_prop::ConstantPropagationPass;
    use crate::optimize::OptimizationPass;
    use crate::ssa::{SsaInstruction, SsaTerminator, SsaTransformer};
    use crate::{Instruction, Reg};
    use std::collections::HashSet;

    #[test]
    fn test_block_coalescing_pass() {
        let instrs = vec![
            Instruction::LoadInt {
                dest: Reg(0),
                value: 5,
            },
            Instruction::Jump { target: 2 },
            Instruction::LoadInt {
                dest: Reg(1),
                value: 10,
            },
            Instruction::Return { src: Some(Reg(1)) },
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = SsaTransformer::new(cfg);
        let mut ssa_cfg = transformer.transform();

        let pass = BlockCoalescingPass;
        let empty_used = HashSet::new();
        let changed = pass.run(&mut ssa_cfg, &empty_used, false);

        assert!(changed);
        assert_eq!(ssa_cfg.blocks.len(), 1);
        let block0 = ssa_cfg.blocks.get(&0).unwrap();
        assert_eq!(block0.instructions.len(), 2);

        match &block0.terminator {
            SsaTerminator::Return { src } => {
                assert!(src.is_some());
            }
            _ => panic!("Expected return terminator"),
        }
    }

    #[test]
    fn test_constant_propagation_pass() {
        let instrs = vec![
            Instruction::LoadInt {
                dest: Reg(0),
                value: 5,
            },
            Instruction::LoadInt {
                dest: Reg(1),
                value: 10,
            },
            Instruction::BinaryOp {
                dest: Reg(2),
                op: causm_core::BinaryOperator::Add,
                left: Reg(0),
                right: Reg(1),
            },
            Instruction::Return { src: Some(Reg(2)) },
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = SsaTransformer::new(cfg);
        let mut ssa_cfg = transformer.transform();

        let pass = ConstantPropagationPass;
        let empty_used = HashSet::new();
        let changed = pass.run(&mut ssa_cfg, &empty_used, false);

        assert!(changed);
        let block0 = ssa_cfg.blocks.get(&0).unwrap();

        let found_load_15 = block0
            .instructions
            .iter()
            .any(|inst| matches!(inst, SsaInstruction::LoadInt { value: 15, .. }));
        assert!(
            found_load_15,
            "Expected folded instruction LoadInt with value 15"
        );
    }
}
