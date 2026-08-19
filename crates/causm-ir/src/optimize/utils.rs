use crate::properties::SsaInstructionProperties;
use crate::ssa::{SsaInstruction, SsaReg, SsaTerminator};

pub(crate) fn get_ssa_dest_reg(instr: &SsaInstruction) -> Option<SsaReg> {
    instr.defined_ssa_reg()
}

pub(crate) fn for_each_ssa_src_reg(
    instr: &SsaInstruction,
    f: &mut impl FnMut(SsaReg),
) {
    for reg in instr.used_ssa_regs() {
        f(reg);
    }
}

pub(crate) fn for_each_ssa_term_src_reg(
    term: &SsaTerminator,
    f: &mut impl FnMut(SsaReg),
) {
    match term {
        SsaTerminator::Branch { cond, .. } => {
            f(*cond);
        }
        SsaTerminator::Return { src: Some(r) } => {
            f(*r);
        }
        SsaTerminator::MatchEntropy { target, .. } => {
            f(*target);
        }
        _ => {}
    }
}

pub(crate) fn has_side_effects(instr: &SsaInstruction) -> bool {
    !matches!(
        instr,
        SsaInstruction::BinaryOp { .. }
            | SsaInstruction::UnaryOp { .. }
            | SsaInstruction::LoadInt { .. }
            | SsaInstruction::LoadFloat { .. }
            | SsaInstruction::LoadBool { .. }
            | SsaInstruction::LoadString { .. }
            | SsaInstruction::LoadNull { .. }
            | SsaInstruction::ConstInt { .. }
            | SsaInstruction::ConstFloat { .. }
            | SsaInstruction::ConstBool { .. }
            | SsaInstruction::ConstString { .. }
            | SsaInstruction::ConstNull { .. }
            | SsaInstruction::Move { .. }
            | SsaInstruction::ConditionalSelect { .. }
            | SsaInstruction::Clone { .. }
            | SsaInstruction::StructLit { .. }
            | SsaInstruction::TopologyLit { .. }
            | SsaInstruction::ArrayLit { .. }
            | SsaInstruction::FieldAccess { .. }
            | SsaInstruction::IndexAccess { .. }
            | SsaInstruction::FieldUpdate { .. }
            | SsaInstruction::IndexFieldUpdate { .. }
            | SsaInstruction::TypeCast { .. }
    )
}
