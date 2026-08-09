use crate::ssa::{SsaInstruction, SsaReg, SsaTerminator};

pub(crate) fn get_ssa_dest_reg(instr: &SsaInstruction) -> Option<SsaReg> {
    match instr {
        SsaInstruction::BinaryOp { dest, .. }
        | SsaInstruction::UnaryOp { dest, .. }
        | SsaInstruction::LoadInt { dest, .. }
        | SsaInstruction::LoadFloat { dest, .. }
        | SsaInstruction::LoadBool { dest, .. }
        | SsaInstruction::LoadString { dest, .. }
        | SsaInstruction::LoadNull { dest }
        | SsaInstruction::ConstInt { dest, .. }
        | SsaInstruction::ConstFloat { dest, .. }
        | SsaInstruction::ConstBool { dest, .. }
        | SsaInstruction::ConstString { dest, .. }
        | SsaInstruction::ConstNull { dest }
        | SsaInstruction::Move { dest, .. }
        | SsaInstruction::Clone { dest, .. }
        | SsaInstruction::Call { dest, .. }
        | SsaInstruction::DynamicCall { dest, .. }
        | SsaInstruction::StructLit { dest, .. }
        | SsaInstruction::TopologyLit { dest, .. }
        | SsaInstruction::ArrayLit { dest, .. }
        | SsaInstruction::FieldAccess { dest, .. }
        | SsaInstruction::IndexAccess { dest, .. }
        | SsaInstruction::TypeCast { dest, .. }
        | SsaInstruction::Defer { dest, .. } => Some(*dest),

        SsaInstruction::FieldUpdate { target, .. }
        | SsaInstruction::IndexFieldUpdate { target, .. } => Some(*target),

        _ => None,
    }
}

pub(crate) fn for_each_ssa_src_reg(
    instr: &SsaInstruction,
    f: &mut impl FnMut(SsaReg),
) {
    match instr {
        SsaInstruction::BinaryOp { left, right, .. } => {
            f(*left);
            f(*right);
        }
        SsaInstruction::UnaryOp { src, .. } => {
            f(*src);
        }
        SsaInstruction::Move { src, .. } => {
            f(*src);
        }
        SsaInstruction::Consume { src } => {
            f(*src);
        }
        SsaInstruction::ConsumeField { src, .. } => {
            f(*src);
        }
        SsaInstruction::ConsumeFieldDynamic { target, index } => {
            f(*target);
            f(*index);
        }
        SsaInstruction::Clone { src, .. } => {
            f(*src);
        }
        SsaInstruction::Call { args, .. } => {
            for a in args {
                f(*a);
            }
        }
        SsaInstruction::DynamicCall { args, .. } => {
            for a in args {
                f(*a);
            }
        }
        SsaInstruction::StructLit { fields, .. } => {
            for v in fields.values() {
                f(*v);
            }
        }
        SsaInstruction::TopologyLit { fields, .. } => {
            for v in fields.values() {
                f(*v);
            }
        }
        SsaInstruction::ArrayLit { elements, .. } => {
            for e in elements {
                f(*e);
            }
        }
        SsaInstruction::FieldAccess { target, .. } => {
            f(*target);
        }
        SsaInstruction::FieldUpdate {
            old_target, src, ..
        } => {
            f(*old_target);
            f(*src);
        }
        SsaInstruction::IndexAccess { target, index, .. } => {
            f(*target);
            f(*index);
        }
        SsaInstruction::TypeCast { src, .. } => {
            f(*src);
        }
        SsaInstruction::IndexFieldUpdate {
            old_target,
            index,
            src,
            ..
        } => {
            f(*old_target);
            f(*index);
            f(*src);
        }
        SsaInstruction::For { source, .. } => {
            f(*source);
        }
        SsaInstruction::ForStep { source, .. } => {
            f(*source);
        }
        SsaInstruction::SplitMap { source, .. } => {
            f(*source);
        }
        SsaInstruction::Await { target } => {
            f(*target);
        }
        SsaInstruction::Print { src } => {
            f(*src);
        }
        SsaInstruction::Debug { src } => {
            f(*src);
        }
        SsaInstruction::ChanSend { src, .. } => {
            f(*src);
        }
        SsaInstruction::Lease { source_reg, .. } => {
            f(*source_reg);
        }
        SsaInstruction::EndLease { source_reg, .. } => {
            f(*source_reg);
        }
        SsaInstruction::Entangle { regs } => {
            for &r in regs {
                f(r);
            }
        }
        SsaInstruction::MatchEntropy { target, .. } => {
            f(*target);
        }
        _ => {}
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
