use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_core::value::Payload;
use causm_core::BinaryOperator;

impl Vm {
    pub(crate) fn is_intrinsic(&self, name: &str) -> bool {
        matches!(
            name,
            "sqrt"
                | "sin"
                | "cos"
                | "tan"
                | "exp"
                | "ln"
                | "log10"
                | "floor"
                | "ceil"
                | "round"
        )
    }

    pub(crate) fn call_intrinsic(
        &self,
        name: &str,
        args: Vec<Payload>,
    ) -> Result<Payload, TemporalError> {
        if args.len() != 1 {
            return Err(TemporalError::EvalError(format!(
                "{} expects 1 argument",
                name
            )));
        }
        let f = args[0].as_float().ok_or_else(|| {
            TemporalError::TypeMismatch(format!("{} expects numeric", name))
        })?;
        let res = match name {
            "sqrt" => f.sqrt(),
            "sin" => f.sin(),
            "cos" => f.cos(),
            "tan" => f.tan(),
            "exp" => f.exp(),
            "ln" => f.ln(),
            "log10" => f.log10(),
            "floor" => f.floor(),
            "ceil" => f.ceil(),
            "round" => f.round(),
            _ => unreachable!(),
        };
        Ok(Payload::Float(res.to_bits()))
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
        }
    }

    pub(crate) fn evaluate_binary_operation(
        &self,
        left_value: Payload,
        right_value: Payload,
        op: &BinaryOperator,
    ) -> Result<Payload, TemporalError> {
        let result = match (left_value, right_value) {
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
                BinaryOperator::Eq => Payload::Bool(l == r),
                BinaryOperator::Neq => Payload::Bool(l != r),
                BinaryOperator::Lt => Payload::Bool(l < r),
                BinaryOperator::Gt => Payload::Bool(l > r),
                BinaryOperator::Le => Payload::Bool(l <= r),
                BinaryOperator::Ge => Payload::Bool(l >= r),
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
                }
            }
            (Payload::Bool(l), Payload::Bool(r)) => match op {
                BinaryOperator::Eq => Payload::Bool(l == r),
                BinaryOperator::Neq => Payload::Bool(l != r),
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
