use std::collections::{HashMap, HashSet};

/// Live range of a variable or register over CFG points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRange {
    pub var: String,
    pub def_point: crate::solver::PointIndex,
    pub use_points: Vec<crate::solver::PointIndex>,
    pub is_consumed: bool,
    pub consume_point: Option<crate::solver::PointIndex>,
}

/// Analysis table containing computed live ranges for all bindings across all timelines.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LiveRangeTable {
    pub ranges: HashMap<String, LiveRange>,
    pub live_at_points: HashMap<crate::solver::PointIndex, HashSet<String>>,
}

impl LiveRangeTable {
    pub fn compute_from_facts(facts: &crate::solver::ProgramFacts) -> Self {
        let mut ranges = HashMap::new();
        let mut live_at_points: HashMap<crate::solver::PointIndex, HashSet<String>> =
            HashMap::new();

        for (var, origins) in &facts.var_origins {
            if let Some(def_pt) = origins.iter().next() {
                ranges.insert(
                    var.clone(),
                    LiveRange {
                        var: var.clone(),
                        def_point: def_pt.clone(),
                        use_points: Vec::new(),
                        is_consumed: facts.var_consumes.contains_key(var),
                        consume_point: facts
                            .var_consumes
                            .get(var)
                            .and_then(|c| c.iter().next())
                            .cloned(),
                    },
                );
            }
        }

        for (var, accesses) in &facts.var_accesses {
            if let Some(range) = ranges.get_mut(var) {
                for (pt, _) in accesses {
                    range.use_points.push(pt.clone());
                    live_at_points
                        .entry(pt.clone())
                        .or_default()
                        .insert(var.clone());
                }
            }
        }

        Self {
            ranges,
            live_at_points,
        }
    }
}
