use causm_frontend::parser;

#[test]
fn test_syntax_missing_identifier_in_let_failure() {
    let source = r#"
        @0ms: {
            let = 123
        }
    "#;
    let res = parser::parse_causm(source);
    assert!(
        res.is_err(),
        "Expected parser error on missing identifier in let"
    );
    let err_msg = format!("{}", res.unwrap_err());
    assert!(
        err_msg.contains("Expected identifier after 'let'"),
        "Unexpected: {}",
        err_msg
    );
}

#[test]
fn test_syntax_invalid_let_identifier_failure() {
    let source = r#"
        @0ms: {
            let 123 = 456
        }
    "#;
    let res = parser::parse_causm(source);
    assert!(
        res.is_err(),
        "Expected parser error on non-identifier in let"
    );
    let err_msg = format!("{}", res.unwrap_err());
    assert!(
        err_msg.contains("Expected identifier after 'let'"),
        "Unexpected: {}",
        err_msg
    );
}

#[test]
fn test_syntax_invalid_import_target_failure() {
    let source = r#"
        @0ms: {
            import =
        }
    "#;
    let res = parser::parse_causm(source);
    assert!(
        res.is_err(),
        "Expected parser error on invalid import target"
    );
    let err_msg = format!("{}", res.unwrap_err());
    assert!(
        err_msg.contains("Expected string path in import"),
        "Unexpected: {}",
        err_msg
    );
}

#[test]
fn test_syntax_unexpected_binary_operator_operand_failure() {
    let source = r#"
        @0ms: {
            let x = 1 + *
        }
    "#;
    let res = parser::parse_causm(source);
    assert!(
        res.is_err(),
        "Expected parser error on unexpected binary operand"
    );
    let err_msg = format!("{}", res.unwrap_err());
    assert!(
        err_msg.contains("Unexpected token"),
        "Unexpected: {}",
        err_msg
    );
}

#[test]
fn test_syntax_unexpected_token_in_expression_failure() {
    let source = r#"
        @0ms: {
            let x = @
        }
    "#;
    let res = parser::parse_causm(source);
    assert!(
        res.is_err(),
        "Expected parser error on unexpected token '@' in expression"
    );
    let err_msg = format!("{}", res.unwrap_err());
    assert!(
        err_msg.contains("Unexpected token"),
        "Unexpected: {}",
        err_msg
    );
}
