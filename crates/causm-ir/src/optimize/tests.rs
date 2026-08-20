#[cfg(test)]
mod test {
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

    #[test]
    fn test_cfg_simplification_pass_unreachable() {
        use crate::optimize::cfg_simp::CfgSimplificationPass;

        let instrs = vec![
            Instruction::LoadBool {
                dest: Reg(0),
                value: true,
            },
            Instruction::JumpIf {
                cond: Reg(0),
                target: 4,
            },
            Instruction::LoadInt {
                dest: Reg(1),
                value: 10,
            },
            Instruction::Return { src: Some(Reg(1)) },
            Instruction::LoadInt {
                dest: Reg(2),
                value: 20,
            },
            Instruction::Return { src: Some(Reg(2)) },
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = SsaTransformer::new(cfg);
        let mut ssa_cfg = transformer.transform();

        assert_eq!(ssa_cfg.blocks.len(), 3);

        let pass = CfgSimplificationPass;
        let empty_used = HashSet::new();
        let changed = pass.run(&mut ssa_cfg, &empty_used, false);

        assert!(changed);
        assert_eq!(ssa_cfg.blocks.len(), 2);
    }

    #[test]
    fn test_channel_liveness_pass() {
        use crate::optimize::channels::ChannelLivenessPass;

        let instrs = vec![
            Instruction::OpenChan {
                name: "dead_chan".to_string(),
                capacity: 10,
                decay_after_ms: None,
            },
            Instruction::OpenChan {
                name: "live_chan".to_string(),
                capacity: 10,
                decay_after_ms: None,
            },
            Instruction::ChanSend {
                chan_id: "live_chan".to_string(),
                src: Reg(0),
            },
            Instruction::Return { src: None },
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = SsaTransformer::new(cfg);
        let mut ssa_cfg = transformer.transform();

        let pass = ChannelLivenessPass::new(HashSet::new());
        let empty_used = HashSet::new();
        let changed = pass.run(&mut ssa_cfg, &empty_used, false);

        assert!(changed);
        let block0 = ssa_cfg.blocks.get(&0).unwrap();
        let remaining_open_chans: Vec<_> = block0
            .instructions
            .iter()
            .filter_map(|i| match i {
                SsaInstruction::OpenChan { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(remaining_open_chans, vec!["live_chan"]);
    }

    #[test]
    fn test_verifier_pass_unclosed_temporal_block() {
        use crate::optimize::verifier::{verify_ssa_cfg, VerificationError};

        let instrs = vec![
            Instruction::While { max_ms: 100 },
            Instruction::LoadInt {
                dest: Reg(0),
                value: 42,
            },
            Instruction::Return { src: Some(Reg(0)) },
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = SsaTransformer::new(cfg);
        let ssa_cfg = transformer.transform();

        let res = verify_ssa_cfg(&ssa_cfg);
        assert!(res.is_err());
        let errs = res.unwrap_err();
        assert!(errs.iter().any(|e| matches!(
            e,
            VerificationError::UnbalancedTemporalBlock { .. }
        )));
    }

    #[test]
    fn test_lease_optimization_pass() {
        use crate::optimize::lease::LeaseOptimizationPass;

        let instrs = vec![
            Instruction::Lease {
                target_reg: Reg(1),
                source_reg: Reg(0),
                duration_ms: 0,
            },
            Instruction::EndLease {
                source_reg: Reg(0),
                duration_ms: 0,
            },
            Instruction::Return { src: None },
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = SsaTransformer::new(cfg);
        let mut ssa_cfg = transformer.transform();

        let pass = LeaseOptimizationPass;
        let empty_used = HashSet::new();
        let changed = pass.run(&mut ssa_cfg, &empty_used, false);

        assert!(changed);
        let block0 = ssa_cfg.blocks.get(&0).unwrap();
        assert!(block0.instructions.is_empty());
    }

    #[test]
    fn test_concurrency_analysis_unmerged_branch() {
        use crate::optimize::concurrency::{
            analyze_concurrency, ConcurrencyAnalysisPass, ConcurrencyError,
        };

        let instrs = vec![
            Instruction::Split {
                parent: "main".to_string(),
                branches: vec!["b1".to_string(), "b2".to_string()],
            },
            Instruction::Merge {
                branches: vec!["b1".to_string()],
                target: "main".to_string(),
                resolution: causm_core::MergeResolution {
                    rules: std::collections::HashMap::new(),
                    auto: true,
                    fallback: None,
                    taking_ms: None,
                },
            },
            Instruction::Return { src: None },
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = SsaTransformer::new(cfg);
        let mut ssa_cfg = transformer.transform();

        let pass = ConcurrencyAnalysisPass;
        let empty_used = HashSet::new();
        let _ = pass.run(&mut ssa_cfg, &empty_used, false);

        let res = analyze_concurrency(&ssa_cfg);
        assert!(res.is_err());
        let errs = res.unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ConcurrencyError::UnmergedBranch { ref branch, .. } if branch == "b2")));
    }

    #[test]
    fn test_dead_routine_tree_shaking_pass() {
        use crate::optimize::dead_code::prune_unreachable_routines;
        use crate::{IrBlock, IrProgram, IrRoutine};
        use std::collections::HashMap;

        let mut routines = HashMap::new();
        routines.insert(
            "used_fn".to_string(),
            IrRoutine {
                params: Vec::new(),
                return_type: causm_core::types::Type::Integer,
                taking_ms: None,
                foreign_binding: None,
                instructions: vec![Instruction::Return { src: None }],
                spans: vec![None],
            },
        );
        routines.insert(
            "unused_fn".to_string(),
            IrRoutine {
                params: Vec::new(),
                return_type: causm_core::types::Type::Integer,
                taking_ms: None,
                foreign_binding: None,
                instructions: vec![Instruction::Return { src: None }],
                spans: vec![None],
            },
        );

        let mut ir = IrProgram {
            blocks: vec![IrBlock {
                time: causm_core::TimeCoordinate::Global(0),
                entropy_mode: Some(causm_core::EntropyMode::Deterministic),
                instructions: vec![Instruction::Call {
                    routine: "used_fn".to_string(),
                    args: vec![],
                    dest: Reg(0),
                }],
                spans: vec![None],
            }],
            routines,
            symbols: HashMap::new(),
            type_decay_limits: HashMap::new(),
            auto_drop_specs: HashMap::new(),
            struct_extends: HashMap::new(),
            decay_handlers: HashMap::new(),
        };

        prune_unreachable_routines(&mut ir);

        assert!(ir.routines.contains_key("used_fn"));
        assert!(!ir.routines.contains_key("unused_fn"));
    }
}
