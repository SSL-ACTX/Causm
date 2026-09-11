use crate::parser::arena_parser::ArenaParser;
use crate::parser::lexer::TokenKind;
use causm_core::arena::{SliceRange, StmtId, StmtNode};

impl<'a> ArenaParser<'a> {
    pub fn parse_interface_stmt(&mut self) -> Result<Option<StmtId>, String> {
        let iface_tok = self.bump();
        let name = match self.peek() {
            TokenKind::Ident(sym) => {
                let s = *sym;
                self.bump();
                s
            }
            _ => return Err("Expected interface name".into()),
        };
        if self.peek() == &TokenKind::Lt {
            self.bump();
            let mut depth = 1;
            while depth > 0 && self.peek() != &TokenKind::Eof {
                match self.peek() {
                    TokenKind::Lt => depth += 1,
                    TokenKind::Gt => depth -= 1,
                    _ => {}
                }
                self.bump();
            }
        }
        if let TokenKind::Ident(sym) = self.peek() {
            if causm_core::symbol::resolve(*sym) == "decay_after" {
                self.bump();
                if matches!(self.peek(), TokenKind::Duration(_) | TokenKind::Int(_))
                {
                    self.bump();
                }
            }
        }
        let mut extends = Vec::new();
        if self.peek() == &TokenKind::Eq {
            self.bump();
            while self.peek() != &TokenKind::LBrace
                && self.peek() != &TokenKind::Eof
                && self.peek() != &TokenKind::Semi
            {
                if let TokenKind::Ident(sym) = self.peek() {
                    extends.push(*sym);
                    self.bump();
                } else if self.peek() == &TokenKind::Plus {
                    self.bump();
                    if self.peek() == &TokenKind::Interface {
                        self.bump();
                    }
                } else if self.peek() == &TokenKind::Interface {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        let ext_start = self.arena.symbol_pool.len();
        for e in extends {
            self.arena.symbol_pool.push(e);
        }
        let ext_end = self.arena.symbol_pool.len();

        let mut method_ids = Vec::new();
        if self.peek() == &TokenKind::LBrace {
            self.bump();
            while self.peek() != &TokenKind::RBrace && self.peek() != &TokenKind::Eof
            {
                if self.peek() == &TokenKind::Semi {
                    self.bump();
                    continue;
                }
                if let Some(stmt_id) = self.parse_statement()? {
                    method_ids.push(stmt_id);
                }
            }
            if self.peek() == &TokenKind::RBrace {
                self.bump();
            }
        }
        let m_start = self.arena.stmt_pool.len();
        for m in method_ids {
            self.arena.stmt_pool.push(m);
        }
        let m_end = self.arena.stmt_pool.len();

        let id = self.arena.alloc_stmt(
            StmtNode::InterfaceDecl {
                name,
                extends: SliceRange::new(ext_start, ext_end),
                methods: SliceRange::new(m_start, m_end),
            },
            iface_tok.span,
        );
        Ok(Some(id))
    }

    pub fn parse_type_stmt(&mut self) -> Result<Option<StmtId>, String> {
        let type_tok = self.bump();
        let name = match self.peek() {
            TokenKind::Ident(sym) => {
                let s = *sym;
                self.bump();
                s
            }
            _ => return Err("Expected type name".into()),
        };
        if self.peek() == &TokenKind::Lt {
            self.bump();
            let mut depth = 1;
            while depth > 0 && self.peek() != &TokenKind::Eof {
                match self.peek() {
                    TokenKind::Lt => depth += 1,
                    TokenKind::Gt => depth -= 1,
                    _ => {}
                }
                self.bump();
            }
        }
        if self.peek() == &TokenKind::Eq || self.peek() == &TokenKind::Colon {
            self.bump();
        }
        if self.peek() == &TokenKind::Distinct {
            self.bump();
            if let TokenKind::Ident(_) = self.peek() {
                self.bump();
            }
            let f_start = self.arena.field_assigns_pool.len();
            self.arena
                .field_assigns_pool
                .push(causm_core::arena::FieldAssignNode {
                    field: causm_core::symbol::intern("value"),
                    expr: causm_core::arena::ExprId(0),
                    type_name: None,
                    is_const: false,
                });
            let f_end = self.arena.field_assigns_pool.len();
            let id = self.arena.alloc_stmt(
                StmtNode::TypeDecl {
                    name,
                    extends: None,
                    fields: SliceRange::new(f_start, f_end),
                    decay_after_ms: None,
                    auto_drop: None,
                },
                type_tok.span,
            );
            return Ok(Some(id));
        }
        let mut extends = None;
        if self.peek() == &TokenKind::Struct {
            self.bump();
        } else if self.peek() != &TokenKind::LBrace {
            if let TokenKind::Ident(t_bound) = self.peek() {
                let f_sym = *t_bound;
                self.bump();
                if self.peek() == &TokenKind::Plus {
                    self.bump();
                    extends = Some(f_sym);
                    if self.peek() == &TokenKind::Struct {
                        self.bump();
                    }
                } else {
                    let f_start = self.arena.field_assigns_pool.len();
                    self.arena.field_assigns_pool.push(
                        causm_core::arena::FieldAssignNode {
                            field: f_sym,
                            expr: causm_core::arena::ExprId(0),
                            type_name: Some(f_sym),
                            is_const: false,
                        },
                    );
                    let f_end = self.arena.field_assigns_pool.len();
                    let id = self.arena.alloc_stmt(
                        StmtNode::TypeDecl {
                            name,
                            extends: None,
                            fields: SliceRange::new(f_start, f_end),
                            decay_after_ms: None,
                            auto_drop: None,
                        },
                        type_tok.span,
                    );
                    return Ok(Some(id));
                }
            }
        }
        let mut decay_after_ms = None;
        let mut auto_drop = None;
        loop {
            if let TokenKind::Ident(sym) = self.peek() {
                let s = causm_core::symbol::resolve(*sym);
                if s == "decay_after" {
                    self.bump();
                    match self.peek() {
                        TokenKind::Int(ms) => {
                            decay_after_ms = Some(*ms as u64);
                            self.bump();
                        }
                        TokenKind::Duration(ms) => {
                            decay_after_ms = Some(*ms);
                            self.bump();
                        }
                        _ => {}
                    }
                    continue;
                } else if s == "auto_drop" {
                    self.bump();
                    if self.peek() == &TokenKind::LParen {
                        self.bump();
                        let lib_name = if let TokenKind::Str(s) = self.peek() {
                            s.clone()
                        } else {
                            String::new()
                        };
                        self.bump();
                        if self.peek() == &TokenKind::Comma {
                            self.bump();
                        }
                        let routine_name = if let TokenKind::Str(s) = self.peek() {
                            s.clone()
                        } else {
                            String::new()
                        };
                        self.bump();
                        if self.peek() == &TokenKind::Comma {
                            self.bump();
                        }
                        let field_name = if let TokenKind::Ident(s) = self.peek() {
                            causm_core::symbol::resolve(*s)
                        } else {
                            String::new()
                        };
                        self.bump();
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                        auto_drop = Some(causm_core::types::AutoDropSpec {
                            lib_name,
                            routine_name,
                            field_name,
                        });
                    }
                    continue;
                }
            } else if self.peek() == &TokenKind::Auto {
                self.bump();
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    let lib_name = if let TokenKind::Str(s) = self.peek() {
                        s.clone()
                    } else {
                        String::new()
                    };
                    self.bump();
                    if self.peek() == &TokenKind::Comma {
                        self.bump();
                    }
                    let routine_name = if let TokenKind::Str(s) = self.peek() {
                        s.clone()
                    } else {
                        String::new()
                    };
                    self.bump();
                    if self.peek() == &TokenKind::Comma {
                        self.bump();
                    }
                    let field_name = if let TokenKind::Ident(s) = self.peek() {
                        causm_core::symbol::resolve(*s)
                    } else {
                        String::new()
                    };
                    self.bump();
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                    auto_drop = Some(causm_core::types::AutoDropSpec {
                        lib_name,
                        routine_name,
                        field_name,
                    });
                }
                continue;
            }
            break;
        }
        let mut fields_list = Vec::new();
        if self.peek() == &TokenKind::LBrace {
            self.bump();
            while self.peek() != &TokenKind::RBrace && self.peek() != &TokenKind::Eof
            {
                let is_const = if let TokenKind::Ident(s) = self.peek() {
                    causm_core::symbol::resolve(*s) == "const"
                } else {
                    false
                };
                if is_const {
                    self.bump();
                }
                if let TokenKind::Ident(f_sym) = self.peek() {
                    let field = *f_sym;
                    self.bump();
                    let mut type_name = None;
                    if self.peek() == &TokenKind::Colon {
                        self.bump();
                        if let TokenKind::Ident(t_sym) = self.peek() {
                            type_name = Some(*t_sym);
                            self.bump();
                        }
                        while self.peek() != &TokenKind::Eq
                            && self.peek() != &TokenKind::Comma
                            && self.peek() != &TokenKind::RBrace
                            && self.peek() != &TokenKind::Eof
                        {
                            self.bump();
                        }
                    }
                    let expr = if self.peek() == &TokenKind::Eq {
                        self.bump();
                        self.parse_expression(0)?
                    } else {
                        causm_core::arena::ExprId(u32::MAX)
                    };
                    fields_list.push(causm_core::arena::FieldAssignNode {
                        field,
                        expr,
                        type_name,
                        is_const,
                    });
                    if self.peek() == &TokenKind::Comma {
                        self.bump();
                    }
                } else {
                    self.bump();
                }
            }
            if self.peek() == &TokenKind::RBrace {
                self.bump();
            }
        }
        loop {
            if let TokenKind::Ident(sym) = self.peek() {
                let s = causm_core::symbol::resolve(*sym);
                if s == "decay_after" {
                    self.bump();
                    match self.peek() {
                        TokenKind::Int(ms) => {
                            decay_after_ms = Some(*ms as u64);
                            self.bump();
                        }
                        TokenKind::Duration(ms) => {
                            decay_after_ms = Some(*ms);
                            self.bump();
                        }
                        _ => {}
                    }
                    continue;
                } else if s == "auto_drop" {
                    self.bump();
                    if self.peek() == &TokenKind::LParen {
                        self.bump();
                        let lib_name = if let TokenKind::Str(s) = self.peek() {
                            s.clone()
                        } else {
                            String::new()
                        };
                        self.bump();
                        if self.peek() == &TokenKind::Comma {
                            self.bump();
                        }
                        let routine_name = if let TokenKind::Str(s) = self.peek() {
                            s.clone()
                        } else {
                            String::new()
                        };
                        self.bump();
                        if self.peek() == &TokenKind::Comma {
                            self.bump();
                        }
                        let field_name = if let TokenKind::Ident(s) = self.peek() {
                            causm_core::symbol::resolve(*s)
                        } else {
                            String::new()
                        };
                        self.bump();
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                        auto_drop = Some(causm_core::types::AutoDropSpec {
                            lib_name,
                            routine_name,
                            field_name,
                        });
                    }
                    continue;
                }
            } else if self.peek() == &TokenKind::Auto {
                self.bump();
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    let lib_name = if let TokenKind::Str(s) = self.peek() {
                        s.clone()
                    } else {
                        String::new()
                    };
                    self.bump();
                    if self.peek() == &TokenKind::Comma {
                        self.bump();
                    }
                    let routine_name = if let TokenKind::Str(s) = self.peek() {
                        s.clone()
                    } else {
                        String::new()
                    };
                    self.bump();
                    if self.peek() == &TokenKind::Comma {
                        self.bump();
                    }
                    let field_name = if let TokenKind::Ident(s) = self.peek() {
                        causm_core::symbol::resolve(*s)
                    } else {
                        String::new()
                    };
                    self.bump();
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                    auto_drop = Some(causm_core::types::AutoDropSpec {
                        lib_name,
                        routine_name,
                        field_name,
                    });
                }
                continue;
            }
            break;
        }
        let f_start = self.arena.field_assigns_pool.len();
        for f in fields_list {
            self.arena.field_assigns_pool.push(f);
        }
        let f_end = self.arena.field_assigns_pool.len();
        let id = self.arena.alloc_stmt(
            StmtNode::TypeDecl {
                name,
                extends,
                fields: SliceRange::new(f_start, f_end),
                decay_after_ms,
                auto_drop,
            },
            type_tok.span,
        );
        Ok(Some(id))
    }

    pub fn parse_enum_stmt(&mut self) -> Result<Option<StmtId>, String> {
        let enum_tok = self.bump();
        let name = match self.peek() {
            TokenKind::Ident(sym) => *sym,
            _ => return Err("Expected enum name".into()),
        };
        self.bump();
        let mut variants_vec = Vec::new();
        if self.peek() == &TokenKind::LBrace {
            self.bump();
            while self.peek() != &TokenKind::RBrace && self.peek() != &TokenKind::Eof
            {
                let v_name = match self.peek() {
                    TokenKind::Ident(sym) => Some(causm_core::symbol::resolve(*sym)),
                    TokenKind::Valid => Some("Valid".into()),
                    TokenKind::Lease => Some("Leased".into()),
                    TokenKind::Decayed => Some("Decayed".into()),
                    TokenKind::Pending => Some("Pending".into()),
                    TokenKind::Consumed => Some("Consumed".into()),
                    _ => None,
                };
                if let Some(name_str) = v_name {
                    self.bump();
                    let mut payload_types = Vec::new();
                    if self.peek() == &TokenKind::LParen {
                        self.bump();
                        while self.peek() != &TokenKind::RParen
                            && self.peek() != &TokenKind::Eof
                        {
                            if let TokenKind::Ident(t_sym) = self.peek() {
                                let t_name = causm_core::symbol::resolve(*t_sym);
                                let typ = match t_name.as_str() {
                                    "int" => causm_core::TypeName::Builtin(
                                        causm_core::BuiltinType::Integer,
                                    ),
                                    "float" => causm_core::TypeName::Builtin(
                                        causm_core::BuiltinType::Float,
                                    ),
                                    "bool" => causm_core::TypeName::Builtin(
                                        causm_core::BuiltinType::Bool,
                                    ),
                                    "string" => causm_core::TypeName::Builtin(
                                        causm_core::BuiltinType::String,
                                    ),
                                    _ => causm_core::TypeName::Custom(t_name),
                                };
                                payload_types.push(typ);
                                self.bump();
                            } else {
                                self.bump();
                            }
                            if self.peek() == &TokenKind::Comma {
                                self.bump();
                            }
                        }
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                    }
                    variants_vec.push(causm_core::EnumVariantDef {
                        name: name_str,
                        payload_types,
                    });
                    if self.peek() == &TokenKind::Comma {
                        self.bump();
                    }
                } else {
                    self.bump();
                }
            }
            if self.peek() == &TokenKind::RBrace {
                self.bump();
            }
        }
        let id = self.arena.alloc_stmt(
            StmtNode::EnumDecl {
                name,
                variants: variants_vec,
            },
            enum_tok.span,
        );
        Ok(Some(id))
    }
}
