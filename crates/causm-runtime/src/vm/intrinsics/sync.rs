//! Sync intrinsics (atomics, mutex, channel).

use crate::vm::TemporalError;
use causm_core::value::{EntropicState, Payload};
use std::collections::HashMap;

pub fn dispatch(
    name: &str,
    args: &[Payload],
) -> Option<Result<Payload, TemporalError>> {
    match name {
        "__sync_atomic_new_int" => {
            let initial = match args.first() {
                Some(Payload::Integer(i)) => *i,
                _ => 0,
            };
            let mut m = HashMap::new();
            m.insert(
                "val".to_string(),
                EntropicState::Valid(Payload::Integer(initial)),
            );
            Some(Ok(Payload::Struct(m)))
        }
        "__sync_atomic_load_int" => match args.first() {
            Some(Payload::Struct(fields)) => match fields.get("val") {
                Some(EntropicState::Valid(v)) => Some(Ok(v.clone())),
                _ => Some(Ok(Payload::Integer(0))),
            },
            _ => Some(Err(TemporalError::TypeMismatch(
                "Atomic.load_int expects AtomicInt".into(),
            ))),
        },
        "__sync_atomic_store_int" => {
            let val = match args.get(1) {
                Some(Payload::Integer(i)) => *i,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "Atomic.store_int: value must be int".into(),
                    )))
                }
            };
            let mut m = HashMap::new();
            m.insert(
                "val".to_string(),
                EntropicState::Valid(Payload::Integer(val)),
            );
            Some(Ok(Payload::Struct(m)))
        }
        "__sync_atomic_fetch_add" => {
            let old = match args.first() {
                Some(Payload::Struct(f)) => match f.get("val") {
                    Some(EntropicState::Valid(Payload::Integer(i))) => *i,
                    _ => 0,
                },
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "Atomic.fetch_add expects AtomicInt".into(),
                    )))
                }
            };
            let delta = match args.get(1) {
                Some(Payload::Integer(d)) => *d,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "Atomic.fetch_add: delta must be int".into(),
                    )))
                }
            };
            let mut next = HashMap::new();
            next.insert(
                "val".to_string(),
                EntropicState::Valid(Payload::Integer(old + delta)),
            );
            let mut res = HashMap::new();
            res.insert(
                "atomic".to_string(),
                EntropicState::Valid(Payload::Struct(next)),
            );
            res.insert(
                "old".to_string(),
                EntropicState::Valid(Payload::Integer(old)),
            );
            Some(Ok(Payload::Struct(res)))
        }
        "__sync_atomic_cas_int" => {
            let cur = match args.first() {
                Some(Payload::Struct(f)) => match f.get("val") {
                    Some(EntropicState::Valid(Payload::Integer(i))) => *i,
                    _ => 0,
                },
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "Atomic.cas_int expects AtomicInt".into(),
                    )))
                }
            };
            let exp = match args.get(1) {
                Some(Payload::Integer(i)) => *i,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "Atomic.cas_int: bad expected".into(),
                    )))
                }
            };
            let des = match args.get(2) {
                Some(Payload::Integer(i)) => *i,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "Atomic.cas_int: bad desired".into(),
                    )))
                }
            };
            let ok = cur == exp;
            let mut next = HashMap::new();
            next.insert(
                "val".to_string(),
                EntropicState::Valid(Payload::Integer(if ok { des } else { cur })),
            );
            let mut res = HashMap::new();
            res.insert(
                "atomic".to_string(),
                EntropicState::Valid(Payload::Struct(next)),
            );
            res.insert(
                "success".to_string(),
                EntropicState::Valid(Payload::Bool(ok)),
            );
            Some(Ok(Payload::Struct(res)))
        }
        "__sync_atomic_new_bool" => {
            let initial = matches!(args.first(), Some(Payload::Bool(true)));
            let mut m = HashMap::new();
            m.insert(
                "val".to_string(),
                EntropicState::Valid(Payload::Bool(initial)),
            );
            Some(Ok(Payload::Struct(m)))
        }
        "__sync_atomic_load_bool" => match args.first() {
            Some(Payload::Struct(fields)) => match fields.get("val") {
                Some(EntropicState::Valid(v)) => Some(Ok(v.clone())),
                _ => Some(Ok(Payload::Bool(false))),
            },
            _ => Some(Err(TemporalError::TypeMismatch(
                "Atomic.load_bool expects AtomicBool".into(),
            ))),
        },
        "__sync_atomic_store_bool" => {
            let val = match args.get(1) {
                Some(Payload::Bool(b)) => *b,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "Atomic.store_bool: bad value".into(),
                    )))
                }
            };
            let mut m = HashMap::new();
            m.insert("val".to_string(), EntropicState::Valid(Payload::Bool(val)));
            Some(Ok(Payload::Struct(m)))
        }
        "__sync_atomic_cas_bool" => {
            let cur = match args.first() {
                Some(Payload::Struct(f)) => match f.get("val") {
                    Some(EntropicState::Valid(Payload::Bool(b))) => *b,
                    _ => false,
                },
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "Atomic.cas_bool expects AtomicBool".into(),
                    )))
                }
            };
            let exp = match args.get(1) {
                Some(Payload::Bool(b)) => *b,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "Atomic.cas_bool: bad expected".into(),
                    )))
                }
            };
            let des = match args.get(2) {
                Some(Payload::Bool(b)) => *b,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "Atomic.cas_bool: bad desired".into(),
                    )))
                }
            };
            let ok = cur == exp;
            let mut next = HashMap::new();
            next.insert(
                "val".to_string(),
                EntropicState::Valid(Payload::Bool(if ok { des } else { cur })),
            );
            let mut res = HashMap::new();
            res.insert(
                "atomic".to_string(),
                EntropicState::Valid(Payload::Struct(next)),
            );
            res.insert(
                "success".to_string(),
                EntropicState::Valid(Payload::Bool(ok)),
            );
            Some(Ok(Payload::Struct(res)))
        }
        "__sync_mutex_new" => {
            let mut m = HashMap::new();
            m.insert(
                "locked".to_string(),
                EntropicState::Valid(Payload::Bool(false)),
            );
            m.insert(
                "owner".to_string(),
                EntropicState::Valid(Payload::String(String::new())),
            );
            Some(Ok(Payload::Struct(m)))
        }
        "__sync_mutex_try_lock" => {
            let (locked, owner_cur) = match args.first() {
                Some(Payload::Struct(f)) => {
                    let l = matches!(
                        f.get("locked"),
                        Some(EntropicState::Valid(Payload::Bool(true)))
                    );
                    let o = match f.get("owner") {
                        Some(EntropicState::Valid(Payload::String(s))) => s.clone(),
                        _ => String::new(),
                    };
                    (l, o)
                }
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "Mutex.try_lock expects Mutex".into(),
                    )))
                }
            };
            let requester = match args.get(1) {
                Some(Payload::String(s)) => s.clone(),
                _ => "anonymous".to_string(),
            };
            let acquired = !locked;
            let mut next = HashMap::new();
            next.insert(
                "locked".to_string(),
                EntropicState::Valid(Payload::Bool(if acquired {
                    true
                } else {
                    locked
                })),
            );
            next.insert(
                "owner".to_string(),
                EntropicState::Valid(Payload::String(if acquired {
                    requester
                } else {
                    owner_cur
                })),
            );
            let mut res = HashMap::new();
            res.insert(
                "acquired".to_string(),
                EntropicState::Valid(Payload::Bool(acquired)),
            );
            res.insert(
                "mutex".to_string(),
                EntropicState::Valid(Payload::Struct(next)),
            );
            Some(Ok(Payload::Struct(res)))
        }
        "__sync_mutex_unlock" => {
            let mut m = HashMap::new();
            m.insert(
                "locked".to_string(),
                EntropicState::Valid(Payload::Bool(false)),
            );
            m.insert(
                "owner".to_string(),
                EntropicState::Valid(Payload::String(String::new())),
            );
            Some(Ok(Payload::Struct(m)))
        }
        "__sync_mutex_is_locked" => match args.first() {
            Some(Payload::Struct(f)) => {
                let locked = matches!(
                    f.get("locked"),
                    Some(EntropicState::Valid(Payload::Bool(true)))
                );
                Some(Ok(Payload::Bool(locked)))
            }
            _ => Some(Err(TemporalError::TypeMismatch(
                "Mutex.is_locked expects Mutex".into(),
            ))),
        },
        "__sync_mutex_owner" => match args.first() {
            Some(Payload::Struct(f)) => match f.get("owner") {
                Some(EntropicState::Valid(Payload::String(s))) => {
                    Some(Ok(Payload::String(s.clone())))
                }
                _ => Some(Ok(Payload::String(String::new()))),
            },
            _ => Some(Err(TemporalError::TypeMismatch(
                "Mutex.owner expects Mutex".into(),
            ))),
        },
        "__sync_channel_new" => {
            let cap = match args.first() {
                Some(Payload::Integer(i)) => (*i).max(1),
                _ => 1,
            };
            let mut m = HashMap::new();
            m.insert(
                "cap".to_string(),
                EntropicState::Valid(Payload::Integer(cap)),
            );
            m.insert(
                "closed".to_string(),
                EntropicState::Valid(Payload::Bool(false)),
            );
            m.insert(
                "count".to_string(),
                EntropicState::Valid(Payload::Integer(0)),
            );
            m.insert(
                "data".to_string(),
                EntropicState::Valid(Payload::Array(vec![
                    Payload::Integer(0);
                    cap as usize
                ])),
            );
            m.insert(
                "head".to_string(),
                EntropicState::Valid(Payload::Integer(0)),
            );
            m.insert(
                "tail".to_string(),
                EntropicState::Valid(Payload::Integer(0)),
            );
            Some(Ok(Payload::Struct(m)))
        }
        "__sync_channel_send" => {
            let (cap, closed, mut count, mut data, head, mut tail) =
                match extract_channel_fields(args) {
                    Ok(f) => f,
                    Err(e) => return Some(Err(e)),
                };
            let val = args.get(1).cloned().unwrap_or(Payload::Null);
            let can_send = !closed && count < cap;
            if can_send {
                if let Payload::Array(ref mut arr) = data {
                    if (tail as usize) < arr.len() {
                        arr[tail as usize] = val;
                    }
                }
                tail = (tail + 1) % cap;
                count += 1;
            }
            let mut res = HashMap::new();
            res.insert(
                "chan".to_string(),
                EntropicState::Valid(Payload::Struct(build_channel_struct(
                    cap, closed, count, data, head, tail,
                ))),
            );
            res.insert(
                "ok".to_string(),
                EntropicState::Valid(Payload::Bool(can_send)),
            );
            Some(Ok(Payload::Struct(res)))
        }
        "__sync_channel_recv" => {
            let (cap, closed, mut count, mut data, mut head, tail) =
                match extract_channel_fields(args) {
                    Ok(f) => f,
                    Err(e) => return Some(Err(e)),
                };
            let has_item = count > 0;
            let mut item = Payload::Integer(0);
            if has_item {
                if let Payload::Array(ref arr) = data {
                    if (head as usize) < arr.len() {
                        item = arr[head as usize].clone();
                    }
                }
                if let Payload::Array(ref mut arr) = data {
                    if (head as usize) < arr.len() {
                        arr[head as usize] = Payload::Integer(0);
                    }
                }
                head = (head + 1) % cap;
                count -= 1;
            }
            let mut res = HashMap::new();
            res.insert(
                "chan".to_string(),
                EntropicState::Valid(Payload::Struct(build_channel_struct(
                    cap, closed, count, data, head, tail,
                ))),
            );
            res.insert(
                "ok".to_string(),
                EntropicState::Valid(Payload::Bool(has_item)),
            );
            res.insert("val".to_string(), EntropicState::Valid(item));
            Some(Ok(Payload::Struct(res)))
        }
        "__sync_channel_close" => {
            let (cap, _, count, data, head, tail) =
                match extract_channel_fields(args) {
                    Ok(f) => f,
                    Err(e) => return Some(Err(e)),
                };
            Some(Ok(Payload::Struct(build_channel_struct(
                cap, true, count, data, head, tail,
            ))))
        }
        "__sync_channel_is_closed" => {
            let (_, closed, ..) = match extract_channel_fields(args) {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Payload::Bool(closed)))
        }
        "__sync_channel_len" => {
            let (_, _, count, ..) = match extract_channel_fields(args) {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Payload::Integer(count)))
        }
        "__sync_channel_is_full" => {
            let (cap, _, count, ..) = match extract_channel_fields(args) {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Payload::Bool(count >= cap)))
        }
        "__sync_channel_is_empty" => {
            let (_, _, count, ..) = match extract_channel_fields(args) {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok(Payload::Bool(count == 0)))
        }
        _ => None,
    }
}

fn extract_channel_fields(
    args: &[Payload],
) -> Result<(i64, bool, i64, Payload, i64, i64), TemporalError> {
    match args.first() {
        Some(Payload::Struct(f)) => {
            let cap = match f.get("cap") {
                Some(EntropicState::Valid(Payload::Integer(i))) => *i,
                _ => 1,
            };
            let closed = matches!(
                f.get("closed"),
                Some(EntropicState::Valid(Payload::Bool(true)))
            );
            let count = match f.get("count") {
                Some(EntropicState::Valid(Payload::Integer(i))) => *i,
                _ => 0,
            };
            let data = match f.get("data") {
                Some(EntropicState::Valid(p)) => p.clone(),
                _ => Payload::Array(vec![]),
            };
            let head = match f.get("head") {
                Some(EntropicState::Valid(Payload::Integer(i))) => *i,
                _ => 0,
            };
            let tail = match f.get("tail") {
                Some(EntropicState::Valid(Payload::Integer(i))) => *i,
                _ => 0,
            };
            Ok((cap, closed, count, data, head, tail))
        }
        _ => Err(TemporalError::TypeMismatch(
            "Channel op expects Channel".into(),
        )),
    }
}

fn build_channel_struct(
    cap: i64,
    closed: bool,
    count: i64,
    data: Payload,
    head: i64,
    tail: i64,
) -> HashMap<String, EntropicState> {
    let mut m = HashMap::new();
    m.insert(
        "cap".to_string(),
        EntropicState::Valid(Payload::Integer(cap)),
    );
    m.insert(
        "closed".to_string(),
        EntropicState::Valid(Payload::Bool(closed)),
    );
    m.insert(
        "count".to_string(),
        EntropicState::Valid(Payload::Integer(count)),
    );
    m.insert("data".to_string(), EntropicState::Valid(data));
    m.insert(
        "head".to_string(),
        EntropicState::Valid(Payload::Integer(head)),
    );
    m.insert(
        "tail".to_string(),
        EntropicState::Valid(Payload::Integer(tail)),
    );
    m
}
