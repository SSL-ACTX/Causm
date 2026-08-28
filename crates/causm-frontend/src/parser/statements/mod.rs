use crate::parser::Rule;
use causm_core::*;
use pest::iterators::Pair;

pub mod control_flow;
pub mod data;
pub mod entropic;
pub mod misc;
pub mod structural;
pub mod temporal;
pub mod utils;

pub(crate) fn parse_statement(pair: Pair<Rule>) -> SpannedStatement {
    let span = Span {
        start: pair.as_span().start(),
        end: pair.as_span().end(),
    };

    let mut attributes = Vec::new();
    let target_pair = if pair.as_rule() == Rule::statement {
        let mut inner_pairs = pair.into_inner().peekable();
        while let Some(p) = inner_pairs.peek() {
            if p.as_rule() == Rule::attribute {
                let attr_p = inner_pairs.next().unwrap();
                attributes.push(parse_attribute(attr_p));
            } else {
                break;
            }
        }
        inner_pairs.next().unwrap()
    } else {
        pair
    };

    let stmt = match target_pair.as_rule() {
        // Structural
        Rule::timeline_block
        | Rule::directive_stmt
        | Rule::isolate_stmt
        | Rule::routine_stmt
        | Rule::require_decl
        | Rule::anchor_stmt
        | Rule::rewind_stmt => structural::parse_structural_stmt(target_pair),

        // Data
        Rule::assignment_stmt
        | Rule::state_stmt
        | Rule::policy_stmt
        | Rule::type_decl
        | Rule::enum_decl
        | Rule::interface_decl
        | Rule::decay_handler_stmt
        | Rule::field_update_stmt => data::parse_data_stmt(target_pair),

        // Control Flow
        Rule::if_stmt
        | Rule::if_let_stmt
        | Rule::match_stmt
        | Rule::using_stmt
        | Rule::loop_stmt
        | Rule::while_stmt
        | Rule::for_stmt
        | Rule::for_step_stmt
        | Rule::speculate_stmt
        | Rule::collapse_stmt
        | Rule::select_stmt
        | Rule::match_entropy_stmt
        | Rule::slice_stmt
        | Rule::break_stmt => control_flow::parse_control_flow_stmt(target_pair),

        // Temporal
        Rule::split_stmt | Rule::merge_stmt => {
            temporal::parse_temporal_stmt(target_pair)
        }

        // Entropic
        Rule::lease_stmt | Rule::entangle_stmt => {
            entropic::parse_entropic_stmt(target_pair)
        }

        // Misc
        Rule::print_stmt
        | Rule::debug_stmt
        | Rule::return_stmt
        | Rule::yield_stmt
        | Rule::await_stmt
        | Rule::assert_time_stmt
        | Rule::import_stmt
        | Rule::from_import_stmt
        | Rule::foreign_block_stmt
        | Rule::commit_stmt => misc::parse_misc_stmt(target_pair),

        Rule::expression_stmt => {
            let inner_expr = target_pair.into_inner().next().unwrap();
            Statement::Expression(crate::parser::expressions::parse_expression(
                inner_expr,
            ))
        }
        Rule::macro_def_stmt => misc::parse_macro_def(target_pair),
        Rule::macro_call_stmt => misc::parse_macro_call(target_pair),
        _ => Statement::Expression(crate::parser::expressions::parse_expression(
            target_pair,
        )),
    };

    SpannedStatement {
        stmt,
        span,
        attributes,
    }
}

fn parse_attribute(pair: Pair<Rule>) -> Attribute {
    let span = Span {
        start: pair.as_span().start(),
        end: pair.as_span().end(),
    };
    let mut inner = pair.into_inner();
    let name = inner.next().expect("attribute name").as_str().to_string();
    let mut args = Vec::new();
    if let Some(arg_list) = inner.next() {
        for arg in arg_list.into_inner() {
            let s = arg.as_str().trim().trim_matches('"').to_string();
            args.push(s);
        }
    }

    let kind = match name.as_str() {
        "derive" => AttributeKind::Derive(args),
        "must_use" => AttributeKind::MustUse(args.into_iter().next()),
        "inline" => AttributeKind::Inline,
        "test" => AttributeKind::Test,
        _ => AttributeKind::Custom { name, args },
    };

    Attribute { kind, span }
}

pub fn parse_timeline_block(pair: Pair<Rule>) -> TimelineBlock {
    let mut inner = pair.into_inner().peekable();
    let time_coord_pair = inner.next().expect("Timeline missing time");

    let time = match time_coord_pair.as_rule() {
        Rule::duration_literal => {
            let ms = crate::parser::statements::utils::parse_duration_to_ms(
                time_coord_pair.as_str(),
            );
            TimeCoordinate::Periodic(ms)
        }
        Rule::time_coord => {
            let time_pair = time_coord_pair
                .into_inner()
                .next()
                .expect("Invalid time structure");
            match time_pair.as_rule() {
                Rule::absolute_time => TimeCoordinate::Global(
                    time_pair.as_str().replace("ms", "").parse().unwrap_or(0),
                ),
                Rule::relative_time => TimeCoordinate::Relative(
                    time_pair
                        .as_str()
                        .replace("+", "")
                        .replace("ms", "")
                        .parse()
                        .unwrap_or(0),
                ),
                Rule::branch_name => {
                    TimeCoordinate::Branch(time_pair.as_str().to_string())
                }
                _ => TimeCoordinate::Global(0),
            }
        }
        _ => TimeCoordinate::Global(0),
    };

    let mut no_z3 = false;
    let mut entropy_mode = None;

    while let Some(next_pair) = inner.peek() {
        if next_pair.as_rule() == Rule::timeline_directive {
            let directive_pair = inner.next().unwrap();
            match directive_pair.as_str() {
                "@no_z3" => no_z3 = true,
                "@chaos" => entropy_mode = Some(EntropyMode::Chaos),
                "@deterministic" => entropy_mode = Some(EntropyMode::Deterministic),
                _ => {}
            }
        } else {
            break;
        }
    }

    let mut statements = Vec::new();
    if let Some(block_inner) = inner.next() {
        for stmt_pair in block_inner.into_inner() {
            let spanned = parse_statement(stmt_pair);
            statements.push(spanned);
        }
    }
    TimelineBlock {
        time,
        no_z3,
        entropy_mode,
        statements,
    }
}
