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
                | "push"
                | "pop"
                | "array_push"
                | "array_slice"
                | "string_from_bytes"
                | "char_at"
                | "str_slice"
                | "json_parse"
                | "json_stringify"
        )
    }

    pub(crate) fn call_intrinsic(
        &self,
        name: &str,
        args: Vec<Payload>,
    ) -> Result<Payload, TemporalError> {
        match name {
            "push" | "array_push" => {
                if args.len() != 2 {
                    return Err(TemporalError::EvalError(format!(
                        "{} expects (array, item)",
                        name
                    )));
                }
                match &args[0] {
                    Payload::Array(arr) => {
                        let mut next = arr.clone();
                        next.push(args[1].clone());
                        Ok(Payload::Array(next))
                    }
                    _ => Err(TemporalError::TypeMismatch(format!(
                        "{} expects first argument to be array",
                        name
                    ))),
                }
            }
            "pop" => {
                if args.len() != 1 {
                    return Err(TemporalError::EvalError(
                        "pop expects (array)".to_string(),
                    ));
                }
                match &args[0] {
                    Payload::Array(arr) => {
                        Ok(arr.last().cloned().unwrap_or(Payload::Null))
                    }
                    _ => Err(TemporalError::TypeMismatch(
                        "pop expects argument to be array".to_string(),
                    )),
                }
            }
            "array_slice" => {
                if args.len() != 3 {
                    return Err(TemporalError::EvalError(
                        "array_slice expects (array, start, end)".to_string(),
                    ));
                }
                let start = match args[1] {
                    Payload::Integer(i) => i.max(0) as usize,
                    _ => {
                        return Err(TemporalError::TypeMismatch(
                            "array_slice start must be integer".to_string(),
                        ))
                    }
                };
                let end = match args[2] {
                    Payload::Integer(i) => i.max(0) as usize,
                    _ => {
                        return Err(TemporalError::TypeMismatch(
                            "array_slice end must be integer".to_string(),
                        ))
                    }
                };
                match &args[0] {
                    Payload::Array(arr) => {
                        let clamped_start = start.min(arr.len());
                        let clamped_end = end.min(arr.len()).max(clamped_start);
                        Ok(Payload::Array(arr[clamped_start..clamped_end].to_vec()))
                    }
                    _ => Err(TemporalError::TypeMismatch(
                        "array_slice expects array".to_string(),
                    )),
                }
            }
            "string_from_bytes" => {
                if args.len() != 1 {
                    return Err(TemporalError::EvalError(
                        "string_from_bytes expects (array_of_bytes)".to_string(),
                    ));
                }
                match &args[0] {
                    Payload::Array(arr) => {
                        let bytes: Vec<u8> = arr
                            .iter()
                            .filter_map(|p| match p {
                                Payload::Integer(i) => Some(*i as u8),
                                _ => None,
                            })
                            .collect();
                        let s = String::from_utf8_lossy(&bytes).to_string();
                        Ok(Payload::String(s))
                    }
                    _ => Err(TemporalError::TypeMismatch(
                        "string_from_bytes expects array".to_string(),
                    )),
                }
            }
            "char_at" => {
                if args.len() != 2 {
                    return Err(TemporalError::EvalError(
                        "char_at expects (string, index)".to_string(),
                    ));
                }
                let idx = match args[1] {
                    Payload::Integer(i) => {
                        if i < 0 {
                            return Ok(Payload::Integer(0));
                        }
                        i as usize
                    }
                    _ => {
                        return Err(TemporalError::TypeMismatch(
                            "char_at index must be integer".to_string(),
                        ))
                    }
                };
                match &args[0] {
                    Payload::String(s) => {
                        let ch = if s.is_ascii() {
                            s.as_bytes()
                                .get(idx)
                                .copied()
                                .map(|b| b as i64)
                                .unwrap_or(0)
                        } else {
                            s.chars().nth(idx).map(|c| c as i64).unwrap_or(0)
                        };
                        Ok(Payload::Integer(ch))
                    }
                    _ => Err(TemporalError::TypeMismatch(
                        "char_at expects string".to_string(),
                    )),
                }
            }
            "str_slice" => {
                if args.len() != 3 {
                    return Err(TemporalError::EvalError(
                        "str_slice expects (string, start, end)".to_string(),
                    ));
                }
                let start = match args[1] {
                    Payload::Integer(i) => i.max(0) as usize,
                    _ => {
                        return Err(TemporalError::TypeMismatch(
                            "str_slice start must be integer".to_string(),
                        ))
                    }
                };
                let end = match args[2] {
                    Payload::Integer(i) => i.max(0) as usize,
                    _ => {
                        return Err(TemporalError::TypeMismatch(
                            "str_slice end must be integer".to_string(),
                        ))
                    }
                };
                match &args[0] {
                    Payload::String(s) => {
                        if s.is_ascii() {
                            let len = s.len();
                            let clamped_start = start.min(len);
                            let clamped_end = end.min(len).max(clamped_start);
                            Ok(Payload::String(
                                s[clamped_start..clamped_end].to_string(),
                            ))
                        } else {
                            let chars: Vec<char> = s.chars().collect();
                            let clamped_start = start.min(chars.len());
                            let clamped_end =
                                end.min(chars.len()).max(clamped_start);
                            let sliced: String =
                                chars[clamped_start..clamped_end].iter().collect();
                            Ok(Payload::String(sliced))
                        }
                    }
                    _ => Err(TemporalError::TypeMismatch(
                        "str_slice expects string".to_string(),
                    )),
                }
            }
            "json_parse" => {
                if args.len() != 1 {
                    return Err(TemporalError::EvalError(
                        "json_parse expects (json_string)".to_string(),
                    ));
                }
                match &args[0] {
                    Payload::String(s) => {
                        let parsed = crate::vm::intrinsics::json::parse_json(s)
                            .map_err(|e| {
                                TemporalError::EvalError(format!(
                                    "JSON parse error: {}",
                                    e
                                ))
                            })?;
                        Ok(parsed)
                    }
                    _ => Err(TemporalError::TypeMismatch(
                        "json_parse expects string".to_string(),
                    )),
                }
            }
            "json_stringify" => {
                if args.len() != 1 {
                    return Err(TemporalError::EvalError(
                        "json_stringify expects (payload)".to_string(),
                    ));
                }
                let s = crate::vm::intrinsics::json::stringify_json(&args[0]);
                Ok(Payload::String(s))
            }
            _ => {
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
        }
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
                    if r < 0 || r >= 64 {
                        return Err(TemporalError::EvalError(
                            "Shift operand out of bounds [0, 63]".into(),
                        ));
                    }
                    Payload::Integer(l << (r as u32))
                }
                BinaryOperator::Shr => {
                    if r < 0 || r >= 64 {
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
