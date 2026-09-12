use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_core::value::{EntropicState, Payload};
use causm_core::BinaryOperator;
use causm_ir::Reg;

#[allow(non_snake_case)]
impl Vm {
    pub(crate) fn LoadInt(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: i64,
    ) -> Result<(), TemporalError> {
        self.insert_reg(
            branch_id,
            dest.0,
            EntropicState::Valid(Payload::Integer(value)),
        )
    }

    pub(crate) fn LoadFloat(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: u64,
    ) -> Result<(), TemporalError> {
        self.insert_reg(
            branch_id,
            dest.0,
            EntropicState::Valid(Payload::Float(value)),
        )
    }

    pub(crate) fn LoadBool(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: bool,
    ) -> Result<(), TemporalError> {
        self.insert_reg(
            branch_id,
            dest.0,
            EntropicState::Valid(Payload::Bool(value)),
        )
    }

    pub(crate) fn LoadString(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: String,
    ) -> Result<(), TemporalError> {
        self.insert_reg(
            branch_id,
            dest.0,
            EntropicState::Valid(Payload::String(value)),
        )
    }

    pub(crate) fn LoadNull(
        &mut self,
        branch_id: &str,
        dest: Reg,
    ) -> Result<(), TemporalError> {
        self.insert_reg(branch_id, dest.0, EntropicState::Valid(Payload::Null))
    }

    pub(crate) fn ConstInt(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: i64,
    ) -> Result<(), TemporalError> {
        self.LoadInt(branch_id, dest, value)
    }

    pub(crate) fn ConstFloat(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: u64,
    ) -> Result<(), TemporalError> {
        self.LoadFloat(branch_id, dest, value)
    }

    pub(crate) fn ConstBool(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: bool,
    ) -> Result<(), TemporalError> {
        self.LoadBool(branch_id, dest, value)
    }

    pub(crate) fn ConstString(
        &mut self,
        branch_id: &str,
        dest: Reg,
        value: String,
    ) -> Result<(), TemporalError> {
        self.LoadString(branch_id, dest, value)
    }

    pub(crate) fn ConstNull(
        &mut self,
        branch_id: &str,
        dest: Reg,
    ) -> Result<(), TemporalError> {
        self.LoadNull(branch_id, dest)
    }

    pub(crate) fn Move(
        &mut self,
        branch_id: &str,
        dest: Reg,
        src: Reg,
    ) -> Result<(), TemporalError> {
        if src == dest {
            return Ok(());
        }
        let state = self.peek_state(branch_id, src.0)?;
        let metadata = {
            let branch = self.get_branch(branch_id)?;
            branch.arena.get_metadata(src.0).cloned()
        };
        self.insert_reg_with_metadata(branch_id, dest.0, state, metadata)
    }

    pub(crate) fn BinaryOp(
        &mut self,
        branch_id: &str,
        dest: Reg,
        op: causm_core::BinaryOperator,
        left: Reg,
        right: Reg,
    ) -> Result<(), TemporalError> {
        let l_val = self.peek_reg(branch_id, left.0)?;
        let r_val = self.peek_reg(branch_id, right.0)?;
        let result = self.evaluate_binary_operation(l_val, r_val, &op)?;
        self.insert_reg(branch_id, dest.0, EntropicState::Valid(result))
    }

    pub(crate) fn UnaryOp(
        &mut self,
        branch_id: &str,
        dest: Reg,
        op: causm_core::UnaryOperator,
        src: Reg,
    ) -> Result<(), TemporalError> {
        let val = self.peek_reg(branch_id, src.0)?;
        let result = self.evaluate_unary_operation(val, &op)?;
        self.insert_reg(branch_id, dest.0, EntropicState::Valid(result))
    }

    pub(crate) fn ConditionalSelect(
        &mut self,
        branch_id: &str,
        dest: Reg,
        cond: Reg,
        true_val: Reg,
        false_val: Reg,
    ) -> Result<(), TemporalError> {
        let is_true = match self.peek_reg(branch_id, cond.0)? {
            Payload::Bool(b) => b,
            other => {
                return Err(TemporalError::TypeMismatch(format!(
                    "ConditionalSelect condition must be bool, got {:?}",
                    other
                )));
            }
        };
        let chosen_reg = if is_true { true_val } else { false_val };
        let val = self.peek_reg(branch_id, chosen_reg.0)?;
        self.insert_reg(branch_id, dest.0, EntropicState::Valid(val))
    }
}

impl Vm {
    pub(crate) fn is_intrinsic(&self, name: &str) -> bool {
        crate::vm::intrinsics::is_intrinsic(name)
    }

    pub(crate) fn call_intrinsic(
        &self,
        name: &str,
        args: Vec<Payload>,
    ) -> Result<Payload, TemporalError> {
        crate::vm::intrinsics::dispatch_intrinsic(name, &args).unwrap_or_else(|| {
            Err(TemporalError::EvalError(format!(
                "Unknown intrinsic: {}",
                name
            )))
        })
    }

    pub(crate) fn evaluate_unary_operation(
        &self,
        val: Payload,
        op: &causm_core::UnaryOperator,
    ) -> Result<Payload, TemporalError> {
        match op {
            causm_core::UnaryOperator::Neg => match val {
                Payload::Integer(i) => Ok(Payload::Integer(-i)),
                Payload::Float(bits) => {
                    let f = f64::from_bits(bits);
                    Ok(Payload::Float((-f).to_bits()))
                }
                _ => Err(TemporalError::TypeMismatch(format!(
                    "Cannot negate {:?}",
                    val
                ))),
            },
            causm_core::UnaryOperator::Not => match val {
                Payload::Bool(b) => Ok(Payload::Bool(!b)),
                _ => Err(TemporalError::TypeMismatch(format!(
                    "Cannot apply NOT to {:?}",
                    val
                ))),
            },
            causm_core::UnaryOperator::BitwiseNot => match val {
                Payload::Integer(i) => Ok(Payload::Integer(!i)),
                _ => Err(TemporalError::TypeMismatch(format!(
                    "Cannot apply bitwise NOT (~) to non-integer {:?}",
                    val
                ))),
            },
        }
    }

    pub(crate) fn evaluate_binary_operation(
        &self,
        left_value: Payload,
        right_value: Payload,
        op: &BinaryOperator,
    ) -> Result<Payload, TemporalError> {
        if *op == BinaryOperator::NullCoalesce {
            return match left_value {
                Payload::Null => Ok(right_value),
                l => Ok(l),
            };
        }

        let result = match (left_value, right_value) {
            // Null equality: any type compared against Null — always succeeds.
            (Payload::Null, Payload::Null) if op == &BinaryOperator::Eq => {
                Payload::Bool(true)
            }
            (Payload::Null, Payload::Null) if op == &BinaryOperator::Neq => {
                Payload::Bool(false)
            }
            (_, Payload::Null) if op == &BinaryOperator::Eq => Payload::Bool(false),
            (_, Payload::Null) if op == &BinaryOperator::Neq => Payload::Bool(true),
            (Payload::Null, _) if op == &BinaryOperator::Eq => Payload::Bool(false),
            (Payload::Null, _) if op == &BinaryOperator::Neq => Payload::Bool(true),
            (Payload::Array(l_elems), Payload::Array(r_elems)) => {
                if l_elems.len() != r_elems.len() {
                    return Err(TemporalError::EvalError(format!(
                        "Array broadcasting dimension mismatch: {} and {}",
                        l_elems.len(),
                        r_elems.len()
                    )));
                }
                let mut res = Vec::new();
                for (l, r) in l_elems.into_iter().zip(r_elems) {
                    res.push(self.evaluate_binary_operation(l, r, op)?);
                }
                Payload::Array(res)
            }
            (Payload::Array(l_elems), r_val) => {
                let mut res = Vec::new();
                for l in l_elems {
                    res.push(self.evaluate_binary_operation(
                        l,
                        r_val.clone(),
                        op,
                    )?);
                }
                Payload::Array(res)
            }
            (l_val, Payload::Array(r_elems)) => {
                let mut res = Vec::new();
                for r in r_elems {
                    res.push(self.evaluate_binary_operation(
                        l_val.clone(),
                        r,
                        op,
                    )?);
                }
                Payload::Array(res)
            }
            (Payload::String(l), r) if op == &BinaryOperator::Add => {
                Payload::String(format!("{}{}", l, r))
            }
            (l, Payload::String(r)) if op == &BinaryOperator::Add => {
                Payload::String(format!("{}{}", l, r))
            }
            (Payload::Integer(l), Payload::Integer(r)) => match op {
                BinaryOperator::Add => Payload::Integer(l + r),
                BinaryOperator::Sub => Payload::Integer(l - r),
                BinaryOperator::Mul => Payload::Integer(l * r),
                BinaryOperator::Div => {
                    if r == 0 {
                        return Err(TemporalError::EvalError(
                            "Division by zero".into(),
                        ));
                    }
                    Payload::Integer(l / r)
                }
                BinaryOperator::Rem => {
                    if r == 0 {
                        return Err(TemporalError::EvalError(
                            "Modulo by zero".into(),
                        ));
                    }
                    Payload::Integer(l % r)
                }
                BinaryOperator::Pow => {
                    if r < 0 {
                        let lf = l as f64;
                        let rf = r as f64;
                        Payload::Float(lf.powf(rf).to_bits())
                    } else {
                        Payload::Integer(l.pow(r as u32))
                    }
                }
                BinaryOperator::BitwiseAnd => Payload::Integer(l & r),
                BinaryOperator::BitwiseOr => Payload::Integer(l | r),
                BinaryOperator::BitwiseXor => Payload::Integer(l ^ r),
                BinaryOperator::Shl => {
                    if !(0..64).contains(&r) {
                        return Err(TemporalError::EvalError(
                            "Shift operand out of bounds [0, 63]".into(),
                        ));
                    }
                    Payload::Integer(l << (r as u32))
                }
                BinaryOperator::Shr => {
                    if !(0..64).contains(&r) {
                        return Err(TemporalError::EvalError(
                            "Shift operand out of bounds [0, 63]".into(),
                        ));
                    }
                    Payload::Integer(l >> (r as u32))
                }
                BinaryOperator::Eq => Payload::Bool(l == r),
                BinaryOperator::Neq => Payload::Bool(l != r),
                BinaryOperator::Lt => Payload::Bool(l < r),
                BinaryOperator::Gt => Payload::Bool(l > r),
                BinaryOperator::Le => Payload::Bool(l <= r),
                BinaryOperator::Ge => Payload::Bool(l >= r),
                BinaryOperator::LogicalAnd | BinaryOperator::LogicalOr => {
                    return Err(TemporalError::TypeMismatch(
                        "Logical operators && and || require boolean operands"
                            .into(),
                    ));
                }
                BinaryOperator::NullCoalesce => unreachable!(),
            },
            (l, r) if l.is_numeric() && r.is_numeric() => {
                let lf = l.as_float().unwrap();
                let rf = r.as_float().unwrap();
                match op {
                    BinaryOperator::Add => Payload::Float((lf + rf).to_bits()),
                    BinaryOperator::Sub => Payload::Float((lf - rf).to_bits()),
                    BinaryOperator::Mul => Payload::Float((lf * rf).to_bits()),
                    BinaryOperator::Div => {
                        if rf == 0.0 {
                            return Err(TemporalError::EvalError(
                                "Division by zero".into(),
                            ));
                        }
                        Payload::Float((lf / rf).to_bits())
                    }
                    BinaryOperator::Rem => {
                        if rf == 0.0 {
                            return Err(TemporalError::EvalError(
                                "Modulo by zero".into(),
                            ));
                        }
                        Payload::Float((lf % rf).to_bits())
                    }
                    BinaryOperator::Pow => Payload::Float(lf.powf(rf).to_bits()),
                    BinaryOperator::Eq => Payload::Bool(lf == rf),
                    BinaryOperator::Neq => Payload::Bool(lf != rf),
                    BinaryOperator::Lt => Payload::Bool(lf < rf),
                    BinaryOperator::Gt => Payload::Bool(lf > rf),
                    BinaryOperator::Le => Payload::Bool(lf <= rf),
                    BinaryOperator::Ge => Payload::Bool(lf >= rf),
                    _ => {
                        return Err(TemporalError::TypeMismatch(format!(
                            "Unsupported binary operator {:?} for float operands",
                            op
                        )));
                    }
                }
            }
            (Payload::Bool(l), Payload::Bool(r)) => match op {
                BinaryOperator::Eq => Payload::Bool(l == r),
                BinaryOperator::Neq => Payload::Bool(l != r),
                BinaryOperator::LogicalAnd => Payload::Bool(l && r),
                BinaryOperator::LogicalOr => Payload::Bool(l || r),
                BinaryOperator::BitwiseAnd => Payload::Bool(l & r),
                BinaryOperator::BitwiseOr => Payload::Bool(l | r),
                BinaryOperator::BitwiseXor => Payload::Bool(l ^ r),
                _ => {
                    return Err(TemporalError::EvalError(
                        "Invalid boolean operator".into(),
                    ))
                }
            },
            (Payload::String(l), Payload::String(r)) => match op {
                BinaryOperator::Add => Payload::String(format!("{}{}", l, r)),
                BinaryOperator::Eq => Payload::Bool(l == r),
                BinaryOperator::Neq => Payload::Bool(l != r),
                _ => {
                    return Err(TemporalError::EvalError(
                        "String operator unsupported".into(),
                    ))
                }
            },
            (l, r) => {
                return Err(TemporalError::TypeMismatch(format!(
                    "Type mismatch in binary op: {:?} {:?} {:?}",
                    l, op, r
                )));
            }
        };

        Ok(result)
    }
}
