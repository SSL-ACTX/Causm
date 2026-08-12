pub mod display;
pub mod transformer;
pub mod types;

pub use transformer::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Instruction, Reg, CFG};
    use std::collections::HashSet;

    #[test]
    fn test_ssa_renaming_and_phi() {
        let instrs = vec![
            Instruction::LoadInt {
                dest: Reg(0),
                value: 5,
            },
            Instruction::JumpIf {
                cond: Reg(0),
                target: 4,
            },
            Instruction::LoadInt {
                dest: Reg(1),
                value: 10,
            },
            Instruction::Jump { target: 5 },
            Instruction::LoadInt {
                dest: Reg(1),
                value: 20,
            },
            Instruction::BinaryOp {
                dest: Reg(2),
                op: causm_core::BinaryOperator::Add,
                left: Reg(1),
                right: Reg(0),
            },
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = SsaTransformer::new(cfg);
        let ssa_cfg = transformer.transform();

        let b3 = ssa_cfg.blocks.get(&3).unwrap();
        assert_eq!(b3.phi_nodes.len(), 1);
        let phi = &b3.phi_nodes[0];
        assert_eq!(phi.original_reg, Reg(1));
        assert_eq!(phi.dest, SsaReg { reg: 1, version: 3 });

        let incoming: HashSet<(crate::cfg::BlockId, SsaReg)> =
            phi.incoming.iter().cloned().collect();
        assert!(incoming.contains(&(1, SsaReg { reg: 1, version: 1 })));
        assert!(incoming.contains(&(2, SsaReg { reg: 1, version: 2 })));
    }
}
