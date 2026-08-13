use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use crate::expression::analyze_expression;
use causm_core::*;

#[allow(non_snake_case, unused_variables)]
impl EntropicAnalyzer {
    pub(crate) fn Assignment(
        &mut self,
        target: &String,
        mutable: &bool,
        var_type: &Option<TypeName>,
        lifetime: &Option<LifetimeAnnotation>,
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

        let final_type = if let Some(explicit_type_name) = var_type {
            let explicit_type =
                causm_core::types::Type::from_typename(explicit_type_name);
            if !self.types_compatible(&explicit_type, &inferred_type) {
                return Err(self.annotate(SemanticErrorKind::TypeMismatch(
                    format!(
                        "explicit type {:?} does not match expression type {:?}",
                        explicit_type, inferred_type
                    ),
                )));
            }
            if let Expression::StructLit(ref type_name, _) = expr {
                if type_name.borrow().is_none() {
                    if let TypeName::Custom(ref name) = explicit_type_name {
                        *type_name.borrow_mut() = Some(name.clone());
                    }
                }
            }
            explicit_type
        } else if let Some(existing_type) = self.get_variable_type(target) {
            if !self.types_compatible(&existing_type, &inferred_type) {
                return Err(self.annotate(SemanticErrorKind::TypeMismatch(
                    format!(
                        "reassignment of {} requires matching type {:?}, got {:?}",
                        target, existing_type, inferred_type
                    ),
                )));
            }
            if let Expression::StructLit(ref type_name, _) = expr {
                if type_name.borrow().is_none() {
                    if let causm_core::types::Type::Custom(ref name) = existing_type
                    {
                        *type_name.borrow_mut() = Some(name.clone());
                    }
                }
            }
            existing_type
        } else {
            inferred_type
        };

        let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
        if *mutable {
            branch.mutables.insert(target.clone());
        }
        branch.types.insert(target.clone(), final_type);
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

    pub(crate) fn EnumDecl(
        &mut self,
        name: &str,
        variants: &[EnumVariantDef],
    ) -> Result<(), SemanticError> {
        let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
        branch.types.insert(
            name.to_string(),
            causm_core::types::Type::Custom(name.to_string()),
        );
        branch.custom_types.insert(
            name.to_string(),
            causm_core::types::Type::Custom(name.to_string()),
        );
        for variant in variants {
            branch.types.insert(
                variant.name.clone(),
                causm_core::types::Type::Custom(name.to_string()),
            );
        }
        Ok(())
    }
}
