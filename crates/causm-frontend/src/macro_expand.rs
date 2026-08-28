//! Declarative macro expansion pass.
//!
//! Run this pass on the parsed `Program` BEFORE analysis and lowering.
//! It:
//!   1. Collects all `Statement::MacroDef` nodes into a registry (removed from the AST).
//!   2. Replaces every `Statement::Expression(Expression::Call { routine: "__macro_call__<name>", args })`
//!      with the expanded statements produced by text-substituting args into the macro body template
//!      and re-parsing the result.
//!
//! Macro args are matched positionally to the macro's `params` list. Each `$name` in the body
//! template is replaced by the corresponding raw argument string.

use causm_core::{MacroParam, Program, SpannedStatement, Statement};
use std::collections::HashMap;

/// Registry of known macros: name → (params, body_template).
pub type MacroRegistry = HashMap<String, (Vec<MacroParam>, String)>;

/// Expand all macros in `program` in-place. Collects definitions first, then substitutes calls.
pub fn expand_program(program: &mut Program) {
    let mut registry = MacroRegistry::new();

    // Pass 1: collect MacroDef statements (strip them from the AST).
    for timeline in &mut program.timelines {
        collect_macros_in_block(&mut timeline.statements, &mut registry);
    }

    // Pass 2: expand macro calls.
    for timeline in &mut program.timelines {
        expand_block(&mut timeline.statements, &registry);
    }
}

fn collect_macros_in_block(
    stmts: &mut Vec<SpannedStatement>,
    registry: &mut MacroRegistry,
) {
    let mut i = 0;
    while i < stmts.len() {
        if let Statement::MacroDef {
            name,
            params,
            body_template,
        } = &stmts[i].stmt
        {
            registry.insert(name.clone(), (params.clone(), body_template.clone()));
            stmts.remove(i);
            // don't advance i; the next element has shifted into position i
        } else {
            // Recurse into nested blocks
            collect_in_statement(&mut stmts[i].stmt, registry);
            i += 1;
        }
    }
}

fn collect_in_statement(stmt: &mut Statement, registry: &mut MacroRegistry) {
    match stmt {
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            collect_macros_in_block(then_branch, registry);
            if let Some(b) = else_branch {
                collect_macros_in_block(b, registry);
            }
        }
        Statement::While { body, .. }
        | Statement::Loop { body, .. }
        | Statement::LoopTick { body }
        | Statement::LoopOn { body, .. } => {
            collect_macros_in_block(body, registry);
        }
        Statement::RoutineDef { body, .. } => {
            collect_macros_in_block(body, registry);
        }
        _ => {}
    }
}

fn expand_block(stmts: &mut Vec<SpannedStatement>, registry: &MacroRegistry) {
    let mut i = 0;
    while i < stmts.len() {
        // Recurse into nested blocks first
        expand_in_statement(&mut stmts[i].stmt, registry);

        // Check if this is a macro call sentinel
        if let Statement::Expression(causm_core::Expression::Call {
            routine,
            args,
        }) = &stmts[i].stmt
        {
            if let Some(macro_name) = routine.strip_prefix("__macro_call__") {
                if let Some((params, body_template)) = registry.get(macro_name) {
                    let span = stmts[i].span.clone();
                    let expanded = expand_macro_call(
                        macro_name,
                        params,
                        body_template,
                        args.as_slice(),
                        span,
                    );
                    // Replace the single call statement with the (possibly multiple) expanded stmts
                    stmts.remove(i);
                    for (j, new_stmt) in expanded.into_iter().enumerate() {
                        stmts.insert(i + j, new_stmt);
                    }
                    // Don't advance i — re-process from same position (expanded stmts may call macros)
                    continue;
                }
            }
        }
        i += 1;
    }
}

fn expand_in_statement(stmt: &mut Statement, registry: &MacroRegistry) {
    match stmt {
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            expand_block(then_branch, registry);
            if let Some(b) = else_branch {
                expand_block(b, registry);
            }
        }
        Statement::While { body, .. }
        | Statement::Loop { body, .. }
        | Statement::LoopTick { body }
        | Statement::LoopOn { body, .. } => {
            expand_block(body, registry);
        }
        Statement::RoutineDef { body, .. } => {
            expand_block(body, registry);
        }
        _ => {}
    }
}

fn expand_macro_call(
    name: &str,
    params: &[MacroParam],
    body_template: &str,
    args: &[causm_core::Expression],
    span: causm_core::Span,
) -> Vec<SpannedStatement> {
    // Substitute $param_name → arg string for each positional parameter
    let mut expanded = body_template.to_string();
    for (param, arg_expr) in params.iter().zip(args.iter()) {
        let arg_str = expr_to_string(arg_expr);
        expanded = expanded.replace(&format!("${}", param.name), &arg_str);
    }

    // Wrap in a minimal timeline block so the parser can parse it as statements
    let wrapped = format!("@0ms: {{\n{}\n}}", expanded);
    match crate::parser::parse_causm(&wrapped) {
        Ok(prog) => prog
            .timelines
            .into_iter()
            .flat_map(|tb| tb.statements)
            .map(|mut ss| {
                ss.span = span.clone();
                ss
            })
            .collect(),
        Err(e) => {
            eprintln!("[macro] Expansion of '{}' failed to parse: {}", name, e);
            Vec::new()
        }
    }
}

/// Convert an expression back to a source string for substitution.
/// For `Expression::Literal` (raw text captured by the parser) this is direct.
fn expr_to_string(expr: &causm_core::Expression) -> String {
    match expr {
        causm_core::Expression::Literal(s) => s.clone(),
        causm_core::Expression::Identifier(s) => s.clone(),
        causm_core::Expression::Integer(i) => i.to_string(),
        causm_core::Expression::Boolean(b) => b.to_string(),
        _ => format!("{:?}", expr),
    }
}
