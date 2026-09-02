// src/solver.rs

/// Abstract interface representing SMT-solver operations and SMT-logic types
/// for Causm's temporal and entropic analysis.
pub trait SolverBackend: Sized {
    /// Boolean solver expression type.
    type Bool: Clone;
    /// Infinite-precision integer solver expression type.
    type Int: Clone;

    /// Creates a new solver instance.
    fn new() -> Self;

    /// Resets the solver.
    fn reset(&mut self);

    /// Checks the satisfiability of the active solver assertions.
    /// Returns `true` if satisfiable, `false` otherwise.
    fn check(&mut self) -> bool;

    /// Pushes a new solver assertion scope (backtracking point).
    fn push(&mut self);

    /// Pops a specified number of solver assertion scopes.
    fn pop(&mut self, num: u32);

    /// Asserts a boolean condition in the current solver context.
    fn assert(&mut self, cond: &Self::Bool);

    /// Declares a boolean constant (variable) in the solver.
    fn bool_const(&mut self, name: &str) -> Self::Bool;

    /// Creates a constant boolean value.
    fn bool_from_bool(&mut self, val: bool) -> Self::Bool;

    /// Logically negates a boolean expression.
    fn bool_not(&mut self, a: &Self::Bool) -> Self::Bool;

    /// Conjunction of boolean expressions.
    fn bool_and(&mut self, args: &[&Self::Bool]) -> Self::Bool;

    /// Disjunction of boolean expressions.
    fn bool_or(&mut self, args: &[&Self::Bool]) -> Self::Bool;

    /// If-then-else selection for boolean expressions.
    fn bool_ite(
        &mut self,
        cond: &Self::Bool,
        then: &Self::Bool,
        orelse: &Self::Bool,
    ) -> Self::Bool;

    /// Equivalence of two boolean expressions.
    fn bool_eq(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool;

    /// Implication (a implies b).
    fn bool_implies(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool;

    /// Declares an integer constant in the solver.
    fn int_const(&mut self, name: &str) -> Self::Int;

    /// Creates a constant integer value from a `u64`.
    fn int_from_u64(&mut self, val: u64) -> Self::Int;

    /// Sums integer expressions.
    fn int_add(&mut self, args: &[&Self::Int]) -> Self::Int;

    /// Less than comparison of two integer expressions.
    fn int_lt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;

    /// Less than or equal comparison of two integer expressions.
    fn int_le(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;

    /// Greater than comparison of two integer expressions.
    fn int_gt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;

    /// Greater than or equal comparison of two integer expressions.
    fn int_ge(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;

    /// Equality comparison of two integer expressions.
    fn int_eq(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool;

    /// If-then-else selection for integer expressions.
    fn int_ite(
        &mut self,
        cond: &Self::Bool,
        then: &Self::Int,
        orelse: &Self::Int,
    ) -> Self::Int;

    /// Evaluates an integer expression in the current solver model, returning its `u64` value if possible.
    fn eval_u64(&mut self, val: &Self::Int) -> Option<u64>;
}

/// A string-based mock solver backend for unit testing the verifier.
#[cfg(test)]
pub struct MockBackend {
    pub assertions: Vec<String>,
    pub scopes: Vec<Vec<String>>,
    pub check_result: bool,
    pub val_evaluations: std::collections::HashMap<String, u64>,
}

#[cfg(test)]
impl SolverBackend for MockBackend {
    type Bool = String;
    type Int = String;

    fn new() -> Self {
        Self {
            assertions: Vec::new(),
            scopes: Vec::new(),
            check_result: false,
            val_evaluations: std::collections::HashMap::new(),
        }
    }

    fn reset(&mut self) {
        self.assertions.clear();
        self.scopes.clear();
    }

    fn check(&mut self) -> bool {
        self.check_result
    }

    fn push(&mut self) {
        self.scopes.push(self.assertions.clone());
    }

    fn pop(&mut self, num: u32) {
        for _ in 0..num {
            if let Some(scope) = self.scopes.pop() {
                self.assertions = scope;
            }
        }
    }

    fn assert(&mut self, cond: &Self::Bool) {
        self.assertions.push(cond.clone());
    }

    fn bool_const(&mut self, name: &str) -> Self::Bool {
        name.to_string()
    }

    fn bool_from_bool(&mut self, val: bool) -> Self::Bool {
        val.to_string()
    }

    fn bool_not(&mut self, a: &Self::Bool) -> Self::Bool {
        format!("(not {})", a)
    }

    fn bool_and(&mut self, args: &[&Self::Bool]) -> Self::Bool {
        let joined: Vec<String> = args.iter().map(|&s| s.clone()).collect();
        format!("(and {})", joined.join(" "))
    }

    fn bool_or(&mut self, args: &[&Self::Bool]) -> Self::Bool {
        let joined: Vec<String> = args.iter().map(|&s| s.clone()).collect();
        format!("(or {})", joined.join(" "))
    }

    fn bool_ite(
        &mut self,
        cond: &Self::Bool,
        then: &Self::Bool,
        orelse: &Self::Bool,
    ) -> Self::Bool {
        format!("(ite {} {} {})", cond, then, orelse)
    }

    fn bool_eq(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        format!("(= {} {})", a, b)
    }

    fn bool_implies(&mut self, a: &Self::Bool, b: &Self::Bool) -> Self::Bool {
        format!("(=> {} {})", a, b)
    }

    fn int_const(&mut self, name: &str) -> Self::Int {
        name.to_string()
    }

    fn int_from_u64(&mut self, val: u64) -> Self::Int {
        val.to_string()
    }

    fn int_add(&mut self, args: &[&Self::Int]) -> Self::Int {
        let joined: Vec<String> = args.iter().map(|&s| s.clone()).collect();
        format!("(+ {})", joined.join(" "))
    }

    fn int_lt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        format!("(< {} {})", a, b)
    }

    fn int_le(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        format!("(<= {} {})", a, b)
    }

    fn int_gt(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        format!("(> {} {})", a, b)
    }

    fn int_ge(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        format!("(>= {} {})", a, b)
    }

    fn int_eq(&mut self, a: &Self::Int, b: &Self::Int) -> Self::Bool {
        format!("(= {} {})", a, b)
    }

    fn int_ite(
        &mut self,
        cond: &Self::Bool,
        then: &Self::Int,
        orelse: &Self::Int,
    ) -> Self::Int {
        format!("(ite {} {} {})", cond, then, orelse)
    }

    fn eval_u64(&mut self, val: &Self::Int) -> Option<u64> {
        self.val_evaluations.get(val).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_backend_assertions() {
        let mut solver = MockBackend::new();
        solver.check_result = true;

        let a = solver.bool_const("a");
        let b = solver.bool_const("b");
        let and_expr = solver.bool_and(&[&a, &b]);
        solver.assert(&and_expr);

        assert!(solver.check());
        assert_eq!(solver.assertions.len(), 1);
        assert_eq!(solver.assertions[0], "(and a b)");
    }
}
