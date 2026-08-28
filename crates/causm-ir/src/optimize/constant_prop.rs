use super::OptimizationPass;
use crate::ssa::{SsaCFG, SsaInstruction, SsaReg, SsaTerminator};
use std::collections::{HashMap, HashSet};

pub struct ConstantPropagationPass;

impl OptimizationPass for ConstantPropagationPass {
    fn name(&self) -> &str {
        "ConstantPropagation"
    }

    fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        _globally_used_regs: &HashSet<u32>,
        _is_routine: bool,
    ) -> bool {
        constant_propagation(ssa_cfg)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SsaConstant {
    Int(i64),
    Float(u64),
    Bool(bool),
    String(String),
    Null,
}

fn fold_binary_op(
    op: &causm_core::BinaryOperator,
    left: &SsaConstant,
    right: &SsaConstant,
) -> Option<SsaConstant> {
    match (left, right) {
        (SsaConstant::Int(l), SsaConstant::Int(r)) => match op {
            causm_core::BinaryOperator::Add => {
                Some(SsaConstant::Int(l.wrapping_add(*r)))
            }
            causm_core::BinaryOperator::Sub => {
                Some(SsaConstant::Int(l.wrapping_sub(*r)))
            }
            causm_core::BinaryOperator::Mul => {
                Some(SsaConstant::Int(l.wrapping_mul(*r)))
            }
            causm_core::BinaryOperator::Div => {
                if *r != 0 {
                    Some(SsaConstant::Int(l / r))
                } else {
                    None
                }
            }
            causm_core::BinaryOperator::Rem => {
                if *r != 0 {
                    Some(SsaConstant::Int(l % r))
                } else {
                    None
                }
            }
            causm_core::BinaryOperator::Pow => {
                if *r >= 0 {
                    Some(SsaConstant::Int(l.pow(*r as u32)))
                } else {
                    None
                }
            }
            causm_core::BinaryOperator::BitwiseAnd => Some(SsaConstant::Int(l & r)),
            causm_core::BinaryOperator::BitwiseOr => Some(SsaConstant::Int(l | r)),
            causm_core::BinaryOperator::BitwiseXor => Some(SsaConstant::Int(l ^ r)),
            causm_core::BinaryOperator::Shl => {
                if *r >= 0 && *r < 64 {
                    Some(SsaConstant::Int(l << (*r as u32)))
                } else {
                    None
                }
            }
            causm_core::BinaryOperator::Shr => {
                if *r >= 0 && *r < 64 {
                    Some(SsaConstant::Int(l >> (*r as u32)))
                } else {
                    None
                }
            }
            causm_core::BinaryOperator::Eq => Some(SsaConstant::Bool(l == r)),
            causm_core::BinaryOperator::Neq => Some(SsaConstant::Bool(l != r)),
            causm_core::BinaryOperator::Lt => Some(SsaConstant::Bool(l < r)),
            causm_core::BinaryOperator::Gt => Some(SsaConstant::Bool(l > r)),
            causm_core::BinaryOperator::Le => Some(SsaConstant::Bool(l <= r)),
            causm_core::BinaryOperator::Ge => Some(SsaConstant::Bool(l >= r)),
            _ => None,
        },
        (SsaConstant::Float(l), SsaConstant::Float(r)) => {
            let lf = f64::from_bits(*l);
            let rf = f64::from_bits(*r);
            match op {
                causm_core::BinaryOperator::Add => {
                    Some(SsaConstant::Float((lf + rf).to_bits()))
                }
                causm_core::BinaryOperator::Sub => {
                    Some(SsaConstant::Float((lf - rf).to_bits()))
                }
                causm_core::BinaryOperator::Mul => {
                    Some(SsaConstant::Float((lf * rf).to_bits()))
                }
                causm_core::BinaryOperator::Div => {
                    Some(SsaConstant::Float((lf / rf).to_bits()))
                }
                causm_core::BinaryOperator::Eq => Some(SsaConstant::Bool(lf == rf)),
                causm_core::BinaryOperator::Neq => Some(SsaConstant::Bool(lf != rf)),
                causm_core::BinaryOperator::Lt => Some(SsaConstant::Bool(lf < rf)),
                causm_core::BinaryOperator::Gt => Some(SsaConstant::Bool(lf > rf)),
                causm_core::BinaryOperator::Le => Some(SsaConstant::Bool(lf <= rf)),
                causm_core::BinaryOperator::Ge => Some(SsaConstant::Bool(lf >= rf)),
                _ => None,
            }
        }
        (SsaConstant::Bool(l), SsaConstant::Bool(r)) => match op {
            causm_core::BinaryOperator::Eq => Some(SsaConstant::Bool(l == r)),
            causm_core::BinaryOperator::Neq => Some(SsaConstant::Bool(l != r)),
            causm_core::BinaryOperator::LogicalAnd
            | causm_core::BinaryOperator::BitwiseAnd => {
                Some(SsaConstant::Bool(*l && *r))
            }
            causm_core::BinaryOperator::LogicalOr
            | causm_core::BinaryOperator::BitwiseOr => {
                Some(SsaConstant::Bool(*l || *r))
            }
            causm_core::BinaryOperator::BitwiseXor => {
                Some(SsaConstant::Bool(*l ^ *r))
            }
            _ => None,
        },
        (SsaConstant::String(l), SsaConstant::String(r)) => match op {
            causm_core::BinaryOperator::Add => {
                Some(SsaConstant::String(format!("{}{}", l, r)))
            }
            causm_core::BinaryOperator::Eq => Some(SsaConstant::Bool(l == r)),
            causm_core::BinaryOperator::Neq => Some(SsaConstant::Bool(l != r)),
            _ => None,
        },
        _ => None,
    }
}

fn fold_unary_op(
    op: &causm_core::UnaryOperator,
    src: &SsaConstant,
) -> Option<SsaConstant> {
    match src {
        SsaConstant::Int(v) => match op {
            causm_core::UnaryOperator::Neg => {
                Some(SsaConstant::Int(v.wrapping_neg()))
            }
            causm_core::UnaryOperator::BitwiseNot => Some(SsaConstant::Int(!v)),
            _ => None,
        },
        SsaConstant::Bool(v) => match op {
            causm_core::UnaryOperator::Not => Some(SsaConstant::Bool(!v)),
            _ => None,
        },
        _ => None,
    }
}

fn constant_propagation(ssa_cfg: &mut SsaCFG) -> bool {
    let mut constants: HashMap<SsaReg, SsaConstant> = HashMap::new();
    let mut changed = true;

    while changed {
        changed = false;
        for block in ssa_cfg.blocks.values() {
            for inst in &block.instructions {
                match inst {
                    SsaInstruction::LoadInt { dest, value }
                    | SsaInstruction::ConstInt { dest, value } => {
                        if constants
                            .insert(*dest, SsaConstant::Int(*value))
                            .is_none()
                        {
                            changed = true;
                        }
                    }
                    SsaInstruction::LoadFloat { dest, value }
                    | SsaInstruction::ConstFloat { dest, value } => {
                        if constants
                            .insert(*dest, SsaConstant::Float(*value))
                            .is_none()
                        {
                            changed = true;
                        }
                    }
                    SsaInstruction::LoadBool { dest, value }
                    | SsaInstruction::ConstBool { dest, value } => {
                        if constants
                            .insert(*dest, SsaConstant::Bool(*value))
                            .is_none()
                        {
                            changed = true;
                        }
                    }
                    SsaInstruction::LoadString { dest, value }
                    | SsaInstruction::ConstString { dest, value } => {
                        if constants
                            .insert(*dest, SsaConstant::String(value.clone()))
                            .is_none()
                        {
                            changed = true;
                        }
                    }
                    SsaInstruction::LoadNull { dest }
                    | SsaInstruction::ConstNull { dest } => {
                        if constants.insert(*dest, SsaConstant::Null).is_none() {
                            changed = true;
                        }
                    }
                    SsaInstruction::Move { dest, src }
                    | SsaInstruction::Clone { dest, src } => {
                        if let Some(val) = constants.get(src).cloned() {
                            let old = constants.insert(*dest, val);
                            if old.is_none() {
                                changed = true;
                            }
                        }
                    }
                    SsaInstruction::UnaryOp { dest, op, src } => {
                        if let Some(src_val) = constants.get(src) {
                            if let Some(folded) = fold_unary_op(op, src_val) {
                                let old = constants.insert(*dest, folded);
                                if old.is_none() {
                                    changed = true;
                                }
                            }
                        }
                    }
                    SsaInstruction::BinaryOp {
                        dest,
                        op,
                        left,
                        right,
                    } => {
                        if let Some(l_val) = constants.get(left) {
                            if let Some(r_val) = constants.get(right) {
                                if let Some(folded) =
                                    fold_binary_op(op, l_val, r_val)
                                {
                                    let old = constants.insert(*dest, folded);
                                    if old.is_none() {
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            for phi in &block.phi_nodes {
                if phi.incoming.is_empty() {
                    continue;
                }
                let mut first_val: Option<SsaConstant> = None;
                let mut all_same = true;
                for (_, incoming_reg) in &phi.incoming {
                    if let Some(val) = constants.get(incoming_reg) {
                        if let Some(ref f) = first_val {
                            if f != val {
                                all_same = false;
                                break;
                            }
                        } else {
                            first_val = Some(val.clone());
                        }
                    } else {
                        all_same = false;
                        break;
                    }
                }
                if all_same {
                    if let Some(val) = first_val {
                        let old = constants.insert(phi.dest, val);
                        if old.is_none() {
                            changed = true;
                        }
                    }
                }
            }
        }
    }

    let mut any_modified = false;
    for block in ssa_cfg.blocks.values_mut() {
        for inst in &mut block.instructions {
            match inst {
                SsaInstruction::BinaryOp { dest, .. } => {
                    if let Some(val) = constants.get(dest) {
                        match val {
                            SsaConstant::Int(v) => {
                                *inst = SsaInstruction::LoadInt {
                                    dest: *dest,
                                    value: *v,
                                };
                                any_modified = true;
                            }
                            SsaConstant::Float(v) => {
                                *inst = SsaInstruction::LoadFloat {
                                    dest: *dest,
                                    value: *v,
                                };
                                any_modified = true;
                            }
                            SsaConstant::Bool(v) => {
                                *inst = SsaInstruction::LoadBool {
                                    dest: *dest,
                                    value: *v,
                                };
                                any_modified = true;
                            }
                            SsaConstant::String(v) => {
                                *inst = SsaInstruction::LoadString {
                                    dest: *dest,
                                    value: v.clone(),
                                };
                                any_modified = true;
                            }
                            SsaConstant::Null => {
                                *inst = SsaInstruction::LoadNull { dest: *dest };
                                any_modified = true;
                            }
                        }
                    }
                }
                SsaInstruction::UnaryOp { dest, .. } => {
                    if let Some(val) = constants.get(dest) {
                        match val {
                            SsaConstant::Int(v) => {
                                *inst = SsaInstruction::LoadInt {
                                    dest: *dest,
                                    value: *v,
                                };
                                any_modified = true;
                            }
                            SsaConstant::Float(v) => {
                                *inst = SsaInstruction::LoadFloat {
                                    dest: *dest,
                                    value: *v,
                                };
                                any_modified = true;
                            }
                            SsaConstant::Bool(v) => {
                                *inst = SsaInstruction::LoadBool {
                                    dest: *dest,
                                    value: *v,
                                };
                                any_modified = true;
                            }
                            SsaConstant::String(v) => {
                                *inst = SsaInstruction::LoadString {
                                    dest: *dest,
                                    value: v.clone(),
                                };
                                any_modified = true;
                            }
                            SsaConstant::Null => {
                                *inst = SsaInstruction::LoadNull { dest: *dest };
                                any_modified = true;
                            }
                        }
                    }
                }
                SsaInstruction::Move { dest, .. } => {
                    if let Some(val) = constants.get(dest) {
                        match val {
                            SsaConstant::Int(v) => {
                                *inst = SsaInstruction::LoadInt {
                                    dest: *dest,
                                    value: *v,
                                };
                                any_modified = true;
                            }
                            SsaConstant::Float(v) => {
                                *inst = SsaInstruction::LoadFloat {
                                    dest: *dest,
                                    value: *v,
                                };
                                any_modified = true;
                            }
                            SsaConstant::Bool(v) => {
                                *inst = SsaInstruction::LoadBool {
                                    dest: *dest,
                                    value: *v,
                                };
                                any_modified = true;
                            }
                            SsaConstant::String(v) => {
                                *inst = SsaInstruction::LoadString {
                                    dest: *dest,
                                    value: v.clone(),
                                };
                                any_modified = true;
                            }
                            SsaConstant::Null => {
                                *inst = SsaInstruction::LoadNull { dest: *dest };
                                any_modified = true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if let SsaTerminator::Branch {
            cond,
            then_block,
            else_block,
        } = &block.terminator
        {
            if let Some(SsaConstant::Bool(b)) = constants.get(cond) {
                let target = if *b { *then_block } else { *else_block };
                block.terminator = SsaTerminator::Jump { target };
                any_modified = true;
            }
        }
    }

    any_modified
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::CFG;
    use crate::Instruction;
    use crate::Reg;

    #[test]
    fn test_dedicated_const_instructions_propagation() {
        let instrs = vec![
            Instruction::ConstInt {
                dest: Reg(1),
                value: 10,
            },
            Instruction::ConstInt {
                dest: Reg(2),
                value: 20,
            },
            Instruction::BinaryOp {
                dest: Reg(3),
                op: causm_core::BinaryOperator::Add,
                left: Reg(1),
                right: Reg(2),
            },
            Instruction::ConstString {
                dest: Reg(4),
                value: "hello".to_string(),
            },
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = crate::ssa::SsaTransformer::new(cfg);
        let mut ssa_cfg = transformer.transform();

        let modified = constant_propagation(&mut ssa_cfg);
        assert!(modified);

        let entry = ssa_cfg.blocks.get(&ssa_cfg.entry_block).unwrap();
        // The binary op adding 10 + 20 should be folded into ConstInt 30 or LoadInt 30
        let has_folded_add = entry.instructions.iter().any(|i| {
            matches!(
                i,
                SsaInstruction::LoadInt { value: 30, .. }
                    | SsaInstruction::ConstInt { value: 30, .. }
            )
        });
        assert!(has_folded_add);
    }
}
