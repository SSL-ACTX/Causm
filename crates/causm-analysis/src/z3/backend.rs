use crate::solver::SolverBackend;
use z3::{ast::Bool as Z3Bool, ast::Int as Z3Int, SatResult, Solver as Z3Solver};

pub struct Z3Backend {
    solver: Z3Solver,
}

impl SolverBackend for Z3Backend {
    type Bool = Z3Bool;
    type Int = Z3Int;

    fn new() -> Self {
        Self {
            solver: Z3Solver::new(),
        }
    }

    fn reset(&mut self) {
        self.solver.reset();
    }

    fn check(&mut self) -> bool {
        self.solver.check() == SatResult::Sat
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
        Z3Bool::new_const(name)
    }

    fn bool_from_bool(&mut self, val: bool) -> Self::Bool {
        Z3Bool::from_bool(val)
    }

    fn bool_not(&mut self, a: &Self::Bool) -> Self::Bool {
        a.not()
    }

    fn bool_and(&mut self, args: &[&Self::Bool]) -> Self::Bool {
        Z3Bool::and(args)
    }

    fn bool_or(&mut self, args: &[&Self::Bool]) -> Self::Bool {
        Z3Bool::or(args)
    }

    fn bool_ite(
        &mut self,
        cond: &Self::Bool,
        then: &Self::Bool,
        orelse: &Self::Bool,
    ) -> Self::Bool {
        cond.ite(then, orelse)
    }

    fn bool_eq(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        a.eq(b)
    }

    fn bool_implies(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        a.implies(b)
    }

    fn int_const(&mut self, name: &str) -> Self::Int {
        Z3Int::new_const(name)
    }

    fn int_from_u64(&mut self, val: u64) -> Self::Int {
        Z3Int::from_u64(val)
    }

    fn int_add(&mut self, args: &[&Self::Int]) -> Self::Int {
        Z3Int::add(args)
    }

    fn int_lt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        a.lt(b)
    }

    fn int_le(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        a.le(b)
    }

    fn int_gt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        a.gt(b)
    }

    fn int_ge(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        a.ge(b)
    }

    fn int_eq(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        a.eq(b)
    }

    fn int_ite(
        &mut self,
        cond: &Self::Bool,
        then: &Self::Int,
        orelse: &Self::Int,
    ) -> Self::Int {
        cond.ite(then, orelse)
    }

    fn eval_u64(&mut self, val: &Self::Int) -> Option<u64> {
        self.solver
            .get_model()
            .and_then(|m| m.eval(val, true))
            .and_then(|v| v.as_u64())
    }
}
