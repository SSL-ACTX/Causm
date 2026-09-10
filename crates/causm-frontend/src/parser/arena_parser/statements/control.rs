use crate::parser::arena_parser::ArenaParser;
use causm_core::arena::{StmtId, StmtNode, SliceRange};
use crate::parser::lexer::TokenKind;

impl<'a> ArenaParser<'a> {
    pub fn parse_if_stmt(&mut self) -> Result<Option<StmtId>, String> {
let if_tok = self.bump();
                if self.peek() == &TokenKind::Let {
                    self.bump();
                    let mut pat_str = String::new();
                    let mut paren_depth = 0usize;
                    while self.peek() != &TokenKind::Eof {
                        if paren_depth == 0 && self.peek() == &TokenKind::Eq {
                            self.bump();
                            break;
                        }
                        match self.peek() {
                            TokenKind::Ident(s) => {
                                pat_str.push_str(&causm_core::symbol::resolve(*s));
                            }
                            TokenKind::DoubleColon => pat_str.push_str("::"),
                            TokenKind::LParen => {
                                paren_depth += 1;
                                pat_str.push('(');
                            }
                            TokenKind::RParen => {
                                paren_depth = paren_depth.saturating_sub(1);
                                pat_str.push(')');
                            }
                            TokenKind::Comma => pat_str.push(','),
                            TokenKind::Int(i) => pat_str.push_str(&i.to_string()),
                            TokenKind::Str(s) => {
                                pat_str.push('"');
                                pat_str.push_str(s);
                                pat_str.push('"');
                            }
                            _ => pat_str.push('_'),
                        }
                        self.bump();
                    }
                    let pattern = causm_core::symbol::intern(&pat_str);
                    let expr = self.parse_expression_no_struct_lit(0)?;
                    let then_branch = self.parse_block()?;
                    let else_branch = if self.peek() == &TokenKind::Else {
                        self.bump();
                        Some(self.parse_block()?)
                    } else {
                        None
                    };
                    let mut reconcile_auto = false;
                    if self.peek() == &TokenKind::Reconcile {
                        self.bump();
                        if self.peek() == &TokenKind::Auto {
                            self.bump();
                            reconcile_auto = true;
                        }
                    }
                    let id = self.arena.alloc_stmt(
                        StmtNode::IfLet {
                            pattern,
                            expr,
                            then_branch,
                            else_branch,
                            reconcile_auto,
                        },
                        if_tok.span,
                    );
                    return Ok(Some(id));
                }
                let cond = self.parse_expression_no_struct_lit(0)?;
                let then_branch = self.parse_block()?;
                let else_branch = if self.peek() == &TokenKind::Else {
                    self.bump();
                    Some(self.parse_block()?)
                } else {
                    None
                };
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
                        reconcile_auto = true;
                    } else if self.peek() == &TokenKind::LBrace {
                        self.bump();
                        while self.peek() != &TokenKind::RBrace
                            && self.peek() != &TokenKind::Eof
                        {
                            self.bump();
                        }
                        if self.peek() == &TokenKind::RBrace {
                            self.bump();
                        }
                        reconcile_auto = true;
                    }
                }
                let id = self.arena.alloc_stmt(
                    StmtNode::If {
                        cond,
                        then_branch,
                        else_branch,
                        reconcile_auto,
                    },
                    if_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_using_stmt(&mut self) -> Result<Option<StmtId>, String> {
let u_tok = self.bump();
                let binding = match self.peek() {
                    TokenKind::Ident(sym) => *sym,
                    _ => return Err("Expected identifier after using".into()),
                };
                self.bump();
                if self.peek() == &TokenKind::Eq {
                    self.bump();
                }
                let resource = self.parse_expression(0)?;
                let body = self.parse_block()?;
                let id = self.arena.alloc_stmt(
                    StmtNode::Using {
                        binding,
                        resource,
                        body,
                    },
                    u_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_loop_stmt(&mut self) -> Result<Option<StmtId>, String> {
let loop_tok = self.bump();
                if self.peek() == &TokenKind::On {
                    self.bump();
                    let target = self.parse_expression(0)?;
                    let body = self.parse_block()?;
                    let id = self.arena.alloc_stmt(
                        StmtNode::LoopOn { target, body },
                        loop_tok.span,
                    );
                    return Ok(Some(id));
                }
                let (max_ms, step_ms, _has_step, explicit_tick) =
                    self.parse_optional_loop_modifiers();
                let body = self.parse_block()?;
                let is_tick =
                    explicit_tick || (max_ms.is_none() && step_ms.is_none());
                let id = self.arena.alloc_stmt(
                    StmtNode::Loop {
                        max_ms,
                        step_ms,
                        is_tick,
                        body,
                    },
                    loop_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_match_stmt(&mut self) -> Result<Option<StmtId>, String> {
let match_tok = self.bump();
                let target = self.parse_expression(0)?;
                let mut arms = Vec::new();
                if self.peek() == &TokenKind::LBrace {
                    self.bump();
                    while self.peek() != &TokenKind::RBrace
                        && self.peek() != &TokenKind::Eof
                    {
                        let mut pat_str = String::new();
                        let mut paren_depth = 0usize;
                        let mut brace_depth = 0usize;
                        let mut bracket_depth = 0usize;
                        while self.peek() != &TokenKind::Eof {
                            let at_top = paren_depth == 0
                                && brace_depth == 0
                                && bracket_depth == 0;
                            if at_top
                                && (self.peek() == &TokenKind::FatArrow
                                    || self.peek() == &TokenKind::Colon
                                    || self.peek() == &TokenKind::If
                                    || self.peek() == &TokenKind::RBrace)
                            {
                                break;
                            }
                            match self.peek() {
                                TokenKind::Ident(s) => {
                                    pat_str
                                        .push_str(&causm_core::symbol::resolve(*s));
                                }
                                TokenKind::DoubleColon => {
                                    pat_str.push_str("::");
                                }
                                TokenKind::LParen => {
                                    paren_depth += 1;
                                    pat_str.push('(');
                                }
                                TokenKind::RParen => {
                                    paren_depth = paren_depth.saturating_sub(1);
                                    pat_str.push(')');
                                }
                                TokenKind::LBrace => {
                                    brace_depth += 1;
                                    pat_str.push('{');
                                }
                                TokenKind::RBrace => {
                                    brace_depth = brace_depth.saturating_sub(1);
                                    pat_str.push('}');
                                }
                                TokenKind::LBracket => {
                                    bracket_depth += 1;
                                    pat_str.push('[');
                                }
                                TokenKind::RBracket => {
                                    bracket_depth = bracket_depth.saturating_sub(1);
                                    pat_str.push(']');
                                }
                                TokenKind::Eq => {
                                    pat_str.push('=');
                                }
                                TokenKind::Comma => {
                                    pat_str.push(',');
                                }
                                TokenKind::Int(i) => {
                                    pat_str.push_str(&i.to_string());
                                }
                                TokenKind::Str(s) => {
                                    pat_str.push('"');
                                    pat_str.push_str(s);
                                    pat_str.push('"');
                                }
                                TokenKind::Bool(b) => {
                                    pat_str.push_str(if *b {
                                        "true"
                                    } else {
                                        "false"
                                    });
                                }
                                TokenKind::Valid => {
                                    pat_str.push_str("Valid");
                                }
                                TokenKind::Decayed => {
                                    pat_str.push_str("Decayed");
                                }
                                TokenKind::Pending => {
                                    pat_str.push_str("Pending");
                                }
                                TokenKind::Consumed => {
                                    pat_str.push_str("Consumed");
                                }
                                _ => {
                                    pat_str.push('_');
                                }
                            }
                            self.bump();
                        }
                        let pat_sym = causm_core::symbol::intern(&pat_str);
                        let guard = if self.peek() == &TokenKind::If {
                            self.bump(); // consume 'if'
                            Some(self.parse_expression(0)?)
                        } else {
                            None
                        };
                        if self.peek() == &TokenKind::Colon
                            || self.peek() == &TokenKind::FatArrow
                        {
                            self.bump();
                        }
                        let body_slice = if self.peek() == &TokenKind::LBrace {
                            self.parse_block()?
                        } else {
                            let start = self.arena.stmt_pool.len();
                            while self.peek() != &TokenKind::RBrace
                                && self.peek() != &TokenKind::Eof
                            {
                                let is_next_arm = match self.peek() {
                                    TokenKind::Valid
                                    | TokenKind::Decayed
                                    | TokenKind::Pending
                                    | TokenKind::Consumed => true,
                                    TokenKind::Ident(_) => {
                                        let clone = self.stream.clone();
                                        matches!(
                                            clone.peek_token().kind,
                                            TokenKind::Colon
                                                | TokenKind::FatArrow
                                                | TokenKind::LParen
                                        )
                                    }
                                    _ => false,
                                };
                                if is_next_arm && self.arena.stmt_pool.len() > start
                                {
                                    break;
                                }
                                if let Some(sid) = self.parse_statement()? {
                                    self.arena.stmt_pool.push(sid);
                                } else {
                                    break;
                                }
                            }
                            let end = self.arena.stmt_pool.len();
                            SliceRange::new(start, end)
                        };
                        arms.push(causm_core::arena::MatchArmNode {
                            pattern: pat_sym,
                            guard,
                            body: body_slice,
                        });
                        if self.peek() == &TokenKind::Comma
                            || self.peek() == &TokenKind::Semi
                        {
                            self.bump();
                        }
                    }
                    if self.peek() == &TokenKind::RBrace {
                        self.bump();
                    }
                }
                let a_start = self.arena.match_arms_pool.len();
                for arm in arms {
                    self.arena.match_arms_pool.push(arm);
                }
                let a_end = self.arena.match_arms_pool.len();
                let id = self.arena.alloc_stmt(
                    StmtNode::Match {
                        target,
                        arms: SliceRange::new(a_start, a_end),
                    },
                    match_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_for_stmt(&mut self) -> Result<Option<StmtId>, String> {
let for_tok = self.bump();
                let var_name = if let Some(sym) = self.peek().as_ident_symbol() {
                    sym
                } else {
                    causm_core::symbol::intern("i")
                };
                self.bump();
                let mut mode = causm_core::ParamMode::Peek;
                if self.peek() == &TokenKind::In {
                    self.bump();
                } else if let TokenKind::Ident(m_sym) = self.peek() {
                    let m_str = causm_core::symbol::resolve(*m_sym);
                    match m_str.as_str() {
                        "consume" => {
                            mode = causm_core::ParamMode::Consume;
                            self.bump();
                        }
                        "peek" => {
                            mode = causm_core::ParamMode::Peek;
                            self.bump();
                        }
                        "clone" => {
                            mode = causm_core::ParamMode::Clone;
                            self.bump();
                        }
                        "decay" => {
                            mode = causm_core::ParamMode::Decay;
                            self.bump();
                        }
                        "lease" => {
                            mode = causm_core::ParamMode::Lease;
                            self.bump();
                        }
                        _ => {}
                    }
                    if self.peek() == &TokenKind::In {
                        self.bump();
                    }
                }
                let start_or_iter = self.parse_expression(0)?;
                if self.peek() == &TokenKind::DotDot
                    || self.peek() == &TokenKind::DotDotEq
                {
                    self.bump();
                    let end_expr = self.parse_expression(0)?;
                    let (_max_ms, parsed_step, _, _) =
                        self.parse_optional_loop_modifiers();
                    let step_ms = parsed_step.unwrap_or(1);
                    let body = self.parse_block()?;
                    let id = self.arena.alloc_stmt(
                        StmtNode::ForStep {
                            var_name,
                            start_expr: start_or_iter,
                            end_expr,
                            step_ms,
                            body,
                        },
                        for_tok.span,
                    );
                    Ok(Some(id))
                } else {
                    let (max_ms, parsed_step, has_step_or_pacing, _) =
                        self.parse_optional_loop_modifiers();
                    let body = self.parse_block()?;
                    let is_for_step = mode == causm_core::ParamMode::Peek
                        && max_ms.is_none()
                        && has_step_or_pacing;
                    if is_for_step {
                        let step_ms = parsed_step.unwrap_or(0);
                        let id = self.arena.alloc_stmt(
                            StmtNode::ForStep {
                                var_name,
                                start_expr: start_or_iter,
                                end_expr: start_or_iter,
                                step_ms,
                                body,
                            },
                            for_tok.span,
                        );
                        Ok(Some(id))
                    } else {
                        let id = self.arena.alloc_stmt(
                            StmtNode::For {
                                var_name,
                                mode,
                                iter_expr: start_or_iter,
                                pacing_ms: parsed_step,
                                max_ms,
                                body,
                            },
                            for_tok.span,
                        );
                        Ok(Some(id))
                    }
                }
    }

}

