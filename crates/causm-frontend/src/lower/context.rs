use causm_core::{Span, TypeFieldDef};
use causm_ir::{Instruction, IrRoutine, Reg};
use std::collections::HashMap;

pub struct LoweringContext {
    pub next_reg: u32,
    pub symbols: HashMap<String, Reg>,
    pub instructions: Vec<Instruction>,
    pub spans: Vec<Option<Span>>,
    pub routines: HashMap<String, IrRoutine>,
    pub type_decay_limits: HashMap<String, u64>,
    pub auto_drop_specs: HashMap<String, causm_core::types::AutoDropSpec>,
    pub reg_types: HashMap<u32, String>,
    pub type_decls: HashMap<String, HashMap<String, TypeFieldDef>>,
    pub interfaces: HashMap<String, Vec<causm_core::InterfaceMethod>>,
    pub(crate) struct_extends: HashMap<String, String>,
    pub(crate) decay_handlers: HashMap<String, Vec<Instruction>>,
    pub current_span: Option<Span>,
    pub entropy_modes: Vec<causm_core::EntropyMode>,
}

impl Default for LoweringContext {
    fn default() -> Self {
        Self::new()
    }
}

impl LoweringContext {
    pub fn new() -> Self {
        Self {
            next_reg: 0,
            symbols: HashMap::new(),
            instructions: Vec::new(),
            spans: Vec::new(),
            routines: HashMap::new(),
            type_decay_limits: HashMap::new(),
            auto_drop_specs: HashMap::new(),
            reg_types: HashMap::new(),
            type_decls: HashMap::new(),
            interfaces: HashMap::new(),
            struct_extends: HashMap::new(),
            decay_handlers: HashMap::new(),
            current_span: None,
            entropy_modes: vec![causm_core::EntropyMode::Deterministic],
        }
    }

    pub fn alloc_reg(&mut self) -> Reg {
        let r = Reg(self.next_reg);
        self.next_reg += 1;
        r
    }

    pub fn get_reg(&mut self, name: &str) -> Reg {
        if let Some(r) = self.symbols.get(name) {
            *r
        } else {
            let r = self.alloc_reg();
            self.symbols.insert(name.to_string(), r);
            r
        }
    }

    pub fn push(&mut self, instr: Instruction) {
        self.instructions.push(instr);
        self.spans.push(self.current_span.clone());
    }
}
