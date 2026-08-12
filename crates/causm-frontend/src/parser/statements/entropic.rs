use crate::parser::expressions::parse_expression;
use crate::parser::statements::parse_statement;
use crate::parser::Rule;
use causm_core::*;
use pest::iterators::Pair;

pub fn parse_entropic_stmt(pair: Pair<Rule>) -> Statement {
    match pair.as_rule() {
        Rule::match_entropy_stmt | Rule::match_entropy_expr => {
            let mut inner = pair.into_inner();
            let target = parse_expression(inner.next().unwrap());
            let mut valid_branch = None;
            let mut decayed_branch = None;
            let mut pending_branch = None;
            let mut consumed_branch = None;

            for element in inner {
                if element.as_rule() == Rule::entropy_branch {
                    let text = element.as_str().trim();
                    let is_valid = text.starts_with("Valid");
                    let is_decayed = text.starts_with("Decayed");
                    let is_pending = text.starts_with("Pending");
                    let is_consumed = text.starts_with("Consumed");

                    let mut branch_inner = element.into_inner();
                    let first_child = branch_inner.next();
                    let mut pattern = DecayedPattern::Binding(String::new());
                    let mut body_pair = None;

                    if let Some(pat_pair) = first_child {
                        if pat_pair.as_rule() == Rule::identifier_or_pattern {
                            let inner_pat = pat_pair.into_inner().next().unwrap();
                            match inner_pat.as_rule() {
                                Rule::identifier => {
                                    pattern = DecayedPattern::Binding(
                                        inner_pat.as_str().to_string(),
                                    );
                                }
                                Rule::field_pattern_list => {
                                    let mut fields =
                                        std::collections::HashMap::new();
                                    for field_pat in inner_pat.into_inner() {
                                        let mut field_pat_inner =
                                            field_pat.into_inner();
                                        let field_name = field_pat_inner
                                            .next()
                                            .unwrap()
                                            .as_str()
                                            .to_string();
                                        let val_pair =
                                            field_pat_inner.next().unwrap();
                                        let pattern_val = if val_pair.as_rule()
                                            == Rule::entropic_state_name
                                        {
                                            PatternValue::State(
                                                val_pair.as_str().to_string(),
                                            )
                                        } else {
                                            PatternValue::Expr(parse_expression(
                                                val_pair,
                                            ))
                                        };
                                        fields.insert(field_name, pattern_val);
                                    }
                                    pattern = DecayedPattern::Fields(fields);
                                }
                                _ => unreachable!(),
                            }
                            body_pair = branch_inner.next();
                        } else {
                            body_pair = Some(pat_pair);
                        }
                    }

                    let body = body_pair
                        .map(|stmt_block| {
                            stmt_block
                                .into_inner()
                                .filter_map(|s| s.into_inner().next())
                                .map(parse_statement)
                                .collect()
                        })
                        .unwrap_or_default();

                    if is_valid {
                        valid_branch = Some((pattern, body));
                    } else if is_decayed {
                        decayed_branch = Some((pattern, body));
                    } else if is_pending {
                        pending_branch = Some((pattern, body));
                    } else if is_consumed {
                        consumed_branch = Some(body);
                    }
                }
            }
            Statement::MatchEntropy {
                target,
                valid_branch,
                decayed_branch,
                pending_branch,
                consumed_branch,
            }
        }
        Rule::entangle_stmt => {
            let variables = pair
                .into_inner()
                .next()
                .map(|p| p.into_inner().map(|id| id.as_str().to_string()).collect())
                .unwrap_or_default();
            Statement::Entangle { variables }
        }
        _ => unreachable!(),
    }
}
