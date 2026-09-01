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
                | "__sync_atomic_new_int"
                | "__sync_atomic_load_int"
                | "__sync_atomic_store_int"
                | "__sync_atomic_fetch_add"
                | "__sync_atomic_cas_int"
                | "__sync_atomic_new_bool"
                | "__sync_atomic_load_bool"
                | "__sync_atomic_store_bool"
                | "__sync_atomic_cas_bool"
                | "__sync_mutex_new"
                | "__sync_mutex_try_lock"
                | "__sync_mutex_unlock"
                | "__sync_mutex_is_locked"
                | "__sync_mutex_owner"
                | "__sync_channel_new"
                | "__sync_channel_send"
                | "__sync_channel_recv"
                | "__sync_channel_close"
                | "__sync_channel_is_closed"
                | "__sync_channel_len"
                | "__sync_channel_is_full"
                | "__sync_channel_is_empty"
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
            "__sync_atomic_new_int" => {
                let initial = match args.first() {
                    Some(Payload::Integer(i)) => *i,
                    _ => 0,
                };
                let mut m = std::collections::HashMap::new();
                m.insert("val".to_string(), causm_core::value::EntropicState::Valid(Payload::Integer(initial)));
                Ok(Payload::Struct(m))
            }
            "__sync_atomic_load_int" => match args.first() {
                Some(Payload::Struct(fields)) => match fields.get("val") {
                    Some(causm_core::value::EntropicState::Valid(v)) => Ok(v.clone()),
                    _ => Ok(Payload::Integer(0)),
                },
                _ => Err(TemporalError::TypeMismatch("Atomic.load_int expects AtomicInt".into())),
            },
            "__sync_atomic_store_int" => {
                let val = match args.get(1) {
                    Some(Payload::Integer(i)) => *i,
                    _ => return Err(TemporalError::TypeMismatch("Atomic.store_int: value must be int".into())),
                };
                let mut m = std::collections::HashMap::new();
                m.insert("val".to_string(), causm_core::value::EntropicState::Valid(Payload::Integer(val)));
                Ok(Payload::Struct(m))
            }
            "__sync_atomic_fetch_add" => {
                let old = match args.first() {
                    Some(Payload::Struct(f)) => match f.get("val") {
                        Some(causm_core::value::EntropicState::Valid(Payload::Integer(i))) => *i,
                        _ => 0,
                    },
                    _ => return Err(TemporalError::TypeMismatch("Atomic.fetch_add expects AtomicInt".into())),
                };
                let delta = match args.get(1) {
                    Some(Payload::Integer(d)) => *d,
                    _ => return Err(TemporalError::TypeMismatch("Atomic.fetch_add: delta must be int".into())),
                };
                let mut next = std::collections::HashMap::new();
                next.insert("val".to_string(), causm_core::value::EntropicState::Valid(Payload::Integer(old + delta)));
                let mut res = std::collections::HashMap::new();
                res.insert("atomic".to_string(), causm_core::value::EntropicState::Valid(Payload::Struct(next)));
                res.insert("old".to_string(), causm_core::value::EntropicState::Valid(Payload::Integer(old)));
                Ok(Payload::Struct(res))
            }
            "__sync_atomic_cas_int" => {
                let cur = match args.first() {
                    Some(Payload::Struct(f)) => match f.get("val") {
                        Some(causm_core::value::EntropicState::Valid(Payload::Integer(i))) => *i,
                        _ => 0,
                    },
                    _ => return Err(TemporalError::TypeMismatch("Atomic.cas_int expects AtomicInt".into())),
                };
                let exp = match args.get(1) { Some(Payload::Integer(i)) => *i, _ => return Err(TemporalError::TypeMismatch("Atomic.cas_int: bad expected".into())) };
                let des = match args.get(2) { Some(Payload::Integer(i)) => *i, _ => return Err(TemporalError::TypeMismatch("Atomic.cas_int: bad desired".into())) };
                let ok = cur == exp;
                let mut next = std::collections::HashMap::new();
                next.insert("val".to_string(), causm_core::value::EntropicState::Valid(Payload::Integer(if ok { des } else { cur })));
                let mut res = std::collections::HashMap::new();
                res.insert("atomic".to_string(), causm_core::value::EntropicState::Valid(Payload::Struct(next)));
                res.insert("success".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(ok)));
                Ok(Payload::Struct(res))
            }
            "__sync_atomic_new_bool" => {
                let initial = matches!(args.first(), Some(Payload::Bool(true)));
                let mut m = std::collections::HashMap::new();
                m.insert("val".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(initial)));
                Ok(Payload::Struct(m))
            }
            "__sync_atomic_load_bool" => match args.first() {
                Some(Payload::Struct(fields)) => match fields.get("val") {
                    Some(causm_core::value::EntropicState::Valid(v)) => Ok(v.clone()),
                    _ => Ok(Payload::Bool(false)),
                },
                _ => Err(TemporalError::TypeMismatch("Atomic.load_bool expects AtomicBool".into())),
            },
            "__sync_atomic_store_bool" => {
                let val = match args.get(1) { Some(Payload::Bool(b)) => *b, _ => return Err(TemporalError::TypeMismatch("Atomic.store_bool: bad value".into())) };
                let mut m = std::collections::HashMap::new();
                m.insert("val".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(val)));
                Ok(Payload::Struct(m))
            }
            "__sync_atomic_cas_bool" => {
                let cur = match args.first() {
                    Some(Payload::Struct(f)) => match f.get("val") {
                        Some(causm_core::value::EntropicState::Valid(Payload::Bool(b))) => *b,
                        _ => false,
                    },
                    _ => return Err(TemporalError::TypeMismatch("Atomic.cas_bool expects AtomicBool".into())),
                };
                let exp = match args.get(1) { Some(Payload::Bool(b)) => *b, _ => return Err(TemporalError::TypeMismatch("Atomic.cas_bool: bad expected".into())) };
                let des = match args.get(2) { Some(Payload::Bool(b)) => *b, _ => return Err(TemporalError::TypeMismatch("Atomic.cas_bool: bad desired".into())) };
                let ok = cur == exp;
                let mut next = std::collections::HashMap::new();
                next.insert("val".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(if ok { des } else { cur })));
                let mut res = std::collections::HashMap::new();
                res.insert("atomic".to_string(), causm_core::value::EntropicState::Valid(Payload::Struct(next)));
                res.insert("success".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(ok)));
                Ok(Payload::Struct(res))
            }
            "__sync_mutex_new" => {
                let mut m = std::collections::HashMap::new();
                m.insert("locked".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(false)));
                m.insert("owner".to_string(), causm_core::value::EntropicState::Valid(Payload::String(String::new())));
                Ok(Payload::Struct(m))
            }
            "__sync_mutex_try_lock" => {
                let (locked, owner_cur) = match args.first() {
                    Some(Payload::Struct(f)) => {
                        let l = matches!(f.get("locked"), Some(causm_core::value::EntropicState::Valid(Payload::Bool(true))));
                        let o = match f.get("owner") {
                            Some(causm_core::value::EntropicState::Valid(Payload::String(s))) => s.clone(),
                            _ => String::new(),
                        };
                        (l, o)
                    }
                    _ => return Err(TemporalError::TypeMismatch("Mutex.try_lock expects Mutex".into())),
                };
                let requester = match args.get(1) { Some(Payload::String(s)) => s.clone(), _ => "anonymous".to_string() };
                let acquired = !locked;
                let mut next = std::collections::HashMap::new();
                next.insert("locked".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(if acquired { true } else { locked })));
                next.insert("owner".to_string(), causm_core::value::EntropicState::Valid(Payload::String(if acquired { requester } else { owner_cur })));
                let mut res = std::collections::HashMap::new();
                res.insert("acquired".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(acquired)));
                res.insert("mutex".to_string(), causm_core::value::EntropicState::Valid(Payload::Struct(next)));
                Ok(Payload::Struct(res))
            }
            "__sync_mutex_unlock" => {
                let mut m = std::collections::HashMap::new();
                m.insert("locked".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(false)));
                m.insert("owner".to_string(), causm_core::value::EntropicState::Valid(Payload::String(String::new())));
                Ok(Payload::Struct(m))
            }
            "__sync_mutex_is_locked" => match args.first() {
                Some(Payload::Struct(f)) => Ok(match f.get("locked") {
                    Some(causm_core::value::EntropicState::Valid(Payload::Bool(b))) => Payload::Bool(*b),
                    _ => Payload::Bool(false),
                }),
                _ => Err(TemporalError::TypeMismatch("Mutex.is_locked expects Mutex".into())),
            },
            "__sync_mutex_owner" => match args.first() {
                Some(Payload::Struct(f)) => Ok(match f.get("owner") {
                    Some(causm_core::value::EntropicState::Valid(Payload::String(s))) => Payload::String(s.clone()),
                    _ => Payload::String(String::new()),
                }),
                _ => Err(TemporalError::TypeMismatch("Mutex.owner expects Mutex".into())),
            },
            "__sync_channel_new" => {
                let cap = match args.first() { Some(Payload::Integer(i)) => (*i).max(1), _ => 1 };
                let mut m = std::collections::HashMap::new();
                m.insert("capacity".to_string(), causm_core::value::EntropicState::Valid(Payload::Integer(cap)));
                m.insert("closed".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(false)));
                m.insert("count".to_string(), causm_core::value::EntropicState::Valid(Payload::Integer(0)));
                m.insert("data".to_string(), causm_core::value::EntropicState::Valid(Payload::Array(vec![Payload::Integer(0); cap as usize])));
                m.insert("head".to_string(), causm_core::value::EntropicState::Valid(Payload::Integer(0)));
                m.insert("tail".to_string(), causm_core::value::EntropicState::Valid(Payload::Integer(0)));
                Ok(Payload::Struct(m))
            }
            "__sync_channel_send" => {
                let (cap, closed, mut count, mut data, head, mut tail) = Self::extract_channel_fields(&args)?;
                let val = args.get(1).cloned().unwrap_or(Payload::Null);
                let can_send = !closed && count < cap;
                if can_send {
                    if let Payload::Array(ref mut arr) = data { if (tail as usize) < arr.len() { arr[tail as usize] = val; } }
                    tail = (tail + 1) % cap;
                    count += 1;
                }
                let mut res = std::collections::HashMap::new();
                res.insert("chan".to_string(), causm_core::value::EntropicState::Valid(Payload::Struct(Self::build_channel_struct(cap, closed, count, data, head, tail))));
                res.insert("ok".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(can_send)));
                Ok(Payload::Struct(res))
            }
            "__sync_channel_recv" => {
                let (cap, closed, mut count, mut data, mut head, tail) = Self::extract_channel_fields(&args)?;
                let has_item = count > 0;
                let mut item = Payload::Integer(0);
                if has_item {
                    if let Payload::Array(ref arr) = data { if (head as usize) < arr.len() { item = arr[head as usize].clone(); } }
                    if let Payload::Array(ref mut arr) = data { if (head as usize) < arr.len() { arr[head as usize] = Payload::Integer(0); } }
                    head = (head + 1) % cap;
                    count -= 1;
                }
                let mut res = std::collections::HashMap::new();
                res.insert("chan".to_string(), causm_core::value::EntropicState::Valid(Payload::Struct(Self::build_channel_struct(cap, closed, count, data, head, tail))));
                res.insert("ok".to_string(), causm_core::value::EntropicState::Valid(Payload::Bool(has_item)));
                res.insert("val".to_string(), causm_core::value::EntropicState::Valid(item));
                Ok(Payload::Struct(res))
            }
            "__sync_channel_close" => {
                let (cap, _, count, data, head, tail) = Self::extract_channel_fields(&args)?;
                Ok(Payload::Struct(Self::build_channel_struct(cap, true, count, data, head, tail)))
            }
            "__sync_channel_is_closed" => { let (_, closed, ..) = Self::extract_channel_fields(&args)?; Ok(Payload::Bool(closed)) }
            "__sync_channel_len"       => { let (_, _, count, ..) = Self::extract_channel_fields(&args)?; Ok(Payload::Integer(count)) }
            "__sync_channel_is_full"   => { let (cap, _, count, ..) = Self::extract_channel_fields(&args)?; Ok(Payload::Bool(count >= cap)) }
            "__sync_channel_is_empty"  => { let (_, _, count, ..) = Self::extract_channel_fields(&args)?; Ok(Payload::Bool(count == 0)) }
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

    fn extract_channel_fields(
        args: &[Payload],
    ) -> Result<(i64, bool, i64, Payload, i64, i64), TemporalError> {
        match args.first() {
            Some(Payload::Struct(fields)) => {
                let cap   = match fields.get("capacity") { Some(causm_core::value::EntropicState::Valid(Payload::Integer(i))) => *i, _ => 1 };
                let closed = matches!(fields.get("closed"), Some(causm_core::value::EntropicState::Valid(Payload::Bool(true))));
                let count = match fields.get("count")    { Some(causm_core::value::EntropicState::Valid(Payload::Integer(i))) => *i, _ => 0 };
                let data  = match fields.get("data")     { Some(causm_core::value::EntropicState::Valid(p)) => p.clone(), _ => Payload::Array(vec![]) };
                let head  = match fields.get("head")     { Some(causm_core::value::EntropicState::Valid(Payload::Integer(i))) => *i, _ => 0 };
                let tail  = match fields.get("tail")     { Some(causm_core::value::EntropicState::Valid(Payload::Integer(i))) => *i, _ => 0 };
                Ok((cap, closed, count, data, head, tail))
            }
            _ => Err(TemporalError::TypeMismatch("SyncChannel intrinsic expects SyncChannel struct".into())),
        }
    }

    fn build_channel_struct(
        cap: i64, closed: bool, count: i64, data: Payload, head: i64, tail: i64,
    ) -> std::collections::HashMap<String, causm_core::value::EntropicState> {
        let mut m = std::collections::HashMap::new();
        m.insert("capacity".to_string(), causm_core::value::EntropicState::Valid(Payload::Integer(cap)));
        m.insert("closed".to_string(),   causm_core::value::EntropicState::Valid(Payload::Bool(closed)));
        m.insert("count".to_string(),    causm_core::value::EntropicState::Valid(Payload::Integer(count)));
        m.insert("data".to_string(),     causm_core::value::EntropicState::Valid(data));
        m.insert("head".to_string(),     causm_core::value::EntropicState::Valid(Payload::Integer(head)));
        m.insert("tail".to_string(),     causm_core::value::EntropicState::Valid(Payload::Integer(tail)));
        m
    }
}
