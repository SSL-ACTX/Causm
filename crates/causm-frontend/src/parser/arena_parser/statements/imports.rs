use crate::parser::arena_parser::ArenaParser;
use crate::parser::lexer::TokenKind;
use causm_core::arena::{SliceRange, StmtId, StmtNode};

impl<'a> ArenaParser<'a> {
    pub fn parse_import_stmt(&mut self) -> Result<Option<StmtId>, String> {
        let imp_tok = self.bump();
        let path = match self.peek() {
            TokenKind::Str(ref s) => causm_core::symbol::intern(s),
            _ => {
                return Err(format!(
                    "Expected string path in import, found {:?}",
                    self.peek()
                ))
            }
        };
        self.bump();
        let alias = if self.peek() == &TokenKind::As {
            self.bump();
            match self.peek() {
                TokenKind::Ident(sym) => {
                    let s = *sym;
                    self.bump();
                    Some(s)
                }
                _ => None,
            }
        } else {
            None
        };
        let id = self
            .arena
            .alloc_stmt(StmtNode::Import { path, alias }, imp_tok.span);
        Ok(Some(id))
    }

    pub fn parse_require_stmt(&mut self) -> Result<Option<StmtId>, String> {
        let req_tok = self.bump();
        let mut name_parts = Vec::new();
        if let TokenKind::Ident(s) = self.peek() {
            name_parts.push(causm_core::symbol::resolve(*s));
            self.bump();
            while self.peek() == &TokenKind::Dot {
                self.bump();
                if let TokenKind::Ident(next_s) = self.peek() {
                    name_parts.push(causm_core::symbol::resolve(*next_s));
                    self.bump();
                }
            }
        }
        let mut parameters = std::collections::HashMap::new();
        if self.peek() == &TokenKind::LParen {
            self.bump();
            while self.peek() != &TokenKind::RParen && self.peek() != &TokenKind::Eof
            {
                if let TokenKind::Ident(k) = self.peek().clone() {
                    let k_str = causm_core::symbol::resolve(k);
                    self.bump();
                    if self.peek() == &TokenKind::Eq {
                        self.bump();
                        let v_str = match self.peek() {
                            TokenKind::Str(s) => s.clone(),
                            TokenKind::Ident(s) => causm_core::symbol::resolve(*s),
                            _ => String::new(),
                        };
                        self.bump();
                        parameters.insert(k_str, v_str);
                    }
                }
                if self.peek() == &TokenKind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            if self.peek() == &TokenKind::RParen {
                self.bump();
            }
        }
        let cap = causm_core::Capability {
            path: name_parts.join("."),
            parameters,
        };
        let id = self
            .arena
            .alloc_stmt(StmtNode::Capability(cap), req_tok.span);
        Ok(Some(id))
    }

    pub fn parse_foreign_stmt(&mut self) -> Result<Option<StmtId>, String> {
        let f_tok = self.bump();
        let lib_name = match self.peek() {
            TokenKind::Str(s) => {
                let sym = causm_core::symbol::intern(s);
                self.bump();
                sym
            }
            _ => causm_core::symbol::intern(""),
        };
        let mut abi = causm_core::symbol::intern("C");
        if let TokenKind::Ident(s) = self.peek() {
            if causm_core::symbol::resolve(*s) == "abi" {
                self.bump();
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    if let TokenKind::Str(abi_str) = self.peek() {
                        abi = causm_core::symbol::intern(abi_str);
                        self.bump();
                    }
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                }
            }
        }
        let mut routines = Vec::new();
        if self.peek() == &TokenKind::LBrace {
            self.bump();
            while self.peek() != &TokenKind::RBrace && self.peek() != &TokenKind::Eof
            {
                if let Some(r_id) = self.parse_statement()? {
                    routines.push(r_id);
                }
            }
            if self.peek() == &TokenKind::RBrace {
                self.bump();
            }
        }
        let r_start = self.arena.stmt_pool.len();
        for r in routines {
            self.arena.stmt_pool.push(r);
        }
        let r_end = self.arena.stmt_pool.len();
        let id = self.arena.alloc_stmt(
            StmtNode::ForeignBlock {
                lib_name,
                abi,
                routines: SliceRange::new(r_start, r_end),
            },
            f_tok.span,
        );
        Ok(Some(id))
    }

    pub fn parse_macro_stmt(&mut self) -> Result<Option<StmtId>, String> {
        let macro_tok = self.bump();
        let name = match self.peek() {
            TokenKind::Ident(sym) => {
                let s = *sym;
                self.bump();
                s
            }
            _ => return Err("Expected macro name".into()),
        };
        if self.peek() == &TokenKind::Bang {
            self.bump();
        }
        let mut params = Vec::new();
        if self.peek() == &TokenKind::LParen {
            self.bump();
            while self.peek() != &TokenKind::FatArrow
                && self.peek() != &TokenKind::RParen
                && self.peek() != &TokenKind::Eof
            {
                if self.peek() == &TokenKind::Dollar {
                    self.bump();
                }
                if let TokenKind::Ident(p_sym) = self.peek() {
                    let p_name = causm_core::symbol::resolve(*p_sym)
                        .trim_start_matches('$')
                        .to_string();
                    self.bump();
                    let mut kind = causm_core::MacroParamKind::Expr;
                    if self.peek() == &TokenKind::Colon {
                        self.bump();
                        if let TokenKind::Ident(k_sym) = self.peek() {
                            let k_str = causm_core::symbol::resolve(*k_sym);
                            kind = match k_str.as_str() {
                                "ident" => causm_core::MacroParamKind::Ident,
                                "type" => causm_core::MacroParamKind::Type,
                                "literal" => causm_core::MacroParamKind::Literal,
                                _ => causm_core::MacroParamKind::Expr,
                            };
                            self.bump();
                        }
                    }
                    params.push(causm_core::MacroParam { name: p_name, kind });
                }
                if self.peek() == &TokenKind::Comma {
                    self.bump();
                }
            }
            if self.peek() == &TokenKind::FatArrow {
                self.bump();
            }
        }
        let mut body_template = String::new();
        if self.peek() == &TokenKind::LBrace {
            let start_idx = self.current.span.start + 1;
            self.bump();
            let mut depth = 1;
            let mut end_idx = start_idx;
            while depth > 0 && self.peek() != &TokenKind::Eof {
                match self.peek() {
                    TokenKind::LBrace => depth += 1,
                    TokenKind::RBrace => {
                        depth -= 1;
                        if depth == 0 {
                            end_idx = self.current.span.start;
                            self.bump();
                            break;
                        }
                    }
                    _ => {}
                }
                self.bump();
            }
            if end_idx >= start_idx && end_idx <= self.stream.source.len() {
                body_template = self.stream.source[start_idx..end_idx].to_string();
            }
        }
        if self.peek() == &TokenKind::RParen {
            self.bump();
        }
        let id = self.arena.alloc_stmt(
            StmtNode::MacroDef {
                name,
                params,
                body_template,
            },
            macro_tok.span,
        );
        Ok(Some(id))
    }

    pub fn parse_from_stmt(&mut self) -> Result<Option<StmtId>, String> {
        let from_tok = self.bump();
        let path = match self.peek() {
            TokenKind::Str(ref s) => causm_core::symbol::intern(s),
            _ => return Err("Expected module path after from".into()),
        };
        self.bump();
        if self.peek() == &TokenKind::Import {
            self.bump();
        }
        let mut sym_vec = Vec::new();
        if self.peek() == &TokenKind::Star {
            self.bump();
            sym_vec.push(causm_core::symbol::intern("*"));
        } else {
            while self.peek() != &TokenKind::Semi
                && self.peek() != &TokenKind::RBrace
                && self.peek() != &TokenKind::Eof
            {
                if let TokenKind::Ident(s) = self.peek() {
                    let mut item_name = causm_core::symbol::resolve(*s);
                    self.bump();
                    if self.peek() == &TokenKind::As {
                        self.bump();
                        if let TokenKind::Ident(alias_s) = self.peek() {
                            item_name = format!(
                                "{} as {}",
                                item_name,
                                causm_core::symbol::resolve(*alias_s)
                            );
                            self.bump();
                        }
                    }
                    sym_vec.push(causm_core::symbol::intern(&item_name));
                    if self.peek() == &TokenKind::Comma {
                        self.bump();
                    }
                } else if self.peek() == &TokenKind::Star {
                    self.bump();
                    sym_vec.push(causm_core::symbol::intern("*"));
                    break;
                } else {
                    break;
                }
            }
        }
        let s_start = self.arena.symbol_pool.len();
        for s in sym_vec {
            self.arena.symbol_pool.push(s);
        }
        let s_end = self.arena.symbol_pool.len();
        let id = self.arena.alloc_stmt(
            StmtNode::FromImport {
                path,
                symbols: SliceRange::new(s_start, s_end),
            },
            from_tok.span,
        );
        Ok(Some(id))
    }
}
