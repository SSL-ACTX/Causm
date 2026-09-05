use causm_core::arena::AstArena;
use causm_core::symbol::Symbol;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(
    Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct ModuleId(pub u32);

#[derive(
    Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct DefId {
    pub module: ModuleId,
    pub index: u32,
}

#[derive(Clone, Debug, Default)]
pub struct ParsedModule {
    pub id: ModuleId,
    pub path: String,
    pub arena: Arc<AstArena>,
    pub exports: HashMap<Symbol, DefId>,
}

#[derive(Default)]
pub struct ModuleStore {
    modules: HashMap<ModuleId, ParsedModule>,
    path_to_id: HashMap<String, ModuleId>,
}

impl ModuleStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, path: &str, arena: AstArena) -> ModuleId {
        if let Some(&id) = self.path_to_id.get(path) {
            return id;
        }
        let id = ModuleId(self.modules.len() as u32);
        let mut exports = HashMap::new();
        for &sid in &arena.root_statements {
            match &arena.statements[sid.0 as usize] {
                causm_core::arena::StmtNode::RoutineDef { name, .. } => {
                    exports.insert(
                        *name,
                        DefId {
                            module: id,
                            index: sid.0,
                        },
                    );
                }
                causm_core::arena::StmtNode::TypeDecl { name, .. } => {
                    exports.insert(
                        *name,
                        DefId {
                            module: id,
                            index: sid.0,
                        },
                    );
                }
                causm_core::arena::StmtNode::EnumDecl { name, .. } => {
                    exports.insert(
                        *name,
                        DefId {
                            module: id,
                            index: sid.0,
                        },
                    );
                }
                _ => {}
            }
        }
        let parsed = ParsedModule {
            id,
            path: path.to_string(),
            arena: Arc::new(arena),
            exports,
        };
        self.modules.insert(id, parsed);
        self.path_to_id.insert(path.to_string(), id);
        id
    }

    pub fn get_or_parse_module(
        &mut self,
        path: &str,
        source: &str,
    ) -> Result<ModuleId, String> {
        if let Some(&id) = self.path_to_id.get(path) {
            return Ok(id);
        }
        let mut parser = super::arena_parser::ArenaParser::new(source);
        parser.parse_program()?;
        Ok(self.register(path, parser.arena))
    }

    pub fn get_module(&self, id: ModuleId) -> Option<&ParsedModule> {
        self.modules.get(&id)
    }

    pub fn get_by_path(&self, path: &str) -> Option<&ParsedModule> {
        self.path_to_id
            .get(path)
            .and_then(|id| self.modules.get(id))
    }

    pub fn resolve_export(
        &self,
        module_id: ModuleId,
        symbol: Symbol,
    ) -> Option<DefId> {
        self.get_module(module_id)
            .and_then(|m| m.exports.get(&symbol).copied())
    }

    pub fn get_module_ast(&self, id: ModuleId) -> Option<causm_core::Program> {
        let module = self.get_module(id)?;
        let mut timelines = Vec::new();
        let mut standalone = Vec::new();
        for &sid in &module.arena.root_statements {
            if let causm_core::arena::StmtNode::TimelineBlock {
                coord, body, ..
            } = &module.arena.statements[sid.0 as usize]
            {
                let stmts = module.arena.stmt_pool[body.as_range()]
                    .iter()
                    .map(|&s| {
                        super::arena_parser::to_ast_statement(&module.arena, s)
                    })
                    .collect();
                timelines.push(causm_core::TimelineBlock {
                    time: coord.clone(),
                    no_z3: false,
                    entropy_mode: None,
                    statements: stmts,
                });
            } else {
                standalone
                    .push(super::arena_parser::to_ast_statement(&module.arena, sid));
            }
        }
        if !standalone.is_empty() {
            timelines.insert(
                0,
                causm_core::TimelineBlock {
                    time: causm_core::TimeCoordinate::Global(0),
                    no_z3: false,
                    entropy_mode: None,
                    statements: standalone,
                },
            );
        }
        let mut prog = causm_core::Program { timelines };
        crate::macro_expand::expand_program(&mut prog);
        crate::derive::expand_derives(&mut prog);
        Some(prog)
    }
}

static GLOBAL_MODULE_STORE: std::sync::LazyLock<RwLock<ModuleStore>> =
    std::sync::LazyLock::new(|| RwLock::new(ModuleStore::default()));

pub fn global_module_store() -> &'static RwLock<ModuleStore> {
    &GLOBAL_MODULE_STORE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_module_store_zero_clone_resolution() {
        let math_code = r#"
            routine add(x: int, y: int) -> int {
                return x + y
            }
            struct Vec2 {
                x: int,
                y: int
            }
        "#;
        let mut store = ModuleStore::new();
        let mod_id = store
            .get_or_parse_module("std/math", math_code)
            .expect("module parses");
        let parsed = store.get_module(mod_id).expect("module exists");
        assert_eq!(parsed.exports.len(), 2);
        let add_sym = causm_core::symbol::intern("add");
        let vec2_sym = causm_core::symbol::intern("Vec2");
        assert!(store.resolve_export(mod_id, add_sym).is_some());
        assert!(store.resolve_export(mod_id, vec2_sym).is_some());
    }
}
