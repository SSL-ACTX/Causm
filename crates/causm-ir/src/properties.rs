use crate::ssa::{SsaInstruction, SsaReg};
use crate::Reg;

pub trait InstructionProperties {
    fn defined_reg(&self) -> Option<Reg>;
    fn used_regs(&self) -> Vec<Reg>;
    fn consumed_regs(&self) -> Vec<Reg>;
    fn decayed_regs(&self) -> Vec<Reg>;
}

pub trait SsaInstructionProperties {
    fn defined_ssa_reg(&self) -> Option<SsaReg>;
    fn used_ssa_regs(&self) -> Vec<SsaReg>;
    fn consumed_ssa_regs(&self) -> Vec<SsaReg>;
    fn decayed_ssa_regs(&self) -> Vec<SsaReg>;
}

impl SsaInstructionProperties for SsaInstruction {
    fn defined_ssa_reg(&self) -> Option<SsaReg> {
        match self {
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
            | SsaInstruction::StrBytes { dest, .. }
            | SsaInstruction::ToStr { dest, .. }
            | SsaInstruction::ArrayLen { dest, .. }
            | SsaInstruction::Call { dest, .. }
            | SsaInstruction::DynamicCall { dest, .. }
            | SsaInstruction::TypeAssert { dest, .. }
            | SsaInstruction::TypeCast { dest, .. }
            | SsaInstruction::TryTypeAssert { dest, .. }
            | SsaInstruction::StructLit { dest, .. }
            | SsaInstruction::TopologyLit { dest, .. }
            | SsaInstruction::ArrayLit { dest, .. }
            | SsaInstruction::ArrayRepeat { dest, .. }
            | SsaInstruction::ArraySlice { dest, .. }
            | SsaInstruction::FieldAccess { dest, .. }
            | SsaInstruction::IndexAccess { dest, .. }
            | SsaInstruction::ChanRecv { dest, .. }
            | SsaInstruction::ConditionalSelect { dest, .. }
            | SsaInstruction::Defer { dest, .. }
            | SsaInstruction::Syscall { dest, .. }
            | SsaInstruction::Lease {
                target_reg: dest, ..
            } => Some(*dest),

            SsaInstruction::For { dest_cond, .. }
            | SsaInstruction::ForStep { dest_cond, .. } => Some(*dest_cond),

            SsaInstruction::SplitMap { item_reg, .. } => Some(*item_reg),

            SsaInstruction::FieldUpdate { target, .. }
            | SsaInstruction::IndexFieldUpdate { target, .. } => Some(*target),

            _ => None,
        }
    }

    fn used_ssa_regs(&self) -> Vec<SsaReg> {
        let mut regs = Vec::new();
        match self {
            SsaInstruction::BinaryOp { left, right, .. } => {
                regs.push(*left);
                regs.push(*right);
            }
            SsaInstruction::ConditionalSelect {
                cond,
                true_val,
                false_val,
                ..
            } => {
                regs.push(*cond);
                regs.push(*true_val);
                regs.push(*false_val);
            }
            SsaInstruction::UnaryOp { src, .. }
            | SsaInstruction::Move { src, .. }
            | SsaInstruction::Clone { src, .. }
            | SsaInstruction::StrBytes { src, .. }
            | SsaInstruction::ToStr { src, .. }
            | SsaInstruction::ArrayLen { src, .. }
            | SsaInstruction::TypeAssert { src, .. }
            | SsaInstruction::TypeCast { src, .. }
            | SsaInstruction::TryTypeAssert { src, .. }
            | SsaInstruction::AssertState { src, .. }
            | SsaInstruction::Print { src }
            | SsaInstruction::Debug { src }
            | SsaInstruction::Consume { src }
            | SsaInstruction::ConsumeField { src, .. }
            | SsaInstruction::ChanSend { src, .. }
            | SsaInstruction::JumpIf { cond: src, .. }
            | SsaInstruction::JumpIfNot { cond: src, .. }
            | SsaInstruction::Await { target: src }
            | SsaInstruction::MatchEntropy { target: src, .. }
            | SsaInstruction::Lease {
                source_reg: src, ..
            }
            | SsaInstruction::EndLease {
                source_reg: src, ..
            }
            | SsaInstruction::AutoDrop { target: src, .. } => {
                regs.push(*src);
            }
            SsaInstruction::ConsumeFieldDynamic { target, index }
            | SsaInstruction::IndexAccess { target, index, .. } => {
                regs.push(*target);
                regs.push(*index);
            }
            SsaInstruction::FieldAccess { target, .. } => {
                regs.push(*target);
            }
            SsaInstruction::FieldUpdate {
                old_target, src, ..
            } => {
                regs.push(*old_target);
                regs.push(*src);
            }
            SsaInstruction::IndexFieldUpdate {
                old_target,
                index,
                src,
                ..
            } => {
                regs.push(*old_target);
                regs.push(*index);
                regs.push(*src);
            }
            SsaInstruction::For { source, .. }
            | SsaInstruction::ForStep { source, .. }
            | SsaInstruction::SplitMap { source, .. } => {
                regs.push(*source);
            }
            SsaInstruction::Call { args, .. } => {
                regs.extend(args.iter().cloned());
            }
            SsaInstruction::DynamicCall { args, .. } => {
                regs.extend(args.iter().cloned());
            }
            SsaInstruction::Syscall { args, .. } => {
                regs.extend(args.iter().cloned());
            }
            SsaInstruction::StructLit { fields, .. }
            | SsaInstruction::TopologyLit { fields, .. } => {
                let mut sorted_keys: Vec<_> = fields.keys().collect();
                sorted_keys.sort();
                for k in sorted_keys {
                    regs.push(fields[k]);
                }
            }
            SsaInstruction::ArrayLit { elements, .. } => {
                regs.extend(elements.iter().cloned());
            }
            SsaInstruction::ArrayRepeat { value, count, .. } => {
                regs.push(*value);
                regs.push(*count);
            }
            SsaInstruction::ArraySlice {
                target, start, end, ..
            } => {
                regs.push(*target);
                if let Some(s) = start {
                    regs.push(*s);
                }
                if let Some(e) = end {
                    regs.push(*e);
                }
            }
            SsaInstruction::Entangle { regs: r_vec } => {
                regs.extend(r_vec.iter().cloned());
            }
            _ => {}
        }
        regs
    }

    fn consumed_ssa_regs(&self) -> Vec<SsaReg> {
        let mut regs = Vec::new();
        match self {
            SsaInstruction::Consume { src }
            | SsaInstruction::ConsumeField { src, .. }
            | SsaInstruction::ChanSend { src, .. } => {
                regs.push(*src);
            }
            SsaInstruction::ConsumeFieldDynamic { target, .. }
            | SsaInstruction::FieldUpdate { target, .. }
            | SsaInstruction::IndexFieldUpdate { target, .. } => {
                regs.push(*target);
            }
            SsaInstruction::For { source, .. }
            | SsaInstruction::ForStep { source, .. }
            | SsaInstruction::SplitMap { source, .. } => {
                regs.push(*source);
            }
            SsaInstruction::Call { args, .. } => {
                regs.extend(args.iter().cloned());
            }
            SsaInstruction::DynamicCall { args, .. } => {
                regs.extend(args.iter().cloned());
            }
            _ => {}
        }
        regs
    }

    fn decayed_ssa_regs(&self) -> Vec<SsaReg> {
        let mut regs = Vec::new();
        match self {
            SsaInstruction::FieldAccess { target, .. }
            | SsaInstruction::IndexAccess { target, .. } => {
                regs.push(*target);
            }
            _ => {}
        }
        regs
    }
}
