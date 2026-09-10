use crate::parser::arena_parser::ArenaParser;
use causm_core::arena::{StmtId, StmtNode, SliceRange};
use crate::parser::lexer::TokenKind;

impl<'a> ArenaParser<'a> {
    pub fn parse_at_stmt(&mut self) -> Result<Option<StmtId>, String> {
let at_tok = self.bump();
                let mut coord_prefix = String::new();
                if self.peek() == &TokenKind::Plus {
                    self.bump();
                    coord_prefix.push('+');
                } else if self.peek() == &TokenKind::Minus {
                    self.bump();
                    coord_prefix.push('-');
                }

                let is_attr = match self.peek() {
                    TokenKind::Ident(sym) => {
                        let name = causm_core::symbol::resolve(*sym);
                        name == "derive"
                            || name == "inline"
                            || name == "test"
                            || name == "doc"
                    }
                    _ => false,
                };

                if is_attr {
                    let attr_tok = self.bump();
                    let attr_name = match attr_tok.kind {
                        TokenKind::Ident(s) => causm_core::symbol::resolve(s),
                        _ => "".into(),
                    };
                    let mut attr_args = Vec::new();
                    if self.peek() == &TokenKind::LParen {
                        self.bump();
                        while self.peek() != &TokenKind::RParen
                            && self.peek() != &TokenKind::Eof
                        {
                            if let TokenKind::Ident(s) = self.peek() {
                                attr_args.push(causm_core::symbol::resolve(*s));
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
                    let kind = match attr_name.as_str() {
                        "derive" => causm_core::AttributeKind::Derive(attr_args),
                        "inline" => causm_core::AttributeKind::Inline,
                        "test" => causm_core::AttributeKind::Test,
                        _ => causm_core::AttributeKind::Custom {
                            name: attr_name,
                            args: attr_args,
                        },
                    };
                    self.pending_attributes.push(causm_core::Attribute {
                        kind,
                        span: at_tok.span,
                    });
                    return self.parse_statement();
                }

                let mut is_relative = coord_prefix == "+";
                let mut coord: Option<causm_core::TimeCoordinate> = None;
                let mut directives = Vec::new();

                loop {
                    if self.peek() == &TokenKind::Plus {
                        is_relative = true;
                        self.bump();
                    }
                    match self.peek() {
                        TokenKind::Int(ms) => {
                            coord = if is_relative {
                                Some(causm_core::TimeCoordinate::Relative(
                                    *ms as u64,
                                ))
                            } else {
                                Some(causm_core::TimeCoordinate::Global(*ms as u64))
                            };
                            self.bump();
                        }
                        TokenKind::Duration(ms) => {
                            coord = if is_relative {
                                Some(causm_core::TimeCoordinate::Relative(*ms))
                            } else {
                                Some(causm_core::TimeCoordinate::Global(*ms))
                            };
                            self.bump();
                        }
                        TokenKind::Ident(sym) => {
                            let s = causm_core::symbol::resolve(*sym);
                            match s.as_str() {
                                "every" => {
                                    self.bump();
                                    if let TokenKind::Duration(ms) = self.peek() {
                                        coord = Some(
                                            causm_core::TimeCoordinate::Periodic(
                                                *ms,
                                            ),
                                        );
                                        self.bump();
                                    } else if let TokenKind::Int(ms) = self.peek() {
                                        coord = Some(
                                            causm_core::TimeCoordinate::Periodic(
                                                *ms as u64,
                                            ),
                                        );
                                        self.bump();
                                    }
                                    continue;
                                }
                                "no_z3" => {
                                    directives.push(causm_core::BlockDirective::NoZ3)
                                }
                                "chaos" => directives
                                    .push(causm_core::BlockDirective::Chaos),
                                "deterministic" => directives
                                    .push(causm_core::BlockDirective::Deterministic),
                                _ => {
                                    if coord.is_none() {
                                        coord = Some(
                                            causm_core::TimeCoordinate::Branch(s),
                                        );
                                    }
                                }
                            }
                            self.bump();
                        }
                        _ => {}
                    }
                    if self.peek() == &TokenKind::Comma {
                        self.bump();
                        if self.peek() == &TokenKind::At {
                            self.bump();
                        }
                    } else if self.peek() == &TokenKind::At {
                        self.bump();
                    } else {
                        break;
                    }
                }
                if self.peek() == &TokenKind::Colon {
                    self.bump();
                }
                let body = self.parse_block()?;
                let final_coord =
                    coord.unwrap_or(causm_core::TimeCoordinate::Global(0));
                let id = self.arena.alloc_stmt(
                    StmtNode::TimelineBlock {
                        coord: final_coord,
                        directives,
                        body,
                    },
                    at_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_lease_stmt(&mut self) -> Result<Option<StmtId>, String> {
let l_tok = self.bump();
                let binding = match self.peek().as_ident_symbol() {
                    Some(s) => s,
                    None => causm_core::symbol::intern("res"),
                };
                self.bump();
                if self.peek() == &TokenKind::Eq {
                    self.bump();
                }
                let source = match self.peek().as_ident_symbol() {
                    Some(s) => s,
                    None => causm_core::symbol::intern("src"),
                };
                self.bump();
                let mut duration_ms = 1000;
                if self.peek() == &TokenKind::For
                    || self.peek() == &TokenKind::Taking
                {
                    self.bump();
                }
                if let Some(ms) = self.parse_optional_duration_limit() {
                    duration_ms = ms;
                }
                let body = self.parse_block()?;
                let mut reconcile_auto = false;
                if self.peek() == &TokenKind::Reconcile {
                    self.bump();
                    if self.peek() == &TokenKind::Auto {
                        self.bump();
                        reconcile_auto = true;
                    } else if self.peek() == &TokenKind::LParen {
                        self.bump();
                        while self.peek() != &TokenKind::RParen
                            && self.peek() != &TokenKind::Eof
                        {
                            self.bump();
                        }
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                    }
                }
                let id = self.arena.alloc_stmt(
                    StmtNode::Lease {
                        binding,
                        source,
                        duration_ms,
                        body,
                        reconcile_auto,
                    },
                    l_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_policy_stmt(&mut self) -> Result<Option<StmtId>, String> {
let p_tok = self.bump();
                let target = match self.peek() {
                    TokenKind::Ident(s) => *s,
                    _ => causm_core::symbol::intern(""),
                };
                self.bump();
                if self.peek() == &TokenKind::Eq {
                    self.bump();
                }
                let kind = match self.peek() {
                    TokenKind::Ident(s) => *s,
                    _ => causm_core::symbol::intern(""),
                };
                self.bump();
                let id = self
                    .arena
                    .alloc_stmt(StmtNode::Policy { target, kind }, p_tok.span);
                Ok(Some(id))
    }

    pub fn parse_select_stmt(&mut self) -> Result<Option<StmtId>, String> {
let s_tok = self.bump();
                let max_ms = self.parse_optional_duration_limit().unwrap_or(1000);
                let c_start = self.arena.stmt_pool.len();
                if self.peek() == &TokenKind::LBrace {
                    self.bump();
                    while self.peek() != &TokenKind::RBrace
                        && self.peek() != &TokenKind::Eof
                    {
                        if self.peek() == &TokenKind::Case {
                            self.bump();
                            let case_stmt = self.parse_statement()?;
                            if let Some(cs) = case_stmt {
                                self.arena.stmt_pool.push(cs);
                            }
                        } else if self.peek() == &TokenKind::Timeout {
                            self.bump();
                            if self.peek() == &TokenKind::Colon {
                                self.bump();
                            }
                            let _timeout_body = self.parse_block()?;
                        } else {
                            self.bump();
                        }
                    }
                    if self.peek() == &TokenKind::RBrace {
                        self.bump();
                    }
                }
                if self.peek() == &TokenKind::Reconcile {
                    self.bump();
                    if self.peek() == &TokenKind::Auto {
                        self.bump();
                    } else if self.peek() == &TokenKind::LParen {
                        self.bump();
                        while self.peek() != &TokenKind::RParen
                            && self.peek() != &TokenKind::Eof
                        {
                            self.bump();
                        }
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                    }
                }
                let c_end = self.arena.stmt_pool.len();
                let id = self.arena.alloc_stmt(
                    StmtNode::Select {
                        max_ms,
                        cases: SliceRange::new(c_start, c_end),
                    },
                    s_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_entangle_stmt(&mut self) -> Result<Option<StmtId>, String> {
let ent_tok = self.bump();
                let mut symbols = Vec::new();
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    while self.peek() != &TokenKind::RParen
                        && self.peek() != &TokenKind::Eof
                    {
                        if let TokenKind::Ident(sym) = self.peek() {
                            symbols.push(*sym);
                            self.bump();
                            if self.peek() == &TokenKind::Comma {
                                self.bump();
                            }
                        } else {
                            self.bump();
                        }
                    }
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                }
                let s_start = self.arena.symbol_pool.len();
                for s in symbols {
                    self.arena.symbol_pool.push(s);
                }
                let s_end = self.arena.symbol_pool.len();
                let id = self.arena.alloc_stmt(
                    StmtNode::Entangle(SliceRange::new(s_start, s_end)),
                    ent_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_speculate_stmt(&mut self) -> Result<Option<StmtId>, String> {
let spec_tok = self.bump();
                let max_ms = self.parse_optional_duration_limit().unwrap_or(0);
                let body = self.parse_block()?;
                let mut fallback = None;
                if self.peek() == &TokenKind::Fallback {
                    self.bump();
                    let fb = self.parse_block()?;
                    fallback = Some(fb);
                }
                let id = self.arena.alloc_stmt(
                    StmtNode::Speculate {
                        max_ms,
                        body,
                        fallback,
                    },
                    spec_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_asserttime_stmt(&mut self) -> Result<Option<StmtId>, String> {
let a_tok = self.bump();
                let mut operator = causm_core::BinaryOperator::Eq;
                let mut limit_ms = 0;
                let mut fallback = None;
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    if let TokenKind::Ident(s) = self.peek() {
                        if causm_core::symbol::resolve(*s) == "elapsed" {
                            self.bump();
                        }
                    }
                    operator = match self.peek() {
                        TokenKind::EqEq => {
                            self.bump();
                            causm_core::BinaryOperator::Eq
                        }
                        TokenKind::BangEq => {
                            self.bump();
                            causm_core::BinaryOperator::Neq
                        }
                        TokenKind::Lt => {
                            self.bump();
                            causm_core::BinaryOperator::Lt
                        }
                        TokenKind::LtEq => {
                            self.bump();
                            causm_core::BinaryOperator::Le
                        }
                        TokenKind::Gt => {
                            self.bump();
                            causm_core::BinaryOperator::Gt
                        }
                        TokenKind::GtEq => {
                            self.bump();
                            causm_core::BinaryOperator::Ge
                        }
                        _ => causm_core::BinaryOperator::Eq,
                    };
                    if let TokenKind::Duration(ms) = self.peek() {
                        limit_ms = *ms;
                        self.bump();
                    } else if let TokenKind::Int(ms) = self.peek() {
                        limit_ms = *ms as u64;
                        self.bump();
                    }
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                }
                if self.peek() == &TokenKind::LBrace {
                    let fb = self.parse_block()?;
                    fallback = Some(fb);
                }
                let id = self.arena.alloc_stmt(
                    StmtNode::AssertTime {
                        operator,
                        limit_ms,
                        fallback,
                    },
                    a_tok.span,
                );
                Ok(Some(id))
    }

}

