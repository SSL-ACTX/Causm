use crate::analyzer::{EntropicAnalyzer, SemanticError};
use causm_core::*;

#[allow(non_snake_case, unused_variables)]
impl EntropicAnalyzer {
    pub(crate) fn TypeDecl(
        &mut self,
        name: &str,
        fields: &std::collections::HashMap<String, TypeFieldDef>,
        decay_after_ms: &Option<u64>,
        scoped_branch: &Option<String>,
    ) -> Result<(), SemanticError> {
        let mut schema = std::collections::HashMap::new();
        for (field_name, field_def) in fields {
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
        self.type_decls.insert(name.to_string(), fields.clone());
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
