use crate::parser::arena_parser::ArenaParser;
use causm_core::arena::{StmtId, StmtNode, SliceRange};
use crate::parser::lexer::TokenKind;

impl<'a> ArenaParser<'a> {
    pub fn parse_on_stmt(&mut self) -> Result<Option<StmtId>, String> {
let on_tok = self.bump();
                let mut pat_parts = Vec::new();
                if let TokenKind::Ident(s) = self.peek() {
                    pat_parts.push(causm_core::symbol::resolve(*s));
                    self.bump();
                    while self.peek() == &TokenKind::DoubleColon {
                        self.bump();
                        if let TokenKind::Ident(next_s) = self.peek() {
                            pat_parts.push(causm_core::symbol::resolve(*next_s));
                            self.bump();
                        }
                    }
                }
                let mut taking_ms = None;
                if self.peek() == &TokenKind::Taking {
                    self.bump();
                    if let TokenKind::Int(ms) = self.peek() {
                        taking_ms = Some(*ms as u64);
                        self.bump();
                    } else if let TokenKind::Duration(ms) = self.peek() {
                        taking_ms = Some(*ms);
                        self.bump();
                    }
                }
                let body = self.parse_block()?;
                let handler_name = causm_core::symbol::intern(&pat_parts.join("::"));
                let id = self.arena.alloc_stmt(
                    StmtNode::RoutineDef {
                        name: handler_name,
                        params: SliceRange::new(0, 0),
                        return_type: None,
                        taking_ms,
                        state_constraint: None,
                        required_capabilities: Vec::new(),
                        body,
                    },
                    on_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_enable_stmt(&mut self) -> Result<Option<StmtId>, String> {
let enable_tok = self.bump();
                let res_name = match self.peek() {
                    TokenKind::Ident(sym) => {
                        let s = *sym;
                        self.bump();
                        causm_core::symbol::resolve(s)
                    }
                    _ => "".into(),
                };
                let mut amount = 0u64;
                let mut unit = None;
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    match self.peek() {
                        TokenKind::Int(i) => {
                            amount = *i as u64;
                            self.bump();
                        }
                        TokenKind::Duration(d) => {
                            amount = *d;
                            unit = Some("ms".to_string());
                            self.bump();
                        }
                        _ => {}
                    }
                    if let TokenKind::Ident(u_sym) = self.peek() {
                        unit = Some(causm_core::symbol::resolve(*u_sym));
                        self.bump();
                    }
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                }
                let id = self.arena.alloc_stmt(
                    StmtNode::EnableResource {
                        resource: causm_core::symbol::intern(&res_name),
                        amount,
                        unit: unit.map(|u| causm_core::symbol::intern(&u)),
                    },
                    enable_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_split_stmt(&mut self) -> Result<Option<StmtId>, String> {
let s_tok = self.bump();
                let parent = match self.peek() {
                    TokenKind::Ident(sym) => *sym,
                    _ => causm_core::symbol::intern("main"),
                };
                if matches!(self.peek(), TokenKind::Ident(_)) {
                    self.bump();
                }
                if self.peek() == &TokenKind::Into {
                    self.bump();
                }
                let mut branches_vec = Vec::new();
                if self.peek() == &TokenKind::LBracket {
                    self.bump();
                    while self.peek() != &TokenKind::RBracket
                        && self.peek() != &TokenKind::Eof
                    {
                        if let TokenKind::Ident(b_sym) = self.peek() {
                            branches_vec.push(*b_sym);
                            self.bump();
                        } else {
                            self.bump();
                        }
                        if self.peek() == &TokenKind::Comma {
                            self.bump();
                        }
                    }
                    if self.peek() == &TokenKind::RBracket {
                        self.bump();
                    }
                }
                let start = self.arena.symbol_pool.len();
                for b in branches_vec {
                    self.arena.symbol_pool.push(b);
                }
                let end = self.arena.symbol_pool.len();
                let id = self.arena.alloc_stmt(
                    StmtNode::Split {
                        parent,
                        branches: SliceRange::new(start, end),
                    },
                    s_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_merge_stmt(&mut self) -> Result<Option<StmtId>, String> {
let m_tok = self.bump();
                let mut branches_vec = Vec::new();
                if self.peek() == &TokenKind::LBracket {
                    self.bump();
                    while self.peek() != &TokenKind::RBracket
                        && self.peek() != &TokenKind::Eof
                    {
                        if let TokenKind::Ident(b_sym) = self.peek() {
                            branches_vec.push(*b_sym);
                            self.bump();
                        } else {
                            self.bump();
                        }
                        if self.peek() == &TokenKind::Comma {
                            self.bump();
                        }
                    }
                    if self.peek() == &TokenKind::RBracket {
                        self.bump();
                    }
                }
                if self.peek() == &TokenKind::Into {
                    self.bump();
                }
                let target = match self.peek() {
                    TokenKind::Ident(sym) => *sym,
                    _ => causm_core::symbol::intern("main"),
                };
                if matches!(self.peek(), TokenKind::Ident(_)) {
                    self.bump();
                }
                let mut auto_reconcile = false;
                let mut res_rules = std::collections::HashMap::new();
                let mut taking_ms = None;
                if self.peek() == &TokenKind::Taking
                    || self.peek() == &TokenKind::For
                {
                    self.bump();
                    if let Some(ms) = self.parse_optional_duration_limit() {
                        taking_ms = Some(ms);
                    }
                } else if matches!(
                    self.peek(),
                    TokenKind::Duration(_) | TokenKind::Int(_)
                ) {
                    taking_ms = self.parse_optional_duration_limit();
                }

                if self.peek() == &TokenKind::Reconcile {
                    self.bump();
                    if self.peek() == &TokenKind::Auto
                        || matches!(self.peek().as_ident_symbol(), Some(s) if causm_core::symbol::resolve(s) == "auto")
                        || matches!(self.peek(), TokenKind::Ident(s) if causm_core::symbol::resolve(*s) == "auto")
                    {
                        self.bump();
                        auto_reconcile = true;
                    } else if self.peek() == &TokenKind::LParen {
                        self.bump();
                        while self.peek() != &TokenKind::RParen
                            && self.peek() != &TokenKind::Eof
                        {
                            let mut key_opt = None;
                            if let TokenKind::Ident(k_sym) = self.peek() {
                                key_opt = Some(causm_core::symbol::resolve(*k_sym));
                                self.bump();
                            } else if let TokenKind::Str(s) = self.peek() {
                                key_opt = Some(s.clone());
                                self.bump();
                            }

                            if let Some(key) = key_opt {
                                if self.peek() == &TokenKind::Colon
                                    || self.peek() == &TokenKind::Eq
                                {
                                    self.bump();
                                    let strat = self.parse_resolution_strategy()?;
                                    res_rules.insert(key, strat);
                                }
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
                    } else {
                        auto_reconcile = true;
                    }
                }

                let start = self.arena.symbol_pool.len();
                for b in branches_vec {
                    self.arena.symbol_pool.push(b);
                }
                let end = self.arena.symbol_pool.len();
                let resolutions = causm_core::arena::ArenaMergeResolution {
                    rules: res_rules,
                    auto: auto_reconcile,
                    fallback: None,
                    taking_ms,
                };
                let id = self.arena.alloc_stmt(
                    StmtNode::Merge {
                        branches: SliceRange::new(start, end),
                        target,
                        resolutions,
                    },
                    m_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_send_stmt(&mut self) -> Result<Option<StmtId>, String> {
let mut clone = self.stream.clone();
                let mut is_channel_send = false;
                let next_tok = clone.next_token();
                if next_tok.kind == TokenKind::LParen {
                    let mut depth = 1;
                    while depth > 0 {
                        let t = clone.next_token();
                        if t.kind == TokenKind::Eof {
                            break;
                        }
                        match t.kind {
                            TokenKind::LParen => depth += 1,
                            TokenKind::RParen => depth -= 1,
                            _ => {}
                        }
                    }
                    if clone.next_token().kind == TokenKind::To {
                        is_channel_send = true;
                    }
                }
                if !is_channel_send {
                    let expr = self.parse_expression(0)?;
                    let id = self
                        .arena
                        .alloc_stmt(StmtNode::Expr(expr), self.current.span.clone());
                    return Ok(Some(id));
                }

                let send_tok = self.bump();
                let mut payload = causm_core::arena::ExprId(0);
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    if let TokenKind::Ident(s) = self.peek() {
                        let name = causm_core::symbol::resolve(*s);
                        if matches!(
                            name.as_str(),
                            "consume" | "clone" | "decay" | "peek"
                        ) {
                            self.bump();
                        }
                    }
                    payload = self.parse_expression(0)?;
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                }
                if self.peek() == &TokenKind::To {
                    self.bump();
                }
                let target = match self.peek() {
                    TokenKind::Ident(sym) => *sym,
                    _ => causm_core::symbol::intern("target"),
                };
                if matches!(self.peek(), TokenKind::Ident(_)) {
                    self.bump();
                }
                let id = self
                    .arena
                    .alloc_stmt(StmtNode::Send { target, payload }, send_tok.span);
                Ok(Some(id))
    }

    pub fn parse_ondecay_stmt(&mut self) -> Result<Option<StmtId>, String> {
let d_tok = self.bump();
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                }
                let type_name = match self.peek() {
                    TokenKind::Ident(s) => *s,
                    _ => causm_core::symbol::intern(""),
                };
                if matches!(self.peek(), TokenKind::Ident(_)) {
                    self.bump();
                }
                if self.peek() == &TokenKind::RParen {
                    self.bump();
                }
                let body = self.parse_block()?;
                let id = self.arena.alloc_stmt(
                    StmtNode::DecayHandler { type_name, body },
                    d_tok.span,
                );
                Ok(Some(id))
    }

}

