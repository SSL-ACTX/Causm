use crate::parser::Rule;
use causm_core::*;
use pest::iterators::Pair;
use std::collections::HashMap;

pub fn parse_type_name(pair: Pair<Rule>) -> TypeName {
    match pair.as_rule() {
        Rule::type_name => {
            let inner = pair.into_inner().next().unwrap();
            parse_type_name(inner)
        }
        Rule::union_type => {
            let mut parts = Vec::new();
            for chunk in pair.into_inner() {
                parts.push(parse_type_name(chunk));
            }
            if parts.len() == 1 {
                parts.into_iter().next().unwrap()
            } else {
                TypeName::Union(parts)
            }
        }
        Rule::optional_type => {
            let mut inner = pair.into_inner();
            let base = inner.next().unwrap();
            let base_type = parse_type_name(base);
            if let Some(opt) = inner.next() {
                if opt.as_str() == "?" {
                    TypeName::Optional(Box::new(base_type))
                } else {
                    base_type
                }
            } else {
                base_type
            }
        }
        Rule::base_type => {
            let text = pair.as_str().trim();
            match text {
                "int" => TypeName::Builtin(BuiltinType::Integer),
                "i8" => TypeName::Builtin(BuiltinType::I8),
                "i16" => TypeName::Builtin(BuiltinType::I16),
                "i32" => TypeName::Builtin(BuiltinType::I32),
                "i64" => TypeName::Builtin(BuiltinType::I64),
                "u8" => TypeName::Builtin(BuiltinType::U8),
                "u16" => TypeName::Builtin(BuiltinType::U16),
                "u32" => TypeName::Builtin(BuiltinType::U32),
                "u64" => TypeName::Builtin(BuiltinType::U64),
                "float" => TypeName::Builtin(BuiltinType::Float),
                "f32" => TypeName::Builtin(BuiltinType::F32),
                "f64" => TypeName::Builtin(BuiltinType::F64),
                "bool" => TypeName::Builtin(BuiltinType::Bool),
                "string" => TypeName::Builtin(BuiltinType::String),
                "struct" => TypeName::Builtin(BuiltinType::Struct),
                "topology" => TypeName::Builtin(BuiltinType::Topology),
                "array" => TypeName::Builtin(BuiltinType::Array),
                _ => {
                    let mut inner = pair.into_inner();
                    if let Some(first) = inner.next() {
                        let name = first.as_str().to_string();
                        if let Some(params_pair) = inner.next() {
                            let params = params_pair
                                .into_inner()
                                .map(|p| {
                                    let is_duration = p.as_str().contains("ms");
                                    let inner_p = p.into_inner().next().unwrap();
                                    match inner_p.as_rule() {
                                        Rule::type_name => {
                                            TypeParam::Type(parse_type_name(inner_p))
                                        }
                                        Rule::amount => {
                                            let text = inner_p.as_str();
                                            let val =
                                                text.parse::<u64>().unwrap_or(0);
                                            if is_duration {
                                                TypeParam::Duration(val)
                                            } else {
                                                TypeParam::Amount(val)
                                            }
                                        }
                                        _ => TypeParam::Amount(0),
                                    }
                                })
                                .collect();
                            TypeName::Generic(name, params)
                        } else {
                            TypeName::Custom(name)
                        }
                    } else {
                        TypeName::Custom(text.to_string())
                    }
                }
            }
        }
        Rule::identifier => TypeName::Custom(pair.as_str().to_string()),
        _ => TypeName::Custom(pair.as_str().to_string()),
    }
}

pub fn parse_manifest(pair: Pair<Rule>) -> Manifest {
    let mut manifest = Manifest::default();
    for item in pair.into_inner() {
        match item.as_rule() {
            Rule::resource_decl => {
                let mut inner = item.into_inner();
                let res_type = inner.next().map(|p| p.as_str()).unwrap_or("");
                let amount = inner
                    .next()
                    .map(|p| p.as_str().parse::<u64>().unwrap_or(0))
                    .unwrap_or(0);
                let unit = inner.next().map(|p| p.as_str());

                match res_type {
                    "cpu" => {
                        let multiplier = match unit {
                            Some("ms") => 1,
                            _ => 1,
                        };
                        manifest.cpu_budget_ms = Some(amount * multiplier);
                    }
                    "memory" => {
                        let multiplier = match unit {
                            Some("KB") => 1024,
                            Some("MB") => 1024 * 1024,
                            Some("bytes") => 1,
                            _ => 1,
                        };
                        manifest.memory_budget_bytes = Some(amount * multiplier);
                    }
                    _ => {
                        manifest
                            .resource_budgets
                            .insert(res_type.to_string(), amount);
                    }
                }
            }
            Rule::slice_decl => {
                let amount = item
                    .into_inner()
                    .next()
                    .and_then(|p| p.as_str().parse::<u64>().ok())
                    .unwrap_or(0);
                manifest.slice_ms = Some(amount);
            }
            Rule::require_decl => manifest.capabilities.push(parse_capability(item)),
            _ => {}
        }
    }
    manifest
}

pub fn parse_capability(pair: Pair<Rule>) -> Capability {
    let mut inner = pair.into_inner();
    let path = inner
        .next()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    let mut parameters = HashMap::new();
    if let Some(params_pair) = inner.next() {
        for p in params_pair.into_inner() {
            let mut p_inner = p.into_inner();
            if let (Some(k), Some(v)) = (p_inner.next(), p_inner.next()) {
                parameters
                    .insert(k.as_str().to_string(), v.as_str().replace("\"", ""));
            }
        }
    }
    Capability { path, parameters }
}

pub fn parse_resolution_strategy(pair: Pair<Rule>) -> ResolutionStrategy {
    match pair.as_rule() {
        Rule::resolution_strategy => {
            if let Some(inner) = pair.clone().into_inner().next() {
                match inner.as_rule() {
                    Rule::topology_union => {
                        let mut rules = HashMap::new();
                        let mut default = Box::new(ResolutionStrategy::Decay);
                        let mut on_invalid = None;
                        for rule_group in inner.into_inner() {
                            match rule_group.as_rule() {
                                Rule::resolution_rules => {
                                    for rule_pair in rule_group.into_inner() {
                                        let mut r_inner = rule_pair.into_inner();
                                        if let (Some(k_pair), Some(v_pair)) =
                                            (r_inner.next(), r_inner.next())
                                        {
                                            let k = k_pair
                                                .as_str()
                                                .trim_matches('"')
                                                .to_string();
                                            let v =
                                                parse_resolution_strategy(v_pair);
                                            if k == "_" {
                                                default = Box::new(v);
                                            } else {
                                                rules.insert(k, v);
                                            }
                                        }
                                    }
                                }
                                Rule::on_invalid_clause => {
                                    let clauses = rule_group.into_inner();
                                    let mut branch = None;
                                    let mut anchor = None;
                                    for pkt in clauses {
                                        match pkt.as_rule() {
                                            Rule::identifier => {
                                                if branch.is_none() {
                                                    branch = Some(
                                                        pkt.as_str().to_string(),
                                                    );
                                                } else {
                                                    anchor = Some(
                                                        pkt.as_str().to_string(),
                                                    );
                                                }
                                            }
                                            Rule::rewind_clause => {
                                                for inner in pkt.into_inner() {
                                                    if inner.as_rule()
                                                        == Rule::identifier
                                                    {
                                                        if branch.is_none() {
                                                            branch = Some(
                                                                inner
                                                                    .as_str()
                                                                    .to_string(),
                                                            );
                                                        } else {
                                                            anchor = Some(
                                                                inner
                                                                    .as_str()
                                                                    .to_string(),
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    if let (Some(branch), Some(anchor)) =
                                        (branch, anchor)
                                    {
                                        on_invalid =
                                            Some(CausalReversion { branch, anchor });
                                    }
                                }
                                _ => {}
                            }
                        }
                        ResolutionStrategy::TopologyUnion {
                            key_rules: rules,
                            default,
                            on_invalid,
                        }
                    }
                    Rule::topology_intersect => {
                        let mut rules = HashMap::new();
                        let mut default = Box::new(ResolutionStrategy::Decay);
                        for rule_pair in
                            inner.into_inner().flat_map(|rr| rr.into_inner())
                        {
                            let mut r_inner = rule_pair.into_inner();
                            if let (Some(k_pair), Some(v_pair)) =
                                (r_inner.next(), r_inner.next())
                            {
                                let k =
                                    k_pair.as_str().trim_matches('"').to_string();
                                let v = parse_resolution_strategy(v_pair);
                                if k == "_" {
                                    default = Box::new(v);
                                } else {
                                    rules.insert(k, v);
                                }
                            }
                        }
                        ResolutionStrategy::TopologyIntersect {
                            key_rules: rules,
                            default,
                            on_invalid: None,
                        }
                    }
                    _ => {
                        if inner.as_rule() == Rule::identifier {
                            ResolutionStrategy::Priority(inner.as_str().to_string())
                        } else {
                            let value = inner.as_str();
                            if value == "first_wins" {
                                ResolutionStrategy::FirstWins
                            } else if value == "decay" {
                                ResolutionStrategy::Decay
                            } else if value == "auto" {
                                ResolutionStrategy::Auto
                            } else if let Some(inner_p) =
                                value.strip_prefix("priority(")
                            {
                                if let Some(branch_name) = inner_p.strip_suffix(")")
                                {
                                    ResolutionStrategy::Priority(
                                        branch_name.to_string(),
                                    )
                                } else {
                                    ResolutionStrategy::Priority(value.to_string())
                                }
                            } else {
                                ResolutionStrategy::Custom(value.to_string())
                            }
                        }
                    }
                }
            } else {
                let value = pair.as_str().trim();
                if value == "first_wins" {
                    ResolutionStrategy::FirstWins
                } else if value == "decay" {
                    ResolutionStrategy::Decay
                } else if value == "auto" {
                    ResolutionStrategy::Auto
                } else if let Some(inner_p) = value.strip_prefix("priority(") {
                    if let Some(branch_name) = inner_p.strip_suffix(")") {
                        ResolutionStrategy::Priority(branch_name.to_string())
                    } else {
                        ResolutionStrategy::Priority(value.to_string())
                    }
                } else {
                    ResolutionStrategy::Custom(value.to_string())
                }
            }
        }
        _ => ResolutionStrategy::Custom(pair.as_str().to_string()),
    }
}

pub fn parse_reconcile_clause(pair: Pair<Rule>) -> MergeResolution {
    assert_eq!(pair.as_rule(), Rule::reconcile_clause);
    let is_auto = pair.as_str().contains("auto");
    let mut rules = HashMap::new();
    for child in pair.into_inner() {
        if child.as_rule() == Rule::resolution_rules {
            for rule in child.into_inner() {
                let mut r_inner = rule.into_inner();
                if let (Some(k), Some(v)) = (r_inner.next(), r_inner.next()) {
                    let key = k.as_str().trim_matches('"').to_string();
                    let strat = parse_resolution_strategy(v);
                    rules.insert(key, strat);
                }
            }
        }
    }
    MergeResolution {
        rules,
        auto: is_auto,
        fallback: None,
        taking_ms: None,
    }
}

pub fn parse_duration_limit(pair: Pair<Rule>) -> u64 {
    assert_eq!(pair.as_rule(), Rule::duration_limit);
    let str_val = pair.as_str().trim_matches(|c| c == '(' || c == ')');
    let mut num_str = String::new();
    let mut unit_str = String::new();
    for c in str_val.chars() {
        if c.is_ascii_digit() {
            num_str.push(c);
        } else if c.is_ascii_alphabetic() {
            unit_str.push(c);
        }
    }
    let num = num_str.parse::<u64>().unwrap_or(0);
    if unit_str.contains("ns") {
        num / 1_000_000
    } else if unit_str.contains("us") {
        num / 1000
    } else if unit_str.contains("ms") {
        num
    } else if unit_str.ends_with('s')
        && !unit_str.contains("taking")
        && !unit_str.contains("deadline")
    {
        num * 1000
    } else {
        num
    }
}

pub fn parse_duration_to_ms(str_val: &str) -> u64 {
    let s = str_val.trim();
    if s.ends_with("ns") {
        s.trim_end_matches("ns").parse::<u64>().unwrap_or(0) / 1_000_000
    } else if s.ends_with("us") {
        s.trim_end_matches("us").parse::<u64>().unwrap_or(0) / 1000
    } else if s.ends_with("ms") {
        s.trim_end_matches("ms").parse::<u64>().unwrap_or(0)
    } else if s.ends_with('s') {
        s.trim_end_matches('s').parse::<u64>().unwrap_or(0) * 1000
    } else {
        s.parse::<u64>().unwrap_or(0)
    }
}
