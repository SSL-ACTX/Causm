use crate::analyzer::{
    EntropicAnalyzer, RoutineInfo, SemanticError, SemanticErrorKind,
};
use crate::expression::infer_expression_type;
use causm_core::types::Type;
use causm_core::Expression;

pub enum MethodTargetResolution {
    EnumConstructor,
    StaticRoutine(String, RoutineInfo),
    InterfaceMethod(String, causm_core::InterfaceMethod),
    StructMethod(String, RoutineInfo),
}

/// Resolve a method call `target.method(args)` to its static or dynamic target routine.
pub fn resolve_method_call(
    analyzer: &EntropicAnalyzer,
    target: &Expression,
    method: &str,
) -> Result<MethodTargetResolution, SemanticError> {
    if let Expression::Identifier(ref name) = *target {
        let is_enum_type = analyzer
            .branch_contexts
            .get(&analyzer.current_branch)
            .map(|st| st.custom_types.contains_key(name))
            .unwrap_or(false);
        if is_enum_type {
            return Ok(MethodTargetResolution::EnumConstructor);
        }
    }

    if let Some(ns) = crate::expression::inference::get_static_target_path(target) {
        let static_routine_name = format!("{}.{}", ns, method);
        let is_local_var = analyzer
            .branch_contexts
            .get(&analyzer.current_branch)
            .map(|st| st.types.contains_key(&ns))
            .unwrap_or(false);
        if !is_local_var && analyzer.routines.contains_key(&static_routine_name) {
            let info = analyzer.routines.get(&static_routine_name).unwrap().clone();
            return Ok(MethodTargetResolution::StaticRoutine(
                static_routine_name,
                info,
            ));
        }
    }

    let target_type = infer_expression_type(analyzer, target)?;
    let struct_name = match &target_type {
        Type::Custom(name) => name.clone(),
        Type::Struct(s) => {
            let mut matched = None;
            for (tname, fields) in &analyzer.type_decls {
                if fields.len() == s.fields.len()
                    && fields.keys().all(|k| s.fields.contains_key(k))
                {
                    matched = Some(tname.clone());
                    break;
                }
            }
            matched.ok_or_else(|| {
                analyzer.annotate(SemanticErrorKind::TypeMismatch(
                    "method call target must be a custom type instance".into(),
                ))
            })?
        }
        _ => {
            return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                "method call target must be a custom type instance".into(),
            )));
        }
    };

    if analyzer.interfaces.contains_key(&struct_name) {
        let methods = &analyzer.interfaces[&struct_name];
        let interface_method = methods
            .iter()
            .find(|m| m.name == method)
            .ok_or_else(|| {
                analyzer.annotate(SemanticErrorKind::TypeMismatch(format!(
                    "unknown method {} on interface {}",
                    method, struct_name
                )))
            })?
            .clone();
        return Ok(MethodTargetResolution::InterfaceMethod(
            struct_name,
            interface_method,
        ));
    }

    let mut current_struct = struct_name.clone();
    let mut resolved = None;
    loop {
        let routine_name = format!("{}.{}", current_struct, method);
        if let Some(info) = analyzer.routines.get(&routine_name) {
            resolved = Some((routine_name, info.clone()));
            break;
        }
        if let Some(parent) = analyzer.struct_extends.get(&current_struct) {
            current_struct = parent.clone();
        } else {
            break;
        }
    }

    let (resolved_name, info) = resolved.ok_or_else(|| {
        analyzer.annotate(SemanticErrorKind::EntropyMismatch(format!(
            "unknown method {} on type {}",
            method, struct_name
        )))
    })?;

    Ok(MethodTargetResolution::StructMethod(resolved_name, info))
}
