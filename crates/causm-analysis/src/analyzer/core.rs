use super::types::*;
use causm_core::types::{StructType, Type};
use causm_core::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct EntropicAnalyzer {
    pub branch_contexts: HashMap<String, BranchState>,
    pub current_branch: String,
    pub current_statement: Option<String>,
    pub current_span: Option<causm_core::Span>,
    pub(crate) inspection_depth: usize,
    pub(crate) current_slice_ms: Option<u64>,
    pub source: Option<String>,
    pub(crate) filename: Option<String>,
    pub(crate) capability_stack: Vec<HashMap<String, causm_core::Capability>>,
    pub routines: HashMap<String, RoutineInfo>,
    pub span_states: HashMap<Span, BranchState>,
    pub(crate) type_decls: HashMap<String, HashMap<String, TypeFieldDef>>,
    pub use_z3: bool,
    pub enforce_egc: bool,
    pub in_entropy_match: bool,
    pub(crate) current_routine: Option<String>,
    pub(crate) interfaces: HashMap<String, Vec<causm_core::InterfaceMethod>>,
    pub(crate) struct_extends: HashMap<String, String>,
    pub(crate) merged_branches: HashSet<String>,
    pub analyzed_wcet: std::cell::RefCell<HashMap<String, u64>>,
    pub entropy_mode: causm_core::EntropyMode,
    pub analyzed_routines: HashSet<String>,
}

impl Default for EntropicAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl EntropicAnalyzer {
    pub fn new() -> Self {
        let mut contexts = HashMap::new();
        contexts.insert("main".to_string(), BranchState::default());

        let mut analyzer = Self {
            branch_contexts: contexts,
            current_branch: "main".to_string(),
            current_statement: None,
            current_span: None,
            inspection_depth: 0,
            current_slice_ms: None,
            source: None,
            filename: None,
            capability_stack: Vec::new(),
            routines: HashMap::new(),
            span_states: HashMap::new(),
            type_decls: HashMap::new(),
            use_z3: true,
            enforce_egc: false,
            in_entropy_match: false,
            current_routine: None,
            interfaces: HashMap::new(),
            struct_extends: HashMap::new(),
            merged_branches: HashSet::new(),
            analyzed_wcet: std::cell::RefCell::new(HashMap::new()),
            entropy_mode: causm_core::EntropyMode::Deterministic,
            analyzed_routines: HashSet::new(),
        };
        analyzer.register_intrinsics();
        analyzer
    }

    pub fn analyze_program_with_source(
        &mut self,
        program: &Program,
        source: &str,
        filename: &str,
    ) -> Result<(), SemanticError> {
        self.source = Some(source.to_string());
        self.filename = Some(filename.to_string());
        let result = self.analyze_program(program);
        self.source = None;
        self.filename = None;
        result
    }

    pub(crate) fn annotate(&self, kind: SemanticErrorKind) -> SemanticError {
        let (line, column) =
            if let (Some(span), Some(src)) = (&self.current_span, &self.source) {
                let before = &src[..span.start];
                let ln = before.lines().count() + 1;
                let col = before
                    .lines()
                    .last()
                    .map(|line| line.len() + 1)
                    .unwrap_or(1);
                (Some(ln), Some(col))
            } else {
                (None, None)
            };

        SemanticError {
            kind: Box::new(kind),
            branch: self.current_branch.clone(),
            statement: self.current_statement.clone(),
            file: self.filename.clone(),
            line,
            column,
        }
    }

    pub(crate) fn is_capability_allowed(&self, cap: &str) -> bool {
        self.capability_stack.iter().rev().any(|map| {
            if map.contains_key(cap) {
                return true;
            }
            if cap.contains("[id=") {
                let base = cap.split('[').next().unwrap();
                let wildcard_key = format!("{}[id=*]", base);
                if map.contains_key(&wildcard_key) {
                    return true;
                }
            }
            map.keys()
                .any(|k| k == cap || k.starts_with(&(cap.to_string() + "[id=")))
        })
    }

    pub(crate) fn get_capability(
        &self,
        cap: &str,
    ) -> Option<&causm_core::Capability> {
        self.capability_stack
            .iter()
            .rev()
            .find_map(|map| map.get(cap))
    }

    pub fn register_intrinsics(&mut self) {
        let math_functions = vec![
            ("sqrt", vec![Type::Float], Type::Float),
            ("sin", vec![Type::Float], Type::Float),
            ("cos", vec![Type::Float], Type::Float),
            ("tan", vec![Type::Float], Type::Float),
            ("exp", vec![Type::Float], Type::Float),
            ("ln", vec![Type::Float], Type::Float),
            ("log10", vec![Type::Float], Type::Float),
            ("floor", vec![Type::Float], Type::Float),
            ("ceil", vec![Type::Float], Type::Float),
            ("round", vec![Type::Float], Type::Float),
        ];

        for (name, params, ret) in math_functions {
            self.routines.insert(
                name.to_string(),
                RoutineInfo {
                    params: params
                        .into_iter()
                        .map(|t| (causm_core::ParamMode::Clone, "x".to_string(), t))
                        .collect(),
                    return_type: ret,
                    taking_ms: 1,
                    state_constraint: None,
                    required_capabilities: Vec::new(),
                },
            );
        }

        let collection_intrinsics = vec![
            (
                "push",
                vec![Type::Array(Box::new(Type::Unknown)), Type::Unknown],
                Type::Array(Box::new(Type::Unknown)),
            ),
            (
                "pop",
                vec![Type::Array(Box::new(Type::Unknown))],
                Type::Unknown,
            ),
            (
                "array_push",
                vec![Type::Array(Box::new(Type::Unknown)), Type::Unknown],
                Type::Array(Box::new(Type::Unknown)),
            ),
            (
                "array_slice",
                vec![
                    Type::Array(Box::new(Type::Unknown)),
                    Type::Integer,
                    Type::Integer,
                ],
                Type::Array(Box::new(Type::Unknown)),
            ),
            (
                "string_from_bytes",
                vec![Type::Array(Box::new(Type::Integer))],
                Type::String,
            ),
            ("char_at", vec![Type::String, Type::Integer], Type::Integer),
            (
                "str_slice",
                vec![Type::String, Type::Integer, Type::Integer],
                Type::String,
            ),
        ];

        for (name, params, ret) in collection_intrinsics {
            self.routines.insert(
                name.to_string(),
                RoutineInfo {
                    params: params
                        .into_iter()
                        .map(|t| (causm_core::ParamMode::Peek, "arg".to_string(), t))
                        .collect(),
                    return_type: ret,
                    taking_ms: 1,
                    state_constraint: None,
                    required_capabilities: Vec::new(),
                },
            );
        }
    }

    pub(crate) fn pre_register_program_declarations(&mut self, program: &Program) {
        fn visit_stmts(analyzer: &mut EntropicAnalyzer, stmts: &[SpannedStatement]) {
            for stmt in stmts {
                match &stmt.stmt {
                    Statement::TypeDecl {
                        name,
                        extends,
                        fields,
                        decay_after_ms,
                        auto_drop,
                        scoped_branch,
                    } => {
                        let _ = analyzer.TypeDecl(
                            name,
                            extends,
                            fields,
                            decay_after_ms,
                            auto_drop,
                            scoped_branch,
                        );
                        if let Some(dot_idx) = name.rfind('.') {
                            let bare_name = &name[dot_idx + 1..];
                            let _ = analyzer.TypeDecl(
                                bare_name,
                                extends,
                                fields,
                                decay_after_ms,
                                auto_drop,
                                scoped_branch,
                            );
                        }
                    }
                    Statement::InterfaceDecl {
                        name,
                        extends,
                        methods,
                    } => {
                        let _ = analyzer.InterfaceDecl(name, extends, methods);
                    }
                    Statement::RoutineDef {
                        name,
                        params,
                        return_type,
                        taking_ms,
                        state_constraint,
                        required_capabilities,
                        ..
                    } => {
                        let preliminary_params = params
                            .iter()
                            .map(|p| {
                                let mut param_type = p
                                    .typ
                                    .as_ref()
                                    .map(causm_core::types::Type::from_typename)
                                    .unwrap_or(causm_core::types::Type::Unknown);
                                if p.name == "self" && p.typ.is_none() {
                                    if let Some(dot_idx) = name.find('.') {
                                        let struct_name = &name[..dot_idx];
                                        param_type = causm_core::types::Type::Custom(
                                            struct_name.to_string(),
                                        );
                                    }
                                }
                                (p.mode.clone(), p.name.clone(), param_type)
                            })
                            .collect();
                        let r_info = RoutineInfo {
                            params: preliminary_params,
                            return_type: return_type
                                .as_ref()
                                .map(causm_core::types::Type::from_typename)
                                .unwrap_or(causm_core::types::Type::Unknown),
                            taking_ms: taking_ms.unwrap_or(0),
                            state_constraint: state_constraint.clone(),
                            required_capabilities: required_capabilities.clone(),
                        };
                        analyzer.routines.insert(name.clone(), r_info.clone());
                        let base_name = if let Some(angle_idx) = name.find('<') {
                            if let Some(dot_idx) = name.find('.') {
                                let struct_part = &name[..angle_idx];
                                let method_part = &name[dot_idx..];
                                format!("{}{}", struct_part, method_part)
                            } else {
                                name.clone()
                            }
                        } else {
                            name.clone()
                        };
                        if base_name != *name {
                            analyzer.routines.insert(base_name, r_info);
                        }
                    }
                    Statement::Isolate(iso) => {
                        visit_stmts(analyzer, &iso.body);
                    }
                    Statement::StateDecl {
                        target,
                        var_type,
                        expr,
                    } => {
                        let typ = if let Some(explicit) = var_type {
                            causm_core::types::Type::from_typename(explicit)
                        } else if let Ok(inferred) =
                            crate::expression::infer_expression_type(analyzer, expr)
                        {
                            inferred
                        } else {
                            causm_core::types::Type::Unknown
                        };
                        let branch =
                            analyzer.branch_contexts.get_mut("main").unwrap();
                        branch.mutables.insert(target.clone());
                        branch.types.insert(target.clone(), typ);
                        branch.produced.insert(target.clone());
                    }
                    Statement::RelativisticBlock { body, .. } => {
                        visit_stmts(analyzer, body);
                    }
                    _ => {}
                }
            }
        }

        for block in &program.timelines {
            visit_stmts(self, &block.statements);
        }
    }

    pub fn analyze_program(
        &mut self,
        program: &Program,
    ) -> Result<(), SemanticError> {
        self.branch_contexts.clear();
        self.branch_contexts
            .insert("main".to_string(), BranchState::default());
        self.current_branch = "main".to_string();
        self.current_statement = None;
        self.current_span = None;
        self.inspection_depth = 0;
        self.capability_stack.clear();
        self.routines.clear();
        self.struct_extends.clear();
        self.analyzed_routines.clear();
        self.register_intrinsics();
        self.pre_register_program_declarations(program);

        for block in &program.timelines {
            let old_branch = self.current_branch.clone();
            let old_entropy_mode = self.entropy_mode;
            if let Some(mode) = block.entropy_mode {
                self.entropy_mode = mode;
            }
            match &block.time {
                TimeCoordinate::Branch(id) => {
                    if id != "main" && !self.branch_contexts.contains_key(id) {
                        return Err(self.annotate(
                            SemanticErrorKind::InactiveTimeline(id.clone()),
                        ));
                    }
                    if self.merged_branches.contains(id) {
                        return Err(self.annotate(
                            SemanticErrorKind::InactiveTimeline(id.clone()),
                        ));
                    }
                    self.current_branch = id.clone();
                }
                TimeCoordinate::Global(t) => {
                    let state =
                        self.branch_contexts.get_mut(&self.current_branch).unwrap();
                    if *t > state.accumulated_cost {
                        state.accumulated_cost = *t;
                    }
                }
                TimeCoordinate::Relative(t) => {
                    let state =
                        self.branch_contexts.get_mut(&self.current_branch).unwrap();
                    state.accumulated_cost += *t;
                }
                TimeCoordinate::Periodic(interval_ms) => {
                    let block_cost = crate::statement::estimate_block_cost(
                        self,
                        &block.statements,
                    );
                    if block_cost > *interval_ms {
                        return Err(self.annotate(
                            SemanticErrorKind::PeriodicDeadlineUnachievable(
                                block_cost,
                                *interval_ms,
                            ),
                        ));
                    }
                    let state =
                        self.branch_contexts.get_mut(&self.current_branch).unwrap();
                    state.accumulated_cost += *interval_ms;
                }
            }

            for stmt in &block.statements {
                let old_stmt = self.current_statement.clone();
                let old_span = self.current_span.clone();
                self.current_statement = Some(self.statement_snippet(stmt));
                self.current_span = Some(stmt.span.clone());
                self.analyze_statement(stmt)?;
                self.current_statement = old_stmt;
                self.current_span = old_span;
            }

            if matches!(&block.time, TimeCoordinate::Periodic(_)) {
                // Reset transient local variables at epoch boundary
                let state =
                    self.branch_contexts.get_mut(&self.current_branch).unwrap();
                state.consumed.clear();
            }

            self.current_branch = old_branch;
            self.entropy_mode = old_entropy_mode;
        }

        if self.enforce_egc {
            for state in self.branch_contexts.values() {
                for var in &state.produced {
                    if var != "_"
                        && !var.starts_with('_')
                        && !state.consumed.contains(var)
                    {
                        return Err(self.annotate(
                            SemanticErrorKind::UnconsumedVariable(var.clone()),
                        ));
                    }
                }
            }
        }

        if self.use_z3 {
            let mut verifier = crate::verifier::FormalVerifier::<
                crate::oxiz::OxiZBackend,
            >::new(self);
            verifier.verify(program)?;
        }

        Ok(())
    }

    pub(crate) fn check_available(&self, name: &str) -> Result<(), SemanticError> {
        let state = self.branch_contexts.get(&self.current_branch).unwrap();
        if state.consumed.contains(name) || state.decayed.contains(name) {
            return Err(
                self.annotate(SemanticErrorKind::UseAfterConsume(name.to_string()))
            );
        }
        Ok(())
    }

    pub(crate) fn merge_states(
        &mut self,
        then_end_state: BranchState,
        else_end_state: BranchState,
        reconcile: &Option<MergeResolution>,
    ) -> Result<BranchState, SemanticError> {
        let mut mismatch_vars = Vec::new();
        for var in &then_end_state.consumed {
            if !else_end_state.consumed.contains(var) {
                mismatch_vars.push(var.clone());
            }
        }
        for var in &else_end_state.consumed {
            if !then_end_state.consumed.contains(var) {
                mismatch_vars.push(var.clone());
            }
        }

        if !mismatch_vars.is_empty() {
            let mut covered_by_reconcile = false;
            if let Some(reconcile_rules) = reconcile {
                if !reconcile_rules.auto {
                    covered_by_reconcile = mismatch_vars.iter().all(|name| {
                        if !reconcile_rules.rules.contains_key(name) {
                            return false;
                        }
                        true
                    });
                } else {
                    covered_by_reconcile = true;
                }
            }

            if !covered_by_reconcile {
                return Err(self.annotate(SemanticErrorKind::EntropyMismatch(
                    mismatch_vars.join(", "),
                )));
            }
        }

        let mut merged_types = then_end_state.types.clone();
        let mut type_conflicts = Vec::new();
        for (name, typ) in &else_end_state.types {
            if let Some(existing) = merged_types.get_mut(name) {
                if !self.types_compatible(existing, typ) {
                    type_conflicts.push(name.clone());
                    *existing = causm_core::types::Type::Unknown;
                } else if *existing == causm_core::types::Type::Unknown {
                    *existing = typ.clone();
                }
            } else {
                merged_types.insert(name.clone(), typ.clone());
            }
        }

        if !type_conflicts.is_empty() {
            match reconcile {
                Some(r) if !r.auto => {
                    let uncovered: Vec<_> = type_conflicts
                        .iter()
                        .filter(|n| !r.rules.contains_key(*n))
                        .cloned()
                        .collect();
                    if !uncovered.is_empty() {
                        return Err(self.annotate(
                            SemanticErrorKind::EntropyMismatch(format!(
                                "divergent types for {}",
                                uncovered.join(", ")
                            )),
                        ));
                    }
                }
                _ => {
                    return Err(self.annotate(SemanticErrorKind::EntropyMismatch(
                        format!("divergent types for {}", type_conflicts.join(", ")),
                    )));
                }
            }
        }

        Ok(BranchState {
            consumed: then_end_state
                .consumed
                .union(&else_end_state.consumed)
                .cloned()
                .collect(),
            decayed: then_end_state
                .decayed
                .union(&else_end_state.decayed)
                .cloned()
                .collect(),
            yields: then_end_state
                .yields
                .union(&else_end_state.yields)
                .cloned()
                .collect(),
            mutables: then_end_state
                .mutables
                .union(&else_end_state.mutables)
                .cloned()
                .collect(),
            produced: then_end_state
                .produced
                .union(&else_end_state.produced)
                .cloned()
                .collect(),
            leased: then_end_state
                .leased
                .union(&else_end_state.leased)
                .cloned()
                .collect(),
            lease_bindings: then_end_state
                .lease_bindings
                .union(&else_end_state.lease_bindings)
                .cloned()
                .collect(),
            types: merged_types,
            custom_types: then_end_state.custom_types.clone(),
            accumulated_cost: then_end_state
                .accumulated_cost
                .max(else_end_state.accumulated_cost),
            instantiated_at: {
                let mut merged = then_end_state.instantiated_at.clone();
                for (k, v) in else_end_state.instantiated_at {
                    merged.entry(k).or_insert(v);
                }
                merged
            },
        })
    }

    pub(crate) fn mark_consumed(&mut self, name: &str) -> Result<(), SemanticError> {
        let state = self.branch_contexts.get_mut(&self.current_branch).unwrap();
        if state.mutables.contains(name) {
            return Ok(());
        }
        if state.leased.contains(name) || state.lease_bindings.contains(name) {
            return Err(
                self.annotate(SemanticErrorKind::LeaseViolation(name.to_string()))
            );
        }
        if state.consumed.contains(name) || state.decayed.contains(name) {
            return Err(
                self.annotate(SemanticErrorKind::UseAfterConsume(name.to_string()))
            );
        }
        state.consumed.insert(name.to_string());
        Ok(())
    }

    pub(crate) fn mark_decayed(&mut self, name: &str) -> Result<(), SemanticError> {
        let state = self.branch_contexts.get_mut(&self.current_branch).unwrap();
        if state.mutables.contains(name) {
            return Ok(());
        }
        if state.consumed.contains(name) {
            return Err(
                self.annotate(SemanticErrorKind::UseAfterConsume(name.to_string()))
            );
        }
        state.decayed.insert(name.to_string());
        Ok(())
    }

    pub(crate) fn remove_variable_scope(&mut self, name: &str) {
        let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
        branch.remove_variable_scope(name);
    }

    pub(crate) fn set_variable_type(&mut self, name: &str, vtype: Type) {
        let state = self.branch_contexts.get_mut(&self.current_branch).unwrap();
        state.types.insert(name.to_string(), vtype);
    }

    pub(crate) fn get_variable_type(&self, name: &str) -> Option<Type> {
        self.branch_contexts
            .get(&self.current_branch)
            .and_then(|state| state.types.get(name).cloned())
    }

    pub(crate) fn set_custom_type(&mut self, name: &str, typ: Type) {
        for state in self.branch_contexts.values_mut() {
            state.custom_types.insert(name.to_string(), typ.clone());
        }
    }

    pub(crate) fn get_custom_type(&self, name: &str) -> Option<Type> {
        self.branch_contexts
            .get(&self.current_branch)
            .and_then(|state| state.custom_types.get(name).cloned())
    }

    pub(crate) fn resolve_type(&self, typ: &Type) -> Type {
        match typ {
            Type::Custom(name) => {
                let base_name = name.split('<').next().unwrap_or(name).trim();
                let bare_base = if let Some(dot) = base_name.rfind('.') {
                    &base_name[dot + 1..]
                } else {
                    base_name
                };
                if let Some(fields_map) = self
                    .type_decls
                    .get(base_name)
                    .or_else(|| self.type_decls.get(bare_base))
                    .or_else(|| self.type_decls.get(name))
                {
                    let schema: std::collections::HashMap<String, Type> = fields_map
                        .iter()
                        .filter(|(_, fd)| !fd.is_const)
                        .map(|(k, fd)| (k.clone(), Type::from_typename(&fd.typ)))
                        .collect();
                    return Type::Struct(causm_core::types::StructType {
                        fields: schema,
                        decay_after_ms: None,
                        auto_drop: None,
                        scoped_branch: None,
                    });
                }
                self.get_custom_type(base_name)
                    .or_else(|| self.get_custom_type(bare_base))
                    .or_else(|| self.get_custom_type(name))
                    .unwrap_or_else(|| Type::Custom(name.clone()))
            }
            Type::Struct(s) => {
                let resolved_fields: std::collections::HashMap<String, Type> = s
                    .fields
                    .iter()
                    .map(|(k, v)| (k.clone(), self.resolve_type(v)))
                    .collect();
                Type::Struct(StructType {
                    fields: resolved_fields,
                    decay_after_ms: s.decay_after_ms,
                    auto_drop: s.auto_drop.clone(),
                    scoped_branch: s.scoped_branch.clone(),
                })
            }
            Type::Topology(fields) => {
                let resolved_fields: std::collections::HashMap<String, Type> =
                    fields
                        .iter()
                        .map(|(k, v)| (k.clone(), self.resolve_type(v)))
                        .collect();
                Type::Topology(resolved_fields)
            }
            Type::Array(inner) => Type::Array(Box::new(self.resolve_type(inner))),
            Type::Optional(inner) => {
                Type::Optional(Box::new(self.resolve_type(inner)))
            }
            Type::Union(items) => {
                Type::Union(items.iter().map(|t| self.resolve_type(t)).collect())
            }
            Type::Function {
                params,
                return_type,
            } => Type::Function {
                params: params.iter().map(|p| self.resolve_type(p)).collect(),
                return_type: Box::new(self.resolve_type(return_type)),
            },
            _ => typ.clone(),
        }
    }

    #[allow(dead_code)]
    pub fn format_semantic_error(&self, err: &SemanticError) -> String {
        let mut message = format!("{}", err.kind);
        if let Some(line) = err.line {
            message.push_str(&format!(" at {}:{}", line, err.column.unwrap_or(0)));
        }
        if let Some(stmt) = &err.statement {
            message.push_str(&format!(" in statement: {}", stmt));
        }
        message
    }

    fn custom_struct_compatible(
        &mut self,
        name: &str,
        act_struct: &causm_core::types::StructType,
    ) -> bool {
        if let Some(fields_map) = self.type_decls.get(name).cloned() {
            for k in act_struct.fields.keys() {
                if !fields_map.contains_key(k) {
                    return false;
                }
            }
            for (field_name, field_def) in &fields_map {
                if field_def.is_const {
                    continue;
                }
                if let Some(act_field_type) = act_struct.fields.get(field_name) {
                    let expected_type = Type::from_typename(&field_def.typ);
                    if !self.types_compatible(&expected_type, act_field_type) {
                        return false;
                    }
                } else if field_def.default_value.is_none() {
                    return false;
                }
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn types_compatible(
        &mut self,
        expected: &Type,
        actual: &Type,
    ) -> bool {
        if let Type::Custom(ref exp_name) = expected {
            if exp_name == "any" {
                return true;
            }
            if self.interfaces.contains_key(exp_name.as_str()) {
                if let Type::Custom(ref act_name) = actual {
                    if self
                        .implements_interface(act_name.as_str(), exp_name.as_str())
                    {
                        return true;
                    }
                }
            }
        }
        if let Type::Custom(ref act_name) = actual {
            if act_name == "any" {
                return true;
            }
        }
        if let Type::Custom(name) = expected {
            if let Type::Struct(act_struct) = actual {
                if self.custom_struct_compatible(name, act_struct) {
                    return true;
                }
            }
        }
        if let Type::Custom(name) = actual {
            if let Type::Struct(exp_struct) = expected {
                if self.custom_struct_compatible(name, exp_struct) {
                    return true;
                }
            }
        }

        let expected = self.resolve_type(expected);
        let actual = self.resolve_type(actual);

        if matches!(expected, Type::Unknown) || matches!(actual, Type::Unknown) {
            return true;
        }

        match (&expected, &actual) {
            (
                Type::Integer
                | Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64,
                Type::Integer
                | Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64,
            ) => true,
            (
                Type::Float | Type::F32 | Type::F64,
                Type::Float | Type::F32 | Type::F64,
            ) => true,
            (Type::Bool, Type::Bool) | (Type::String, Type::String) => true,
            (Type::Struct(exp_struct), Type::Struct(act_struct)) => {
                if exp_struct.fields.is_empty() {
                    true
                } else {
                    let mut compatible = true;
                    for (name, exp_field_ty) in &exp_struct.fields {
                        if let Some(act_field_ty) = act_struct.fields.get(name) {
                            if !self.types_compatible(exp_field_ty, act_field_ty) {
                                compatible = false;
                                break;
                            }
                        } else {
                            compatible = false;
                            break;
                        }
                    }
                    compatible
                }
            }
            (Type::Topology(exp_fields), Type::Topology(act_fields)) => {
                if exp_fields.is_empty() {
                    true
                } else {
                    exp_fields == act_fields
                }
            }
            (Type::Array(exp_inner), Type::Array(act_inner)) => {
                self.types_compatible(exp_inner, act_inner)
            }
            (Type::Promise(exp_inner), Type::Promise(act_inner)) => {
                self.types_compatible(exp_inner, act_inner)
            }
            (Type::Optional(exp_inner), Type::Optional(act_inner)) => {
                self.types_compatible(exp_inner, act_inner)
            }
            (Type::Optional(exp_inner), act_ty) => {
                self.types_compatible(exp_inner, act_ty)
            }
            (act_ty, Type::Optional(exp_inner)) => {
                self.types_compatible(act_ty, exp_inner)
            }
            (Type::Union(exp_types), act_ty) => {
                exp_types.iter().any(|t| self.types_compatible(t, act_ty))
            }
            (act_ty, Type::Union(exp_types)) => {
                exp_types.iter().any(|t| self.types_compatible(act_ty, t))
            }
            (
                Type::Function {
                    params: exp_params,
                    return_type: exp_rt,
                },
                Type::Function {
                    params: act_params,
                    return_type: act_rt,
                },
            ) => {
                exp_params.len() == act_params.len()
                    && exp_params
                        .iter()
                        .zip(act_params.iter())
                        .all(|(e, a)| self.types_compatible(e, a))
                    && self.types_compatible(exp_rt, act_rt)
            }
            (Type::Custom(exp_name), Type::Custom(act_name)) => {
                if exp_name == act_name
                    || exp_name == "any"
                    || act_name == "any"
                    || exp_name.split('<').next().unwrap_or(exp_name).trim()
                        == act_name.split('<').next().unwrap_or(act_name).trim()
                    || act_name.starts_with(&format!("{}::", exp_name))
                    || exp_name.starts_with(&format!("{}::", act_name))
                {
                    true
                } else if self.interfaces.contains_key(exp_name.as_str()) {
                    self.implements_interface(act_name.as_str(), exp_name.as_str())
                } else {
                    false
                }
            }
            (Type::Custom(exp_name), Type::Struct(act_struct)) => {
                if let Some(decl_fields) =
                    self.type_decls.get(exp_name.as_str()).cloned()
                {
                    let mut fields_map = std::collections::HashMap::new();
                    for (k, v) in decl_fields {
                        fields_map.insert(k, Type::from_typename(&v.typ));
                    }
                    let exp_struct = causm_core::types::StructType {
                        fields: fields_map,
                        decay_after_ms: None,
                        auto_drop: None,
                        scoped_branch: None,
                    };
                    self.types_compatible(
                        &Type::Struct(exp_struct),
                        &Type::Struct(act_struct.clone()),
                    )
                } else {
                    false
                }
            }
            (Type::Struct(exp_struct), Type::Custom(act_name)) => {
                if let Some(decl_fields) =
                    self.type_decls.get(act_name.as_str()).cloned()
                {
                    let mut fields_map = std::collections::HashMap::new();
                    for (k, v) in decl_fields {
                        fields_map.insert(k, Type::from_typename(&v.typ));
                    }
                    let act_struct = causm_core::types::StructType {
                        fields: fields_map,
                        decay_after_ms: None,
                        auto_drop: None,
                        scoped_branch: None,
                    };
                    self.types_compatible(
                        &Type::Struct(exp_struct.clone()),
                        &Type::Struct(act_struct),
                    )
                } else {
                    false
                }
            }
            (Type::Custom(_), _) => false,
            (_, Type::Custom(_)) => false,
            _ => false,
        }
    }

    pub(crate) fn implements_interface(
        &mut self,
        concrete_name: &str,
        interface_name: &str,
    ) -> bool {
        if !self.interfaces.contains_key(interface_name) {
            return false;
        }
        let interface_methods = self.interfaces[interface_name].clone();
        for im in &interface_methods {
            let mut resolved_method = None;
            let mut current_struct = concrete_name.to_string();
            loop {
                let r_name = format!("{}.{}", current_struct, im.name);
                if self.routines.contains_key(&r_name) {
                    resolved_method = Some(r_name);
                    break;
                }
                if let Some(parent) = self.struct_extends.get(&current_struct) {
                    current_struct = parent.clone();
                } else {
                    break;
                }
            }

            let concrete_method_name = if let Some(resolved) = resolved_method {
                resolved
            } else if let Some(ref default_body) = im.default_body {
                let mut params = im.params.clone();
                if !params.is_empty() && params[0].name == "self" {
                    params[0].typ = None;
                }
                let routine_name = format!("{}.{}", concrete_name, im.name);
                if self
                    .RoutineDef(
                        &routine_name,
                        &params,
                        &im.return_type,
                        &im.taking_ms,
                        &im.state_constraint,
                        &im.required_capabilities,
                        default_body,
                    )
                    .is_err()
                {
                    return false;
                }
                routine_name
            } else {
                return false;
            };

            let (cm_params, cm_return_type, cm_taking_ms) = {
                let cm = &self.routines[&concrete_method_name];
                (cm.params.clone(), cm.return_type.clone(), cm.taking_ms)
            };
            if cm_params.len() != im.params.len() {
                return false;
            }
            if cm_params[0].0 != im.params[0].mode {
                return false;
            }
            for (i, cp_param) in cm_params.iter().enumerate().skip(1) {
                let ip_param = &im.params[i];
                if cp_param.0 != ip_param.mode {
                    return false;
                }
                if let Some(ref typ) = ip_param.typ {
                    let ip_type = Type::from_typename(typ);
                    if !self.types_compatible(&ip_type, &cp_param.2) {
                        return false;
                    }
                }
            }
            if let Some(ref rt) = im.return_type {
                let im_rt = Type::from_typename(rt);
                if !self.types_compatible(&im_rt, &cm_return_type) {
                    return false;
                }
            }
            if let Some(im_budget) = im.taking_ms {
                if cm_taking_ms > im_budget {
                    return false;
                }
            }
        }
        true
    }

    fn statement_snippet(&self, stmt: &SpannedStatement) -> String {
        match &stmt.stmt {
            Statement::Assignment { target, expr, .. } => {
                format!("let {} = {}", target, self.expr_snippet(expr))
            }
            Statement::Split { parent, branches } => {
                format!("split {} into [{}]", parent, branches.join(","))
            }
            Statement::Merge {
                branches, target, ..
            } => {
                format!("merge [{}] into {}", branches.join(","), target)
            }
            Statement::Anchor(name) => format!("anchor {}", name),
            Statement::Rewind(name) => format!("rewind_to({})", name),
            Statement::Commit(_) => "commit { ... }".to_string(),
            Statement::SpeculationMode(_) => "speculation_mode(...)".to_string(),
            Statement::Send {
                value_id,
                target_branch,
            } => {
                format!("send {} to {}", value_id, target_branch)
            }
            Statement::Isolate(block) => format!(
                "isolate {} {{ ... }}",
                block.name.clone().unwrap_or_default()
            ),
            Statement::RelativisticBlock { time, .. } => match time {
                TimeCoordinate::Branch(id) => format!("@{}: {{ ... }}", id),
                _ => "relativistic block".to_string(),
            },
            Statement::Capability(cap) => format!("require {}(...)", cap.path),
            Statement::If {
                binding, condition, ..
            } => {
                if let Some(b) = binding {
                    format!(
                        "if let {} = {} {{ ... }}",
                        b,
                        self.expr_snippet(condition)
                    )
                } else {
                    format!("if ({}) {{ ... }}", self.expr_snippet(condition))
                }
            }
            Statement::Loop { max_ms, .. } => {
                format!("loop (max {}ms) {{ ... }}", max_ms)
            }
            Statement::Speculate { max_ms, .. } => {
                format!("speculate (max {}ms) {{ ... }}", max_ms)
            }
            Statement::Collapse => "collapse".to_string(),
            Statement::Break => "break".to_string(),
            Statement::Entangle { variables } => {
                format!("entangle({})", variables.join(","))
            }
            _ => format!("{:?}", stmt),
        }
    }

    fn expr_snippet(&self, expr: &Expression) -> String {
        match expr {
            Expression::Literal(v) => format!("\"{}\"", v),
            Expression::Identifier(v) => v.clone(),
            Expression::Null => "null".to_string(),
            Expression::Boolean(b) => b.to_string(),
            Expression::FieldAccess { target, field } => {
                format!("{}.{}", self.expr_snippet(target), field)
            }
            Expression::CloneOp(v) => format!("clone({})", v),
            Expression::StrBytes(e) => {
                format!("str_bytes({})", self.expr_snippet(e))
            }
            Expression::ToStr(e) => format!("to_str({})", self.expr_snippet(e)),
            Expression::Len(e) => format!("len({})", self.expr_snippet(e)),
            Expression::RefOp(e) => format!("&{}", self.expr_snippet(e)),
            Expression::Syscall { .. } => "syscall(...)".to_string(),
            Expression::StructLit(_, fields) => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{} = {}", k, self.expr_snippet(v)))
                    .collect();
                format!("struct {{ {} }}", parts.join(", "))
            }
            Expression::TopologyLit(fields) => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{} = {}", k, self.expr_snippet(v)))
                    .collect();
                format!("topology {{ {} }}", parts.join(", "))
            }
            Expression::IndexAccess { target, index } => {
                format!(
                    "{}[{}]",
                    self.expr_snippet(target),
                    self.expr_snippet(index)
                )
            }
            Expression::ChannelReceive(id) => format!("chan_recv({})", id),
            Expression::ArrayLiteral(elements) => {
                let parts: Vec<String> =
                    elements.iter().map(|e| self.expr_snippet(e)).collect();
                format!("[{}]", parts.join(","))
            }
            Expression::ArrayRepeat { value, count } => {
                format!(
                    "[{}; {}]",
                    self.expr_snippet(value),
                    self.expr_snippet(count)
                )
            }
            Expression::ArraySlice {
                target,
                start,
                end,
                inclusive,
            } => {
                let s_str = start
                    .as_ref()
                    .map(|s| self.expr_snippet(s))
                    .unwrap_or_default();
                let dot_str = if *inclusive { "..=" } else { ".." };
                let e_str = end
                    .as_ref()
                    .map(|e| self.expr_snippet(e))
                    .unwrap_or_default();
                format!(
                    "{}[{}{}{}]",
                    self.expr_snippet(target),
                    s_str,
                    dot_str,
                    e_str
                )
            }
            Expression::Integer(v) => format!("{}", v),
            Expression::Float(bits) => format!("{}", f64::from_bits(*bits)),
            Expression::Deferred { capability, .. } => {
                format!("defer {}(...)", capability)
            }
            Expression::MethodCall {
                target,
                method,
                args,
                ..
            } => {
                let args_str: Vec<String> =
                    args.iter().map(|e| self.expr_snippet(e)).collect();
                format!(
                    "{}.{}({})",
                    self.expr_snippet(target),
                    method,
                    args_str.join(", ")
                )
            }
            Expression::Call { routine, args } => {
                let args_str: Vec<String> =
                    args.iter().map(|e| self.expr_snippet(e)).collect();
                format!("call {}({})", routine, args_str.join(", "))
            }
            Expression::BinaryOp { left, op, right } => {
                let op_str = match op {
                    BinaryOperator::Add => "+",
                    BinaryOperator::Sub => "-",
                    BinaryOperator::Mul => "*",
                    BinaryOperator::Div => "/",
                    BinaryOperator::Rem => "%",
                    BinaryOperator::Pow => "^",
                    BinaryOperator::Eq => "==",
                    BinaryOperator::Neq => "!=",
                    BinaryOperator::Lt => "<",
                    BinaryOperator::Gt => ">",
                    BinaryOperator::Le => "<=",
                    BinaryOperator::Ge => ">=",
                    BinaryOperator::LogicalAnd => "&&",
                    BinaryOperator::LogicalOr => "||",
                    BinaryOperator::BitwiseAnd => "&",
                    BinaryOperator::BitwiseOr => "|",
                    BinaryOperator::BitwiseXor => "^",
                    BinaryOperator::Shl => "<<",
                    BinaryOperator::Shr => ">>",
                    BinaryOperator::NullCoalesce => "??",
                };
                format!(
                    "({} {} {})",
                    self.expr_snippet(left),
                    op_str,
                    self.expr_snippet(right)
                )
            }
            Expression::UnaryOp { op, expr } => {
                let op_str = match op {
                    causm_core::UnaryOperator::Neg => "-",
                    causm_core::UnaryOperator::Not => "!",
                    causm_core::UnaryOperator::BitwiseNot => "~",
                };
                format!("{}{}", op_str, self.expr_snippet(expr))
            }
            Expression::TypeAssertion { target, cast_type } => {
                format!("{}.({:?})", self.expr_snippet(target), cast_type)
            }
            Expression::TypeCast { expr, target_type } => {
                format!("{} as {:?}", self.expr_snippet(expr), target_type)
            }
            Expression::TryUnwrap(expr) => {
                format!("{}?", self.expr_snippet(expr))
            }
            Expression::EnumVariant {
                enum_name,
                variant_name,
                args,
            } => {
                let args_str: Vec<String> =
                    args.iter().map(|a| self.expr_snippet(a)).collect();
                format!("{}::{}({})", enum_name, variant_name, args_str.join(", "))
            }
            Expression::FString(parts) => {
                let mut s = "f\"".to_string();
                for part in parts {
                    match part {
                        causm_core::FStringPart::Text(t) => s.push_str(t),
                        causm_core::FStringPart::Expr(e) => {
                            s.push('{');
                            s.push_str(&self.expr_snippet(e));
                            s.push('}');
                        }
                    }
                }
                s.push('"');
                s
            }
            Expression::If {
                condition,
                then_branch,
                else_branch,
            } => {
                format!(
                    "if ({}) {{ {} }} else {{ {} }}",
                    self.expr_snippet(condition),
                    self.expr_snippet(then_branch),
                    self.expr_snippet(else_branch)
                )
            }
            Expression::Match { target, .. } => {
                format!("match {} {{ ... }}", self.expr_snippet(target))
            }
            Expression::ArenaIntrospect(kind) => match kind {
                causm_core::ArenaIntrospect::Remaining => {
                    "arena.remaining()".to_string()
                }
                causm_core::ArenaIntrospect::UsedBytes => {
                    "arena.used_bytes()".to_string()
                }
                causm_core::ArenaIntrospect::Capacity => {
                    "arena.capacity()".to_string()
                }
            },
            Expression::CapabilityCheck(cap) => {
                format!("capability({})", cap.path)
            }
            Expression::Turbofish { expr, .. } => self.expr_snippet(expr),
            Expression::GenericStaticCall {
                type_name, method, ..
            } => {
                format!("{}::{}()", type_name, method)
            }
        }
    }

    pub fn record_state(&mut self, span: Span) {
        if let Some(state) = self.branch_contexts.get(&self.current_branch) {
            self.span_states.insert(span, state.clone());
        }
    }
}
