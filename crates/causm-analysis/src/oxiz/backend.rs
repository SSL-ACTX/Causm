use super::ast::{BoolExpr, IntCmpOp, IntExpr};
use super::solver::OxiZSolver;
use crate::solver::SolverBackend;
use std::sync::Arc;

pub struct OxiZBackend {
    solver: OxiZSolver,
}

impl SolverBackend for OxiZBackend {
    type Bool = BoolExpr;
    type Int = IntExpr;

    fn new() -> Self {
        Self {
            solver: OxiZSolver::new(),
        }
    }

    fn reset(&mut self) {
        self.solver.reset();
    }

    fn check(&mut self) -> bool {
        self.solver.check()
    }

    fn push(&mut self) {
        self.solver.push();
    }

    fn pop(&mut self, num: u32) {
        self.solver.pop(num);
    }

    fn assert(&mut self, cond: &Self::Bool) {
        self.solver.assert(cond);
    }

    fn bool_const(&mut self, name: &str) -> Self::Bool {
        BoolExpr::Var(name.to_string())
    }

    fn bool_from_bool(&mut self, val: bool) -> Self::Bool {
        BoolExpr::Lit(val)
    }

    fn bool_not(&mut self, a: &Self::Bool) -> Self::Bool {
        a.clone().not()
    }

    fn bool_and(&mut self, args: &[&Self::Bool]) -> Self::Bool {
        let vec = args.iter().map(|&a| a.clone()).collect();
        BoolExpr::and(vec)
    }

    fn bool_or(&mut self, args: &[&Self::Bool]) -> Self::Bool {
        let vec = args.iter().map(|&a| a.clone()).collect();
        BoolExpr::or(vec)
    }

    fn bool_ite(
        &mut self,
        cond: &Self::Bool,
        then: &Self::Bool,
        orelse: &Self::Bool,
    ) -> Self::Bool {
        BoolExpr::ite(cond.clone(), then.clone(), orelse.clone())
    }

    fn bool_eq(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        a.clone().eq(b.clone())
    }

    fn bool_implies(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        a.clone().implies(b.clone())
    }

    fn int_const(&mut self, name: &str) -> Self::Int {
        IntExpr::Var(name.to_string())
    }

    fn int_from_u64(&mut self, val: u64) -> Self::Int {
        IntExpr::Lit(val)
    }

    fn int_add(&mut self, args: &[&Self::Int]) -> Self::Int {
        let vec = args.iter().map(|&a| a.clone()).collect();
        IntExpr::add(vec)
    }

    fn int_lt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        BoolExpr::IntCmp(IntCmpOp::Lt, Arc::new(a.clone()), Arc::new(b.clone()))
    }

    fn int_le(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        BoolExpr::IntCmp(IntCmpOp::Le, Arc::new(a.clone()), Arc::new(b.clone()))
    }

    fn int_gt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        BoolExpr::IntCmp(IntCmpOp::Gt, Arc::new(a.clone()), Arc::new(b.clone()))
    }

    fn int_ge(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        BoolExpr::IntCmp(IntCmpOp::Ge, Arc::new(a.clone()), Arc::new(b.clone()))
    }

    fn int_eq(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        BoolExpr::IntCmp(IntCmpOp::Eq, Arc::new(a.clone()), Arc::new(b.clone()))
    }

    fn int_ite(
        &mut self,
        cond: &Self::Bool,
        then: &Self::Int,
        orelse: &Self::Int,
    ) -> Self::Int {
        IntExpr::ite(cond.clone(), then.clone(), orelse.clone())
    }

    fn eval_u64(&mut self, val: &Self::Int) -> Option<u64> {
        self.solver.eval_u64(val)
    }
}
