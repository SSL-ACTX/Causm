use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Debug,
    Default,
    Serialize,
    Deserialize,
)]
pub struct Symbol(pub u32);

impl Symbol {
    pub const INVALID: Symbol = Symbol(0);
    pub const TRUE: Symbol = Symbol(1);
    pub const FALSE: Symbol = Symbol(2);
    pub const NULL: Symbol = Symbol(3);
    pub const SELF: Symbol = Symbol(4);
}

pub struct Interner {
    table: HashMap<String, Symbol>,
    storage: Vec<String>,
}

impl Default for Interner {
    fn default() -> Self {
        let mut interner = Self {
            table: HashMap::with_capacity(256),
            storage: Vec::with_capacity(256),
        };
        interner.intern("");
        interner.intern("true");
        interner.intern("false");
        interner.intern("null");
        interner.intern("self");
        interner
    }
}

impl Interner {
    pub fn intern(&mut self, string: &str) -> Symbol {
        if let Some(&sym) = self.table.get(string) {
            return sym;
        }
        let sym = Symbol(self.storage.len() as u32);
        self.storage.push(string.to_string());
        self.table.insert(string.to_string(), sym);
        sym
    }

    pub fn lookup(&self, sym: Symbol) -> &str {
        self.storage
            .get(sym.0 as usize)
            .map(|s| s.as_str())
            .unwrap_or("")
    }
}

static GLOBAL_INTERNER: LazyLock<Mutex<Interner>> =
    LazyLock::new(|| Mutex::new(Interner::default()));

pub fn intern(string: &str) -> Symbol {
    GLOBAL_INTERNER.lock().unwrap().intern(string)
}

pub fn resolve(sym: Symbol) -> String {
    GLOBAL_INTERNER.lock().unwrap().lookup(sym).to_string()
}
