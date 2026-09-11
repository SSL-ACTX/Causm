use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IntCmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BoolExpr {
    Lit(bool),
    Var(String),
    Not(Arc<BoolExpr>),
    And(Vec<BoolExpr>),
    Or(Vec<BoolExpr>),
    Ite(Arc<BoolExpr>, Arc<BoolExpr>, Arc<BoolExpr>),
    Eq(Arc<BoolExpr>, Arc<BoolExpr>),
    Implies(Arc<BoolExpr>, Arc<BoolExpr>),
    IntCmp(IntCmpOp, Arc<IntExpr>, Arc<IntExpr>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IntExpr {
    Lit(u64),
    Var(String),
    Add(Vec<IntExpr>),
    Ite(Arc<BoolExpr>, Arc<IntExpr>, Arc<IntExpr>),
}

impl BoolExpr {
    #[allow(clippy::should_implement_trait)]
    pub fn not(self) -> Self {
        match self {
            BoolExpr::Lit(b) => BoolExpr::Lit(!b),
            BoolExpr::Not(inner) => (*inner).clone(),
            other => BoolExpr::Not(Arc::new(other)),
        }
    }

    pub fn and(args: Vec<BoolExpr>) -> Self {
        let mut simplified = Vec::new();
        for arg in args {
            match arg {
                BoolExpr::Lit(true) => continue,
                BoolExpr::Lit(false) => return BoolExpr::Lit(false),
                BoolExpr::And(sub) => simplified.extend(sub),
                other => simplified.push(other),
            }
        }
        if simplified.is_empty() {
            BoolExpr::Lit(true)
        } else if simplified.len() == 1 {
            simplified.into_iter().next().unwrap()
        } else {
            BoolExpr::And(simplified)
        }
    }

    pub fn or(args: Vec<BoolExpr>) -> Self {
        let mut simplified = Vec::new();
        for arg in args {
            match arg {
                BoolExpr::Lit(false) => continue,
                BoolExpr::Lit(true) => return BoolExpr::Lit(true),
                BoolExpr::Or(sub) => simplified.extend(sub),
                other => simplified.push(other),
            }
        }
        if simplified.is_empty() {
            BoolExpr::Lit(false)
        } else if simplified.len() == 1 {
            simplified.into_iter().next().unwrap()
        } else {
            BoolExpr::Or(simplified)
        }
    }

    pub fn ite(
        cond: BoolExpr,
        then_branch: BoolExpr,
        else_branch: BoolExpr,
    ) -> Self {
        match cond {
            BoolExpr::Lit(true) => then_branch,
            BoolExpr::Lit(false) => else_branch,
            _ if then_branch == else_branch => then_branch,
            _ => BoolExpr::Ite(
                Arc::new(cond),
                Arc::new(then_branch),
                Arc::new(else_branch),
            ),
        }
    }

    pub fn implies(self, other: BoolExpr) -> Self {
        match (&self, &other) {
            (BoolExpr::Lit(false), _) => BoolExpr::Lit(true),
            (BoolExpr::Lit(true), _) => other,
            (_, BoolExpr::Lit(true)) => BoolExpr::Lit(true),
            _ => BoolExpr::Implies(Arc::new(self), Arc::new(other)),
        }
    }

    pub fn eq(self, other: BoolExpr) -> Self {
        if self == other {
            BoolExpr::Lit(true)
        } else {
            BoolExpr::Eq(Arc::new(self), Arc::new(other))
        }
    }
}

impl IntExpr {
    pub fn add(args: Vec<IntExpr>) -> Self {
        let mut const_sum = 0u64;
        let mut simplified = Vec::new();
        for arg in args {
            match arg {
                IntExpr::Lit(val) => {
                    const_sum = const_sum.saturating_add(val);
                }
                IntExpr::Add(sub) => {
                    for s in sub {
                        if let IntExpr::Lit(v) = s {
                            const_sum = const_sum.saturating_add(v);
                        } else {
                            simplified.push(s);
                        }
                    }
                }
                other => simplified.push(other),
            }
        }
        if const_sum > 0 {
            simplified.push(IntExpr::Lit(const_sum));
        }
        if simplified.is_empty() {
            IntExpr::Lit(0)
        } else if simplified.len() == 1 {
            simplified.into_iter().next().unwrap()
        } else {
            IntExpr::Add(simplified)
        }
    }

    pub fn ite(cond: BoolExpr, then_branch: IntExpr, else_branch: IntExpr) -> Self {
        match cond {
            BoolExpr::Lit(true) => then_branch,
            BoolExpr::Lit(false) => else_branch,
            _ if then_branch == else_branch => then_branch,
            _ => IntExpr::Ite(
                Arc::new(cond),
                Arc::new(then_branch),
                Arc::new(else_branch),
            ),
        }
    }
}
