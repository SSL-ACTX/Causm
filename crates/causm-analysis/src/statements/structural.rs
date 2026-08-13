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
            self.struct_extends
                .insert(name.to_string(), base_name.clone());
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
        extends: &[String],
        methods: &[causm_core::InterfaceMethod],
    ) -> Result<(), SemanticError> {
        let mut resolved_methods = Vec::new();
        for base in extends {
            if let Some(base_methods) = self.interfaces.get(base) {
                resolved_methods.extend(base_methods.clone());
            } else {
                return Err(self.annotate(
                    crate::analyzer::SemanticErrorKind::TypeMismatch(format!(
                        "Interface {} extends undefined interface {}",
                        name, base
                    )),
                ));
            }
        }
        resolved_methods.extend(methods.to_vec());
        self.interfaces.insert(name.to_owned(), resolved_methods);
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

    pub(crate) fn DirectiveBlock(
        &mut self,
        directives: &[BlockDirective],
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        let old_use_z3 = self.use_z3;
        let old_entropy_mode = self.entropy_mode;

        for dir in directives {
            match dir {
                BlockDirective::NoZ3 => self.use_z3 = false,
                BlockDirective::Chaos => self.entropy_mode = EntropyMode::Chaos,
                BlockDirective::Deterministic => {
                    self.entropy_mode = EntropyMode::Deterministic
                }
            }
        }

        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }

        self.use_z3 = old_use_z3;
        self.entropy_mode = old_entropy_mode;
        Ok(())
    }
}
