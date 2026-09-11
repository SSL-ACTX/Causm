use super::*;
use causm_analysis::analyzer::SemanticErrorKind;

#[test]
fn test_type_mismatch_explicit_annotation_assignment() {
    let source = r#"
        @0ms: {
            let x: Int = "string_value"
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for explicit type annotation mismatch"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::TypeMismatch(ref msg) => {
            assert!(msg.contains("Int") && msg.contains("String"));
        }
        _ => panic!("Expected TypeMismatch, got: {:?}", err.kind),
    }
}

#[test]
fn test_type_mismatch_boolean_condition_in_if() {
    let source = r#"
        @0ms: {
            if "not_a_bool" {
                let x = 1
            }
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for non-bool if condition"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::TypeMismatch(ref msg) => {
            assert!(
                msg.contains("Bool")
                    || msg.contains("String")
                    || msg.contains("condition")
            );
        }
        _ => panic!("Expected TypeMismatch, got: {:?}", err.kind),
    }
}

#[test]
fn test_type_undefined_variable_reference() {
    let source = r#"
        @0ms: {
            let x = nonexistent_identifier + 1
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for undefined variable"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::UndefinedVariable(ref name) => {
            assert_eq!(name, "nonexistent_identifier");
        }
        _ => panic!("Expected UndefinedVariable, got: {:?}", err.kind),
    }
}

#[test]
fn test_type_invalid_structural_field_access_on_primitive() {
    let source = r#"
        @0ms: {
            let num = 42
            let x = num.unknown_prop
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compilation failure for field access on primitive"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::TypeMismatch(ref msg) => {
            assert!(msg.contains("field access on non-struct"));
        }
        _ => panic!(
            "Expected TypeMismatch for non-struct field access, got: {:?}",
            err.kind
        ),
    }
}
