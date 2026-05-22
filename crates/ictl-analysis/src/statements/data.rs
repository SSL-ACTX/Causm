use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use crate::expression::analyze_expression;
use ictl_core::*;

#[allow(non_snake_case, unused_variables)]
impl EntropicAnalyzer {
    pub(crate) fn Assignment(
        &mut self,
        target: &String,
        mutable: &bool,
        var_type: &Option<TypeName>,
        expr: &Expression,
    ) -> Result<(), SemanticError> {
        {
            let branch = self.branch_contexts.get(&self.current_branch).unwrap();
            if branch.leased.contains(target)
                || branch.lease_bindings.contains(target)
            {
                return Err(
                    self.annotate(SemanticErrorKind::LeaseViolation(target.clone()))
                );
            }
        }
        analyze_expression(self, expr)?;
        let inferred_type = crate::expression::infer_expression_type(self, expr)?;

        if let Some(explicit_type_name) = var_type {
            let explicit_type =
                ictl_core::types::Type::from_typename(explicit_type_name);
            if !self.types_compatible(&explicit_type, &inferred_type) {
                return Err(self.annotate(SemanticErrorKind::TypeMismatch(
                    format!(
                        "explicit type {:?} does not match expression type {:?}",
                        explicit_type, inferred_type
                    ),
                )));
            }
        } else if let Some(existing_type) = self.get_variable_type(target) {
            if !self.types_compatible(&existing_type, &inferred_type) {
                return Err(self.annotate(SemanticErrorKind::TypeMismatch(
                    format!(
                        "reassignment of {} requires matching type {:?}, got {:?}",
                        target, existing_type, inferred_type
                    ),
                )));
            }
        }

        let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
        if *mutable {
            branch.mutables.insert(target.clone());
        }
        branch.types.insert(target.clone(), inferred_type);
        branch.produced.insert(target.clone());
        branch.consumed.remove(target);
        branch
            .instantiated_at
            .insert(target.clone(), branch.accumulated_cost);
        Ok(())
    }

    pub(crate) fn FieldUpdate(
        &mut self,
        target: &Expression,
        field: &str,
        value: &Expression,
    ) -> Result<(), SemanticError> {
        if let Expression::Identifier(name) = target {
            let branch = self.branch_contexts.get(&self.current_branch).unwrap();
            if branch.leased.contains(name) || branch.lease_bindings.contains(name) {
                return Err(
                    self.annotate(SemanticErrorKind::LeaseViolation(name.clone()))
                );
            }
            if branch.consumed.contains(name) {
                return Err(self.annotate(SemanticErrorKind::CrossBranchViolation(
                    name.clone(),
                )));
            }
        } else {
            analyze_expression(self, target)?;
        }
        analyze_expression(self, value)?;
        Ok(())
    }
}
