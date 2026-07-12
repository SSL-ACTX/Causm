use crate::analyzer::{EntropicAnalyzer, SemanticError};
use causm_core::*;

#[allow(non_snake_case, unused_variables)]
impl EntropicAnalyzer {
    pub(crate) fn TypeDecl(
        &mut self,
        name: &str,
        extends: &Option<String>,
        fields: &std::collections::HashMap<String, TypeFieldDef>,
        decay_after_ms: &Option<u64>,
        scoped_branch: &Option<String>,
    ) -> Result<(), SemanticError> {
        let mut resolved_fields = std::collections::HashMap::new();
        if let Some(ref base_name) = extends {
            if let Some(base_fields) = self.type_decls.get(base_name) {
                resolved_fields = base_fields.clone();
            }
        }
        for (k, v) in fields {
            resolved_fields.insert(k.clone(), v.clone());
        }

        let mut schema = std::collections::HashMap::new();
        for (field_name, field_def) in &resolved_fields {
            if !field_def.is_const {
                schema.insert(
                    field_name.clone(),
                    causm_core::types::Type::from_typename(&field_def.typ),
                );
            }
        }
        let type_struct =
            causm_core::types::Type::Struct(causm_core::types::StructType {
                fields: schema,
                decay_after_ms: *decay_after_ms,
                scoped_branch: scoped_branch.clone(),
            });
        self.set_custom_type(name, type_struct);
        self.type_decls.insert(name.to_string(), resolved_fields);
        Ok(())
    }

    pub(crate) fn InterfaceDecl(
        &mut self,
        name: &str,
        methods: &[causm_core::InterfaceMethod],
    ) -> Result<(), SemanticError> {
        self.interfaces.insert(name.to_owned(), methods.to_vec());
        Ok(())
    }

    pub(crate) fn DecayHandler(
        &mut self,
        _type_name: &String,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }
        Ok(())
    }
}
