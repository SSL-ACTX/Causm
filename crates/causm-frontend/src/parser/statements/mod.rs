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

    let stmt = match pair.as_rule() {
        // Structural
        Rule::timeline_block
        | Rule::isolate_stmt
        | Rule::routine_stmt
        | Rule::require_decl
        | Rule::anchor_stmt
        | Rule::rewind_stmt
        | Rule::reset_stmt => structural::parse_structural_stmt(pair),

        // Data
        Rule::assignment_stmt
        | Rule::type_decl
        | Rule::decay_handler_stmt
        | Rule::field_update_stmt => data::parse_data_stmt(pair),

        // Control Flow
        Rule::if_stmt
        | Rule::loop_stmt
        | Rule::for_stmt
        | Rule::split_stmt
        | Rule::merge_stmt
        | Rule::select_stmt
        | Rule::split_map_stmt
        | Rule::yield_stmt
        | Rule::break_stmt => control_flow::parse_control_flow_stmt(pair),

        // Temporal
        Rule::watchdog_stmt
        | Rule::assert_time_stmt
        | Rule::slice_stmt
        | Rule::await_stmt
        | Rule::await_chan_stmt
        | Rule::lease_stmt => temporal::parse_temporal_stmt(pair),

        // Entropic
        Rule::match_entropy_stmt | Rule::entangle_stmt => {
            entropic::parse_entropic_stmt(pair)
        }

        // Misc
        Rule::open_chan_stmt
        | Rule::chan_send_stmt
        | Rule::commit_stmt
        | Rule::speculate_stmt
        | Rule::collapse_stmt
        | Rule::speculation_mode_stmt
        | Rule::network_request_stmt
        | Rule::print_stmt
        | Rule::debug_stmt
        | Rule::inspect_stmt => misc::parse_misc_stmt(pair),

        _ => {
            Statement::Expression(crate::parser::expressions::parse_expression(pair))
        }
    };

    SpannedStatement { stmt, span }
}

pub fn parse_timeline_block(pair: Pair<Rule>) -> TimelineBlock {
    let mut inner = pair.into_inner();
    let time_coord_pair = inner.next().expect("Timeline missing time");
    let time_pair = time_coord_pair
        .into_inner()
        .next()
        .expect("Invalid time structure");

    let time = match time_pair.as_rule() {
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
        Rule::branch_name => TimeCoordinate::Branch(time_pair.as_str().to_string()),
        _ => TimeCoordinate::Global(0),
    };

    let mut statements = Vec::new();
    if let Some(block_inner) = inner.next() {
        for stmt_pair in block_inner.into_inner() {
            if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                let spanned = parse_statement(actual_stmt);
                statements.push(spanned);
            }
        }
    }
    TimelineBlock { time, statements }
}
