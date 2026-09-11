use super::*;
use causm_analysis::analyzer::SemanticErrorKind;

#[test]
fn test_temporal_use_after_decay_lifetime_failure() {
    let source = r#"
        @0ms: {
            let @decayed(10ms) token = 42
        }
        @50ms: {
            let read = token
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for use-after-decay"
    );
    let err_str = format!("{}", res.unwrap_err());
    assert!(
        err_str.contains("E0002")
            || err_str.contains("Decayed")
            || err_str.contains("decay")
            || err_str.contains("token"),
        "Unexpected error: {}",
        err_str
    );
}

#[test]
fn test_temporal_causal_rewind_past_commit_paradox() {
    let source = r#"
        @0ms: {
            anchor Checkpoint
            yield 1
            rewind_to Checkpoint
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for causal rewind paradox"
    );
    let err_str = format!("{}", res.unwrap_err());
    assert!(
        err_str.contains("E0005")
            || err_str.contains("Causal")
            || err_str.contains("Checkpoint"),
        "Unexpected error: {}",
        err_str
    );
}

#[test]
fn test_temporal_lease_mutation_prohibited_failure() {
    let source = r#"
        @0ms: {
            let state = struct { val = 10 }
            lease view = state for 50ms {
                state = struct { val = 20 }
            }
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for mutating leased variable"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::LeaseViolation(ref name) => {
            assert_eq!(name, "state");
        }
        SemanticErrorKind::EntropiusDiagnostic(ref diag) => {
            assert!(diag.contains("state") || diag.contains("E0004"));
        }
        _ => panic!("Expected LeaseViolation, got: {:?}", err.kind),
    }
}

#[test]
fn test_temporal_nested_lease_prohibited_failure() {
    let source = r#"
        @0ms: {
            let state = struct { val = 10 }
            lease view1 = state for 50ms {
                lease view2 = state for 20ms {
                    let x = 1
                }
            }
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for nested lease"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::NestedLeasing(ref name) => {
            assert_eq!(name, "state");
        }
        _ => panic!("Expected NestedLeasing, got: {:?}", err.kind),
    }
}

#[test]
fn test_temporal_lease_duration_exceeded_wcet_failure() {
    let source = r#"
        @0ms: {
            let state = struct { val = 10 }
            lease view = state for 2ms {
                let a = 1
                let b = 2
                let c = 3
                let d = 4
            }
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure when lease block exceeds duration"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::LeaseDurationExceeded(wcet, duration) => {
            assert!(
                wcet > duration,
                "WCET {} should exceed duration {}",
                wcet,
                duration
            );
        }
        _ => panic!("Expected LeaseDurationExceeded, got: {:?}", err.kind),
    }
}

#[test]
fn test_temporal_illegal_lease_control_flow_failure() {
    let source = r#"
        @0ms: {
            let state = struct { val = 10 }
            lease view = state for 50ms {
                return
            }
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for return within lease"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::IllegalLeaseControlFlow => {}
        _ => panic!("Expected IllegalLeaseControlFlow, got: {:?}", err.kind),
    }
}
