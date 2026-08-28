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
            | SsaInstruction::TryEnumVariant { dest, .. }
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
            | SsaInstruction::ArenaIntrospect { dest, .. }
            | SsaInstruction::CapabilityCheck { dest, .. }
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
            | SsaInstruction::TryEnumVariant { src, .. }
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

impl InstructionProperties for crate::Instruction {
    fn defined_reg(&self) -> Option<Reg> {
        match self {
            crate::Instruction::BinaryOp { dest, .. }
            | crate::Instruction::UnaryOp { dest, .. }
            | crate::Instruction::LoadInt { dest, .. }
            | crate::Instruction::LoadFloat { dest, .. }
            | crate::Instruction::LoadBool { dest, .. }
            | crate::Instruction::LoadString { dest, .. }
            | crate::Instruction::LoadNull { dest }
            | crate::Instruction::ConstInt { dest, .. }
            | crate::Instruction::ConstFloat { dest, .. }
            | crate::Instruction::ConstBool { dest, .. }
            | crate::Instruction::ConstString { dest, .. }
            | crate::Instruction::ConstNull { dest }
            | crate::Instruction::Move { dest, .. }
            | crate::Instruction::Clone { dest, .. }
            | crate::Instruction::StrBytes { dest, .. }
            | crate::Instruction::ToStr { dest, .. }
            | crate::Instruction::ArrayLen { dest, .. }
            | crate::Instruction::Call { dest, .. }
            | crate::Instruction::DynamicCall { dest, .. }
            | crate::Instruction::TypeAssert { dest, .. }
            | crate::Instruction::TypeCast { dest, .. }
            | crate::Instruction::TryTypeAssert { dest, .. }
            | crate::Instruction::TryEnumVariant { dest, .. }
            | crate::Instruction::StructLit { dest, .. }
            | crate::Instruction::TopologyLit { dest, .. }
            | crate::Instruction::ArrayLit { dest, .. }
            | crate::Instruction::ArrayRepeat { dest, .. }
            | crate::Instruction::ArraySlice { dest, .. }
            | crate::Instruction::FieldAccess { dest, .. }
            | crate::Instruction::IndexAccess { dest, .. }
            | crate::Instruction::ChanRecv { dest, .. }
            | crate::Instruction::ConditionalSelect { dest, .. }
            | crate::Instruction::Defer { dest, .. }
            | crate::Instruction::Syscall { dest, .. }
            | crate::Instruction::ArenaIntrospect { dest, .. }
            | crate::Instruction::CapabilityCheck { dest, .. }
            | crate::Instruction::Lease {
                target_reg: dest, ..
            } => Some(*dest),

            crate::Instruction::For { dest_cond, .. }
            | crate::Instruction::ForStep { dest_cond, .. } => Some(*dest_cond),

            crate::Instruction::SplitMap { item_reg, .. } => Some(*item_reg),

            crate::Instruction::FieldUpdate { target, .. }
            | crate::Instruction::IndexFieldUpdate { target, .. } => Some(*target),

            _ => None,
        }
    }

    fn used_regs(&self) -> Vec<Reg> {
        let mut regs = Vec::new();
        match self {
            crate::Instruction::BinaryOp { left, right, .. } => {
                regs.push(*left);
                regs.push(*right);
            }
            crate::Instruction::ConditionalSelect {
                cond,
                true_val,
                false_val,
                ..
            } => {
                regs.push(*cond);
                regs.push(*true_val);
                regs.push(*false_val);
            }
            crate::Instruction::UnaryOp { src, .. }
            | crate::Instruction::Move { src, .. }
            | crate::Instruction::Clone { src, .. }
            | crate::Instruction::StrBytes { src, .. }
            | crate::Instruction::ToStr { src, .. }
            | crate::Instruction::ArrayLen { src, .. }
            | crate::Instruction::TypeAssert { src, .. }
            | crate::Instruction::TypeCast { src, .. }
            | crate::Instruction::TryTypeAssert { src, .. }
            | crate::Instruction::TryEnumVariant { src, .. }
            | crate::Instruction::AssertState { src, .. }
            | crate::Instruction::Print { src }
            | crate::Instruction::Debug { src }
            | crate::Instruction::Consume { src }
            | crate::Instruction::ConsumeField { src, .. }
            | crate::Instruction::ChanSend { src, .. }
            | crate::Instruction::JumpIf { cond: src, .. }
            | crate::Instruction::JumpIfNot { cond: src, .. }
            | crate::Instruction::Await { target: src }
            | crate::Instruction::MatchEntropy { target: src, .. }
            | crate::Instruction::Lease {
                source_reg: src, ..
            }
            | crate::Instruction::EndLease {
                source_reg: src, ..
            }
            | crate::Instruction::AutoDrop { target: src, .. } => {
                regs.push(*src);
            }
            crate::Instruction::ConsumeFieldDynamic { target, index }
            | crate::Instruction::IndexAccess { target, index, .. } => {
                regs.push(*target);
                regs.push(*index);
            }
            crate::Instruction::FieldAccess { target, .. } => {
                regs.push(*target);
            }
            crate::Instruction::FieldUpdate { target, src, .. } => {
                regs.push(*target);
                regs.push(*src);
            }
            crate::Instruction::IndexFieldUpdate {
                target, index, src, ..
            } => {
                regs.push(*target);
                regs.push(*index);
                regs.push(*src);
            }
            crate::Instruction::For { source, .. }
            | crate::Instruction::ForStep { source, .. }
            | crate::Instruction::SplitMap { source, .. } => {
                regs.push(*source);
            }
            crate::Instruction::Call { args, .. } => {
                regs.extend(args.iter().cloned());
            }
            crate::Instruction::DynamicCall { args, .. } => {
                regs.extend(args.iter().cloned());
            }
            crate::Instruction::Syscall { args, .. } => {
                regs.extend(args.iter().cloned());
            }
            crate::Instruction::StructLit { fields, .. }
            | crate::Instruction::TopologyLit { fields, .. } => {
                let mut sorted_keys: Vec<_> = fields.keys().collect();
                sorted_keys.sort();
                for k in sorted_keys {
                    regs.push(fields[k]);
                }
            }
            crate::Instruction::ArrayLit { elements, .. } => {
                regs.extend(elements.iter().cloned());
            }
            crate::Instruction::ArrayRepeat { value, count, .. } => {
                regs.push(*value);
                regs.push(*count);
            }
            crate::Instruction::ArraySlice {
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
            crate::Instruction::Entangle { regs: r_vec } => {
                regs.extend(r_vec.iter().cloned());
            }
            _ => {}
        }
        regs
    }

    fn consumed_regs(&self) -> Vec<Reg> {
        let mut regs = Vec::new();
        match self {
            crate::Instruction::Consume { src }
            | crate::Instruction::ConsumeField { src, .. }
            | crate::Instruction::ChanSend { src, .. } => {
                regs.push(*src);
            }
            crate::Instruction::ConsumeFieldDynamic { target, .. }
            | crate::Instruction::FieldUpdate { target, .. }
            | crate::Instruction::IndexFieldUpdate { target, .. } => {
                regs.push(*target);
            }
            crate::Instruction::For { source, .. }
            | crate::Instruction::ForStep { source, .. }
            | crate::Instruction::SplitMap { source, .. } => {
                regs.push(*source);
            }
            crate::Instruction::Call { args, .. } => {
                regs.extend(args.iter().cloned());
            }
            crate::Instruction::DynamicCall { args, .. } => {
                regs.extend(args.iter().cloned());
            }
            _ => {}
        }
        regs
    }

    fn decayed_regs(&self) -> Vec<Reg> {
        let mut regs = Vec::new();
        match self {
            crate::Instruction::FieldAccess { target, .. }
            | crate::Instruction::IndexAccess { target, .. } => {
                regs.push(*target);
            }
            _ => {}
        }
        regs
    }
}
