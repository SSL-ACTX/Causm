use super::*;
use causm_analysis::analyzer::SemanticErrorKind;

#[test]
fn test_entropy_linear_use_after_consume_failure() {
    let source = r#"
        @0ms: {
            let msg = "payload"
            yield msg
            let second = msg
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for use-after-consume"
    );
    let err_str = format!("{}", res.unwrap_err());
    assert!(
        err_str.contains("E0001")
            || err_str.contains("Use-After-Consume")
            || err_str.contains("msg"),
        "Unexpected error: {}",
        err_str
    );
}

#[test]
fn test_entropy_double_consume_conflict_failure() {
    let source = r#"
        @0ms: {
            let token = 100
            yield token
            yield token
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for double-consume"
    );
    let err_str = format!("{}", res.unwrap_err());
    assert!(
        err_str.contains("E0007")
            || err_str.contains("Double Consume Conflict")
            || err_str.contains("token"),
        "Unexpected error: {}",
        err_str
    );
}

#[test]
fn test_entropy_field_decay_struct_access_failure() {
    let source = r#"
        @0ms: {
            let s = struct { a = "A", b = "B" }
            let x = s.a
            let y = s
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for struct access after field decay"
    );
    let err_str = format!("{}", res.unwrap_err());
    assert!(
        err_str.contains("E0003")
            || err_str.contains("decayed")
            || err_str.contains("consumed")
            || err_str.contains("s"),
        "Unexpected error: {}",
        err_str
    );
}

#[test]
fn test_entropy_egc_unconsumed_variable_failure() {
    let source = r#"
        @0ms: {
            let leftover = 42
        }
    "#;
    let res = compile_program_with_egc(source);
    assert!(
        res.is_err(),
        "Expected compilation failure under EGC for unconsumed variable"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::UnconsumedVariable(ref name) => {
            assert_eq!(name, "leftover");
        }
        _ => panic!("Expected UnconsumedVariable, got: {:?}", err.kind),
    }
}

#[test]
fn test_entropy_entanglement_cross_decay_failure() {
    let source = r#"
        @0ms: {
            let a = 10
            let b = 20
            entangle(a, b)
            yield a
            let c = b
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for entangled decay access"
    );
    let err_str = format!("{}", res.unwrap_err());
    assert!(
        err_str.contains("E0006")
            || err_str.contains("Entanglement Decay")
            || err_str.contains("b"),
        "Unexpected error: {}",
        err_str
    );
}

#[test]
fn test_entropy_cross_branch_split_collision_failure() {
    let source = r#"
        @main: {
            let shared_key = 123
            split main into [branchA, branchB]
        }
        @branchA: {
            yield shared_key
        }
        @branchB: {
            let peek = shared_key + 1
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for cross-branch split collision"
    );
    let err_str = format!("{}", res.unwrap_err());
    assert!(
        err_str.contains("E0008")
            || err_str.contains("Cross-Branch Collision")
            || err_str.contains("shared_key"),
        "Unexpected error: {}",
        err_str
    );
}

#[test]
fn test_entropy_speculative_leak_rollback_failure() {
    let source = r#"
        @main: {
            speculate (max 50ms) {
                let temp_secret = 777
            }
            let leaked = temp_secret
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for speculative leak"
    );
    let err_str = format!("{}", res.unwrap_err());
    assert!(
        err_str.contains("E0009")
            || err_str.contains("Speculative")
            || err_str.contains("temp_secret")
            || err_str.contains("Undefined"),
        "Unexpected error: {}",
        err_str
    );
}
