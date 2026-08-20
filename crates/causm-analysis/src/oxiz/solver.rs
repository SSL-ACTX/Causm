use super::ast::{BoolExpr, IntCmpOp, IntExpr};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct OxiZSolver {
    assertions: Vec<BoolExpr>,
    scopes: Vec<usize>,
    model_bools: HashMap<String, bool>,
    model_ints: HashMap<String, u64>,
}

impl OxiZSolver {
    pub fn new() -> Self {
        Self {
            assertions: Vec::new(),
            scopes: Vec::new(),
            model_bools: HashMap::new(),
            model_ints: HashMap::new(),
        }
    }

    pub fn reset(&mut self) {
        self.assertions.clear();
        self.scopes.clear();
        self.model_bools.clear();
        self.model_ints.clear();
    }

    pub fn push(&mut self) {
        self.scopes.push(self.assertions.len());
    }

    pub fn pop(&mut self, num: u32) {
        for _ in 0..num {
            if let Some(prev_len) = self.scopes.pop() {
                self.assertions.truncate(prev_len);
            } else {
                self.assertions.clear();
            }
        }
    }

    pub fn assert(&mut self, cond: &BoolExpr) {
        self.assertions.push(cond.clone());
    }

    pub fn check(&mut self) -> bool {
        let mut flat_assertions = Vec::new();
        for a in &self.assertions {
            flatten_bool(a, &mut flat_assertions);
        }

        let mut bool_env: HashMap<String, bool> = HashMap::new();
        let mut definitions: Vec<(String, BoolExpr)> = Vec::new();
        let mut constraints: Vec<BoolExpr> = Vec::new();

        // 1. Separate definitions (x == expr) while preserving all constraints
        for a in flat_assertions {
            match &a {
                BoolExpr::Lit(true) => continue,
                BoolExpr::Lit(false) => return false,
                BoolExpr::Eq(left, right) => {
                    if let (BoolExpr::Var(name), expr) = (&**left, &**right) {
                        definitions.push((name.clone(), expr.clone()));
                    } else if let (expr, BoolExpr::Var(name)) = (&**left, &**right) {
                        definitions.push((name.clone(), expr.clone()));
                    }
                    constraints.push(a);
                }
                _ => {
                    constraints.push(a);
                }
            }
        }

        let mut int_env: HashMap<String, u64> = HashMap::new();

        // 2. DPLL with unit propagation
        if solve_dpll(&constraints, &definitions, &mut bool_env, &mut int_env) {
            self.model_bools = bool_env;
            self.model_ints = int_env;
            true
        } else {
            false
        }
    }

    pub fn eval_u64(&self, expr: &IntExpr) -> Option<u64> {
        eval_int(expr, &self.model_bools, &self.model_ints)
    }
}

fn propagate(
    constraints: &[BoolExpr],
    definitions: &[(String, BoolExpr)],
    bool_env: &mut HashMap<String, bool>,
) -> bool {
    let mut changed = true;
    while changed {
        changed = false;

        // 1. Propagate definitions
        for (name, expr) in definitions {
            if !bool_env.contains_key(name) {
                if let Some(val) = eval_bool_partial(expr, bool_env) {
                    bool_env.insert(name.clone(), val);
                    changed = true;
                }
            }
        }

        // 2. Propagate unit clauses & implications from constraints
        for c in constraints {
            match c {
                BoolExpr::Var(name) => {
                    if let Some(&val) = bool_env.get(name) {
                        if !val {
                            return false;
                        }
                    } else {
                        bool_env.insert(name.clone(), true);
                        changed = true;
                    }
                }
                BoolExpr::Not(inner) => {
                    if let BoolExpr::Var(name) = &**inner {
                        if let Some(&val) = bool_env.get(name) {
                            if val {
                                return false;
                            }
                        } else {
                            bool_env.insert(name.clone(), false);
                            changed = true;
                        }
                    } else if let Some(false) = eval_bool_partial(c, bool_env) {
                        return false;
                    }
                }
                BoolExpr::Implies(ant, cons) => {
                    match eval_bool_partial(ant, bool_env) {
                        Some(true) => match &**cons {
                            BoolExpr::Var(name) => {
                                if let Some(&val) = bool_env.get(name) {
                                    if !val {
                                        return false;
                                    }
                                } else {
                                    bool_env.insert(name.clone(), true);
                                    changed = true;
                                }
                            }
                            BoolExpr::Not(inner) => {
                                if let BoolExpr::Var(name) = &**inner {
                                    if let Some(&val) = bool_env.get(name) {
                                        if val {
                                            return false;
                                        }
                                    } else {
                                        bool_env.insert(name.clone(), false);
                                        changed = true;
                                    }
                                } else if let Some(false) =
                                    eval_bool_partial(cons, bool_env)
                                {
                                    return false;
                                }
                            }
                            _ => {
                                if let Some(false) =
                                    eval_bool_partial(cons, bool_env)
                                {
                                    return false;
                                }
                            }
                        },
                        Some(false) => {}
                        None => {
                            if let Some(false) = eval_bool_partial(cons, bool_env) {
                                if let BoolExpr::Var(name) = &**ant {
                                    if let Some(&val) = bool_env.get(name) {
                                        if val {
                                            return false;
                                        }
                                    } else {
                                        bool_env.insert(name.clone(), false);
                                        changed = true;
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    if let Some(false) = eval_bool_partial(c, bool_env) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

fn collect_decision_vars(constraints: &[BoolExpr], out: &mut Vec<String>) {
    for c in constraints {
        match c {
            BoolExpr::Or(args) => {
                for a in args {
                    let mut bvars = HashSet::new();
                    let mut ivars = HashSet::new();
                    collect_vars(a, &mut bvars, &mut ivars);
                    for v in bvars {
                        if !out.contains(&v) {
                            out.push(v);
                        }
                    }
                }
            }
            BoolExpr::Ite(cond, _, _) => {
                let mut bvars = HashSet::new();
                let mut ivars = HashSet::new();
                collect_vars(cond, &mut bvars, &mut ivars);
                for v in bvars {
                    if !out.contains(&v) {
                        out.push(v);
                    }
                }
            }
            BoolExpr::Implies(ant, _) => {
                let mut bvars = HashSet::new();
                let mut ivars = HashSet::new();
                collect_vars(ant, &mut bvars, &mut ivars);
                for v in bvars {
                    if !out.contains(&v) {
                        out.push(v);
                    }
                }
            }
            _ => {}
        }
    }
}

fn solve_dpll(
    constraints: &[BoolExpr],
    definitions: &[(String, BoolExpr)],
    bool_env: &mut HashMap<String, bool>,
    int_env: &mut HashMap<String, u64>,
) -> bool {
    let mut current_env = bool_env.clone();
    if !propagate(constraints, definitions, &mut current_env) {
        return false;
    }

    // 1. Pick unassigned condition/decision variables first
    let mut decision_vars = Vec::new();
    collect_decision_vars(constraints, &mut decision_vars);

    let mut next_var = None;
    for v in decision_vars {
        if !current_env.contains_key(&v) && !definitions.iter().any(|(d, _)| d == &v)
        {
            next_var = Some(v);
            break;
        }
    }

    if let Some(var) = next_var {
        // Try true
        let mut try_true_env = current_env.clone();
        try_true_env.insert(var.clone(), true);
        if solve_dpll(constraints, definitions, &mut try_true_env, int_env) {
            *bool_env = try_true_env;
            return true;
        }

        // Try false
        let mut try_false_env = current_env;
        try_false_env.insert(var, false);
        if solve_dpll(constraints, definitions, &mut try_false_env, int_env) {
            *bool_env = try_false_env;
            return true;
        }

        false
    } else {
        // All condition variables decided!
        // Default any remaining unassigned dead-branch variables:
        let mut all_bvars = HashSet::new();
        let mut all_ivars = HashSet::new();
        for c in constraints {
            collect_vars(c, &mut all_bvars, &mut all_ivars);
        }
        for (d, _) in definitions {
            all_bvars.insert(d.clone());
        }
        for v in all_bvars {
            if !current_env.contains_key(&v)
                && !definitions.iter().any(|(d, _)| d == &v)
            {
                current_env.insert(v, false);
            }
        }
        if !propagate(constraints, definitions, &mut current_env) {
            return false;
        }

        // Verify all constraints
        let mut int_constraints = Vec::new();
        for c in constraints {
            match eval_bool_fully(c, &current_env, &mut int_constraints) {
                Some(true) => continue,
                _ => return false,
            }
        }

        if solve_int_constraints(&int_constraints, &current_env, int_env) {
            *bool_env = current_env;
            true
        } else {
            false
        }
    }
}

fn flatten_bool(expr: &BoolExpr, out: &mut Vec<BoolExpr>) {
    match expr {
        BoolExpr::And(args) => {
            for a in args {
                flatten_bool(a, out);
            }
        }
        _ => out.push(expr.clone()),
    }
}

fn collect_vars(
    expr: &BoolExpr,
    bool_vars: &mut HashSet<String>,
    int_vars: &mut HashSet<String>,
) {
    match expr {
        BoolExpr::Lit(_) => {}
        BoolExpr::Var(name) => {
            bool_vars.insert(name.clone());
        }
        BoolExpr::Not(inner) => collect_vars(inner, bool_vars, int_vars),
        BoolExpr::And(args) | BoolExpr::Or(args) => {
            for a in args {
                collect_vars(a, bool_vars, int_vars);
            }
        }
        BoolExpr::Ite(c, t, e) => {
            collect_vars(c, bool_vars, int_vars);
            collect_vars(t, bool_vars, int_vars);
            collect_vars(e, bool_vars, int_vars);
        }
        BoolExpr::Eq(a, b) | BoolExpr::Implies(a, b) => {
            collect_vars(a, bool_vars, int_vars);
            collect_vars(b, bool_vars, int_vars);
        }
        BoolExpr::IntCmp(_, a, b) => {
            collect_int_vars(a, bool_vars, int_vars);
            collect_int_vars(b, bool_vars, int_vars);
        }
    }
}

fn collect_int_vars(
    expr: &IntExpr,
    bool_vars: &mut HashSet<String>,
    int_vars: &mut HashSet<String>,
) {
    match expr {
        IntExpr::Lit(_) => {}
        IntExpr::Var(name) => {
            int_vars.insert(name.clone());
        }
        IntExpr::Add(args) => {
            for a in args {
                collect_int_vars(a, bool_vars, int_vars);
            }
        }
        IntExpr::Ite(c, t, e) => {
            collect_vars(c, bool_vars, int_vars);
            collect_int_vars(t, bool_vars, int_vars);
            collect_int_vars(e, bool_vars, int_vars);
        }
    }
}

fn eval_bool_partial(expr: &BoolExpr, env: &HashMap<String, bool>) -> Option<bool> {
    match expr {
        BoolExpr::Lit(b) => Some(*b),
        BoolExpr::Var(name) => env.get(name).copied(),
        BoolExpr::Not(inner) => eval_bool_partial(inner, env).map(|b| !b),
        BoolExpr::And(args) => {
            let mut all_true = true;
            for a in args {
                match eval_bool_partial(a, env) {
                    Some(false) => return Some(false),
                    Some(true) => {}
                    None => all_true = false,
                }
            }
            if all_true {
                Some(true)
            } else {
                None
            }
        }
        BoolExpr::Or(args) => {
            let mut all_false = true;
            for a in args {
                match eval_bool_partial(a, env) {
                    Some(true) => return Some(true),
                    Some(false) => {}
                    None => all_false = false,
                }
            }
            if all_false {
                Some(false)
            } else {
                None
            }
        }
        BoolExpr::Ite(c, t, e) => match eval_bool_partial(c, env) {
            Some(true) => eval_bool_partial(t, env),
            Some(false) => eval_bool_partial(e, env),
            None => None,
        },
        BoolExpr::Implies(a, b) => match eval_bool_partial(a, env) {
            Some(false) => Some(true),
            Some(true) => eval_bool_partial(b, env),
            None => match eval_bool_partial(b, env) {
                Some(true) => Some(true),
                _ => None,
            },
        },
        BoolExpr::Eq(a, b) => {
            match (eval_bool_partial(a, env), eval_bool_partial(b, env)) {
                (Some(va), Some(vb)) => Some(va == vb),
                _ => None,
            }
        }
        BoolExpr::IntCmp(..) => None,
    }
}

#[derive(Clone)]
enum SimplifiedIntConstraint {
    Cmp(IntCmpOp, IntExpr, IntExpr),
}

fn eval_bool_fully(
    expr: &BoolExpr,
    bool_env: &HashMap<String, bool>,
    int_constraints: &mut Vec<SimplifiedIntConstraint>,
) -> Option<bool> {
    match expr {
        BoolExpr::Lit(b) => Some(*b),
        BoolExpr::Var(name) => bool_env.get(name).copied(),
        BoolExpr::Not(inner) => {
            eval_bool_fully(inner, bool_env, int_constraints).map(|b| !b)
        }
        BoolExpr::And(args) => {
            for a in args {
                if !eval_bool_fully(a, bool_env, int_constraints)? {
                    return Some(false);
                }
            }
            Some(true)
        }
        BoolExpr::Or(args) => {
            for a in args {
                if eval_bool_fully(a, bool_env, int_constraints)? {
                    return Some(true);
                }
            }
            Some(false)
        }
        BoolExpr::Ite(c, t, e) => {
            let cond = eval_bool_fully(c, bool_env, int_constraints)?;
            if cond {
                eval_bool_fully(t, bool_env, int_constraints)
            } else {
                eval_bool_fully(e, bool_env, int_constraints)
            }
        }
        BoolExpr::Implies(a, b) => {
            let ant = eval_bool_fully(a, bool_env, int_constraints)?;
            if !ant {
                Some(true)
            } else {
                eval_bool_fully(b, bool_env, int_constraints)
            }
        }
        BoolExpr::Eq(a, b) => {
            let va = eval_bool_fully(a, bool_env, int_constraints)?;
            let vb = eval_bool_fully(b, bool_env, int_constraints)?;
            Some(va == vb)
        }
        BoolExpr::IntCmp(op, a, b) => {
            let sim_a = simplify_int(a, bool_env);
            let sim_b = simplify_int(b, bool_env);
            int_constraints.push(SimplifiedIntConstraint::Cmp(
                op.clone(),
                sim_a,
                sim_b,
            ));
            Some(true)
        }
    }
}

fn simplify_int(expr: &IntExpr, bool_env: &HashMap<String, bool>) -> IntExpr {
    match expr {
        IntExpr::Lit(v) => IntExpr::Lit(*v),
        IntExpr::Var(name) => IntExpr::Var(name.clone()),
        IntExpr::Add(args) => {
            let simplified =
                args.iter().map(|a| simplify_int(a, bool_env)).collect();
            IntExpr::add(simplified)
        }
        IntExpr::Ite(c, t, e) => {
            let cond = eval_bool_partial(c, bool_env);
            match cond {
                Some(true) => simplify_int(t, bool_env),
                Some(false) => simplify_int(e, bool_env),
                None => IntExpr::Ite(
                    c.clone(),
                    Arc::new(simplify_int(t, bool_env)),
                    Arc::new(simplify_int(e, bool_env)),
                ),
            }
        }
    }
}

fn eval_int(
    expr: &IntExpr,
    bool_env: &HashMap<String, bool>,
    int_env: &HashMap<String, u64>,
) -> Option<u64> {
    match expr {
        IntExpr::Lit(v) => Some(*v),
        IntExpr::Var(name) => int_env.get(name).copied(),
        IntExpr::Add(args) => {
            let mut sum = 0u64;
            for a in args {
                sum = sum.saturating_add(eval_int(a, bool_env, int_env)?);
            }
            Some(sum)
        }
        IntExpr::Ite(c, t, e) => {
            let cond = eval_bool_partial(c, bool_env).unwrap_or(false);
            if cond {
                eval_int(t, bool_env, int_env)
            } else {
                eval_int(e, bool_env, int_env)
            }
        }
    }
}

fn solve_int_constraints(
    constraints: &[SimplifiedIntConstraint],
    bool_env: &HashMap<String, bool>,
    int_env: &mut HashMap<String, u64>,
) -> bool {
    let mut lower_bounds: HashMap<String, u64> = HashMap::new();
    let mut upper_bounds: HashMap<String, u64> = HashMap::new();
    let mut equalities: Vec<(IntExpr, IntExpr)> = Vec::new();

    for c in constraints {
        match c {
            SimplifiedIntConstraint::Cmp(op, left, right) => {
                let l_val = eval_int(left, bool_env, int_env);
                let r_val = eval_int(right, bool_env, int_env);

                if let (Some(l), Some(r)) = (l_val, r_val) {
                    let satisfied = match op {
                        IntCmpOp::Lt => l < r,
                        IntCmpOp::Le => l <= r,
                        IntCmpOp::Gt => l > r,
                        IntCmpOp::Ge => l >= r,
                        IntCmpOp::Eq => l == r,
                    };
                    if !satisfied {
                        return false;
                    }
                    continue;
                }

                if let IntExpr::Var(v) = left {
                    if let Some(r) = r_val {
                        match op {
                            IntCmpOp::Gt | IntCmpOp::Ge => {
                                let min_val = if matches!(op, IntCmpOp::Gt) {
                                    r + 1
                                } else {
                                    r
                                };
                                let entry =
                                    lower_bounds.entry(v.clone()).or_insert(0);
                                *entry = (*entry).max(min_val);
                            }
                            IntCmpOp::Lt | IntCmpOp::Le => {
                                let max_val = if matches!(op, IntCmpOp::Lt) {
                                    if r == 0 {
                                        return false;
                                    }
                                    r - 1
                                } else {
                                    r
                                };
                                let entry = upper_bounds
                                    .entry(v.clone())
                                    .or_insert(u64::MAX);
                                *entry = (*entry).min(max_val);
                            }
                            IntCmpOp::Eq => {
                                let entry_l =
                                    lower_bounds.entry(v.clone()).or_insert(0);
                                *entry_l = (*entry_l).max(r);
                                let entry_u = upper_bounds
                                    .entry(v.clone())
                                    .or_insert(u64::MAX);
                                *entry_u = (*entry_u).min(r);
                            }
                        }
                    }
                } else if let IntExpr::Var(v) = right {
                    if let Some(l) = l_val {
                        match op {
                            IntCmpOp::Lt | IntCmpOp::Le => {
                                let min_val = if matches!(op, IntCmpOp::Lt) {
                                    l + 1
                                } else {
                                    l
                                };
                                let entry =
                                    lower_bounds.entry(v.clone()).or_insert(0);
                                *entry = (*entry).max(min_val);
                            }
                            IntCmpOp::Gt | IntCmpOp::Ge => {
                                let max_val = if matches!(op, IntCmpOp::Gt) {
                                    if l == 0 {
                                        return false;
                                    }
                                    l - 1
                                } else {
                                    l
                                };
                                let entry = upper_bounds
                                    .entry(v.clone())
                                    .or_insert(u64::MAX);
                                *entry = (*entry).min(max_val);
                            }
                            IntCmpOp::Eq => {
                                let entry_l =
                                    lower_bounds.entry(v.clone()).or_insert(0);
                                *entry_l = (*entry_l).max(l);
                                let entry_u = upper_bounds
                                    .entry(v.clone())
                                    .or_insert(u64::MAX);
                                *entry_u = (*entry_u).min(l);
                            }
                        }
                    }
                } else if matches!(op, IntCmpOp::Eq) {
                    equalities.push((left.clone(), right.clone()));
                }
            }
        }
    }

    // Check bound consistency
    for (v, low) in &lower_bounds {
        if let Some(up) = upper_bounds.get(v) {
            if low > up {
                return false;
            }
        }
        int_env.insert(v.clone(), *low);
    }
    for (v, up) in &upper_bounds {
        if !int_env.contains_key(v) {
            int_env.insert(v.clone(), *up);
        }
    }

    // Assign equalities
    for (a, b) in &equalities {
        if let (IntExpr::Var(v), IntExpr::Var(other)) = (a, b) {
            let val = int_env.get(other).copied().unwrap_or(0);
            int_env.insert(v.clone(), val);
        } else if let (IntExpr::Var(v), right) = (a, b) {
            if let Some(val) = eval_int(right, bool_env, int_env) {
                int_env.insert(v.clone(), val);
            }
        }
    }

    true
}
