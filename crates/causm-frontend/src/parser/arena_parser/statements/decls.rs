use crate::parser::arena_parser::ArenaParser;
use causm_core::arena::{StmtId, StmtNode, SliceRange};
use crate::parser::lexer::TokenKind;

impl<'a> ArenaParser<'a> {
    pub fn parse_let_stmt(&mut self) -> Result<Option<StmtId>, String> {
let let_tok = self.bump();
                let _is_mut = if self.peek() == &TokenKind::Mut {
                    self.bump();
                    true
                } else {
                    false
                };
                if self.peek() == &TokenKind::LBrace {
                    self.bump();
                    let f_start = self.arena.symbol_pool.len();
                    while self.peek() != &TokenKind::RBrace
                        && self.peek() != &TokenKind::Eof
                    {
                        let field_sym = match self.peek() {
                            TokenKind::Ident(s) => *s,
                            _ => causm_core::symbol::intern("_"),
                        };
                        self.bump();
                        let target_sym = if self.peek() == &TokenKind::As {
                            self.bump();
                            match self.peek() {
                                TokenKind::Ident(s) => {
                                    let sym = *s;
                                    self.bump();
                                    sym
                                }
                                _ => field_sym,
                            }
                        } else {
                            field_sym
                        };
                        self.arena.symbol_pool.push(field_sym);
                        self.arena.symbol_pool.push(target_sym);
                        if self.peek() == &TokenKind::Comma {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    if self.peek() == &TokenKind::RBrace {
                        self.bump();
                    }
                    if self.peek() == &TokenKind::Eq {
                        self.bump();
                    }
                    let expr = self.parse_expression(0)?;
                    if self.peek() == &TokenKind::Semi {
                        self.bump();
                    }
                    let f_end = self.arena.symbol_pool.len();
                    let id = self.arena.alloc_stmt(
                        StmtNode::Destructure {
                            fields: SliceRange::new(f_start, f_end),
                            expr,
                        },
                        let_tok.span,
                    );
                    return Ok(Some(id));
                }
                let mut lifetime_annot: Option<causm_core::LifetimeAnnotation> =
                    None;
                if self.peek() == &TokenKind::At {
                    self.bump();
                    let annot_name = match self.peek() {
                        TokenKind::Ident(s) => Some(causm_core::symbol::resolve(*s)),
                        TokenKind::Valid => Some("valid".to_string()),
                        TokenKind::Decayed => Some("decayed".to_string()),
                        _ => None,
                    };
                    if let Some(name) = annot_name {
                        self.bump();
                        if name == "valid" {
                            lifetime_annot =
                                Some(causm_core::LifetimeAnnotation::Valid);
                        } else if name == "decay_rate" || name == "decayed" {
                            let mut ms = 0u64;
                            if self.peek() == &TokenKind::LParen {
                                self.bump();
                                match self.peek() {
                                    TokenKind::Int(i) => ms = *i as u64,
                                    TokenKind::Duration(d) => ms = *d,
                                    _ => {}
                                }
                                self.bump();
                                if self.peek() == &TokenKind::RParen {
                                    self.bump();
                                }
                            }
                            lifetime_annot = Some(if name == "decay_rate" {
                                causm_core::LifetimeAnnotation::DecayRate(ms)
                            } else {
                                causm_core::LifetimeAnnotation::Decayed(ms)
                            });
                        }
                    }
                }

                let target = if let Some(s) = self.peek().as_ident_symbol() {
                    self.bump();
                    s
                } else {
                    return Err(format!(
                        "Expected identifier after 'let', found {:?}",
                        self.peek()
                    ));
                };
                let mut type_annotation = None;
                if self.peek() == &TokenKind::Colon {
                    self.bump();
                    let mut t_str = String::new();
                    let mut angle_depth: usize = 0;
                    while self.peek() != &TokenKind::Eq
                        && self.peek() != &TokenKind::Semi
                        && self.peek() != &TokenKind::RBrace
                        && self.peek() != &TokenKind::Eof
                    {
                        if angle_depth == 0
                            && !t_str.is_empty()
                            && !matches!(
                                self.peek(),
                                TokenKind::Lt | TokenKind::LBracket
                            )
                        {
                            break;
                        }
                        match self.peek() {
                            TokenKind::Ident(s) => {
                                t_str.push_str(&causm_core::symbol::resolve(*s))
                            }
                            TokenKind::Lt => {
                                angle_depth += 1;
                                t_str.push('<');
                            }
                            TokenKind::Gt => {
                                angle_depth = angle_depth.saturating_sub(1);
                                t_str.push('>');
                            }
                            TokenKind::Comma => t_str.push(','),
                            TokenKind::Int(n) => t_str.push_str(&n.to_string()),
                            _ => {}
                        }
                        self.bump();
                    }
                    if !t_str.is_empty() {
                        type_annotation = Some(causm_core::symbol::intern(&t_str));
                    }
                }
                let init = if self.peek() == &TokenKind::Eq {
                    self.bump();
                    Some(self.parse_expression(0)?)
                } else {
                    None
                };
                let id = self.arena.alloc_stmt(
                    StmtNode::Let {
                        target,
                        is_mut: _is_mut,
                        type_annotation,
                        init,
                        lifetime: lifetime_annot,
                    },
                    let_tok.span,
                );
                if self.peek() == &TokenKind::Semi {
                    self.bump();
                }
                Ok(Some(id))
    }

    pub fn parse_pub_stmt(&mut self) -> Result<Option<StmtId>, String> {
let _is_pub = if self.peek() == &TokenKind::Pub {
                    self.bump();
                    true
                } else {
                    false
                };
                let r_tok = if self.peek() == &TokenKind::Routine {
                    self.bump()
                } else {
                    return Err("Expected 'routine' keyword".into());
                };
                let mut name_str = match self.peek() {
                    TokenKind::Ident(sym) => causm_core::symbol::resolve(*sym),
                    TokenKind::Send => "send".to_string(),
                    _ => {
                        return Err(format!(
                            "Expected routine name, found {:?}",
                            self.peek()
                        ))
                    }
                };
                self.bump();
                if self.peek() == &TokenKind::Lt {
                    self.bump();
                    name_str.push('<');
                    let mut depth = 1;
                    while depth > 0 && self.peek() != &TokenKind::Eof {
                        match self.peek() {
                            TokenKind::Lt => {
                                depth += 1;
                                name_str.push('<');
                            }
                            TokenKind::Gt => {
                                depth -= 1;
                                name_str.push('>');
                            }
                            TokenKind::Ident(sym) => {
                                name_str
                                    .push_str(&causm_core::symbol::resolve(*sym));
                            }
                            TokenKind::Comma => {
                                name_str.push_str(", ");
                            }
                            _ => {}
                        }
                        self.bump();
                    }
                }
                while self.peek() == &TokenKind::Dot {
                    self.bump();
                    match self.peek() {
                        TokenKind::Ident(sym) => {
                            name_str.push('.');
                            name_str.push_str(&causm_core::symbol::resolve(*sym));
                            self.bump();
                        }
                        TokenKind::Send => {
                            name_str.push_str(".send");
                            self.bump();
                        }
                        TokenKind::Type => {
                            name_str.push_str(".type");
                            self.bump();
                        }
                        TokenKind::Auto => {
                            name_str.push_str(".auto");
                            self.bump();
                        }
                        _ => {}
                    }
                }
                let name = causm_core::symbol::intern(&name_str);
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                }
                let mut params_vec = Vec::new();
                while self.peek() != &TokenKind::RParen
                    && self.peek() != &TokenKind::Eof
                {
                    let mut mode_sym = causm_core::symbol::intern("peek");
                    if self.peek() == &TokenKind::Amp {
                        self.bump();
                    }
                    if let TokenKind::Ident(mode_or_param) = self.peek() {
                        let name_str = causm_core::symbol::resolve(*mode_or_param);
                        if matches!(
                            name_str.as_str(),
                            "consume" | "clone" | "decay" | "peek"
                        ) {
                            let next_tok = self.stream.peek_token();
                            if matches!(
                                next_tok.kind,
                                TokenKind::Ident(_) | TokenKind::Amp
                            ) {
                                mode_sym = *mode_or_param;
                                self.bump();
                                if self.peek() == &TokenKind::Amp {
                                    self.bump();
                                }
                            }
                        }
                    }
                    let param_sym = match self.peek() {
                        TokenKind::Ident(s) => {
                            let sym = *s;
                            self.bump();
                            Some(sym)
                        }
                        TokenKind::Type => {
                            self.bump();
                            Some(causm_core::symbol::intern("type"))
                        }
                        TokenKind::Send => {
                            self.bump();
                            Some(causm_core::symbol::intern("send"))
                        }
                        _ => None,
                    };
                    if let Some(p) = param_sym {
                        let mut typ = causm_core::symbol::intern("");
                        if self.peek() == &TokenKind::Colon {
                            self.bump();
                            let mut full_type = String::new();
                            match self.peek() {
                                TokenKind::Ident(typ_sym) => {
                                    full_type.push_str(
                                        &causm_core::symbol::resolve(*typ_sym),
                                    );
                                    self.bump();
                                }
                                TokenKind::Struct => {
                                    full_type.push_str("struct");
                                    self.bump();
                                }
                                _ => {}
                            }
                            if self.peek() == &TokenKind::Lt {
                                full_type.push('<');
                                self.bump();
                                let mut depth = 1;
                                while depth > 0 && self.peek() != &TokenKind::Eof {
                                    match self.peek() {
                                        TokenKind::Lt => {
                                            depth += 1;
                                            full_type.push('<');
                                            self.bump();
                                        }
                                        TokenKind::Gt => {
                                            depth -= 1;
                                            full_type.push('>');
                                            self.bump();
                                        }
                                        TokenKind::Ident(s) => {
                                            full_type.push_str(
                                                &causm_core::symbol::resolve(*s),
                                            );
                                            self.bump();
                                        }
                                        TokenKind::Int(n) => {
                                            full_type.push_str(&n.to_string());
                                            self.bump();
                                        }
                                        TokenKind::Duration(ms) => {
                                            full_type.push_str(&format!("{}ms", ms));
                                            self.bump();
                                        }
                                        TokenKind::Comma => {
                                            full_type.push_str(", ");
                                            self.bump();
                                        }
                                        _ => {
                                            self.bump();
                                        }
                                    }
                                }
                            }
                            if !full_type.is_empty() {
                                typ = causm_core::symbol::intern(&full_type);
                            }
                            // consume any leftover tokens in type expression until comma or rparen
                            while self.peek() != &TokenKind::Comma
                                && self.peek() != &TokenKind::RParen
                                && self.peek() != &TokenKind::Eof
                            {
                                self.bump();
                            }
                        }
                        params_vec.push(mode_sym);
                        params_vec.push(p);
                        params_vec.push(typ);
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
                let mut required_capabilities = Vec::new();
                let mut taking_ms = None;
                let mut return_type = None;
                let mut state_constraint = None;
                while self.peek() != &TokenKind::LBrace
                    && self.peek() != &TokenKind::FatArrow
                    && self.peek() != &TokenKind::Semi
                    && self.peek() != &TokenKind::Routine
                    && self.peek() != &TokenKind::RBrace
                    && self.peek() != &TokenKind::Eof
                {
                    let is_require = match self.peek() {
                        TokenKind::Require => true,
                        TokenKind::Ident(s) => {
                            let r = causm_core::symbol::resolve(*s);
                            r == "require" || r == "requires"
                        }
                        _ => false,
                    };
                    if is_require {
                        self.bump();
                        if self.peek() == &TokenKind::LBracket {
                            self.bump();
                            while self.peek() != &TokenKind::RBracket
                                && self.peek() != &TokenKind::Eof
                            {
                                let mut cap_path = String::new();
                                while matches!(
                                    self.peek(),
                                    TokenKind::Ident(_) | TokenKind::Dot
                                ) {
                                    match self.peek() {
                                        TokenKind::Ident(s) => cap_path.push_str(
                                            &causm_core::symbol::resolve(*s),
                                        ),
                                        TokenKind::Dot => cap_path.push('.'),
                                        _ => {}
                                    }
                                    self.bump();
                                }
                                if !cap_path.is_empty() {
                                    required_capabilities
                                        .push(causm_core::symbol::intern(&cap_path));
                                }
                                if self.peek() == &TokenKind::Comma {
                                    self.bump();
                                }
                            }
                            if self.peek() == &TokenKind::RBracket {
                                self.bump();
                            }
                        } else {
                            let mut cap_path = String::new();
                            while matches!(
                                self.peek(),
                                TokenKind::Ident(_) | TokenKind::Dot
                            ) {
                                match self.peek() {
                                    TokenKind::Ident(s) => cap_path
                                        .push_str(&causm_core::symbol::resolve(*s)),
                                    TokenKind::Dot => cap_path.push('.'),
                                    _ => {}
                                }
                                self.bump();
                            }
                            if !cap_path.is_empty() {
                                required_capabilities
                                    .push(causm_core::symbol::intern(&cap_path));
                            }
                        }
                        continue;
                    }
                    if self.peek() == &TokenKind::Arrow {
                        self.bump();
                        if let TokenKind::Ident(rt_sym) = self.peek() {
                            return_type = Some(*rt_sym);
                            self.bump();
                        }
                        while self.peek() != &TokenKind::Taking
                            && self.peek() != &TokenKind::Require
                            && !(self.peek() == &TokenKind::LParen
                                && self.stream.peek_token().kind
                                    == TokenKind::Taking)
                            && self.peek() != &TokenKind::LBrace
                            && self.peek() != &TokenKind::FatArrow
                            && self.peek() != &TokenKind::Semi
                            && self.peek() != &TokenKind::Eof
                        {
                            self.bump();
                        }
                        continue;
                    }
                    let has_taking_paren = if self.peek() == &TokenKind::LParen
                        && self.stream.peek_token().kind == TokenKind::Taking
                    {
                        self.bump();
                        true
                    } else {
                        false
                    };
                    if self.peek() == &TokenKind::Taking {
                        self.bump();
                        match self.peek() {
                            TokenKind::Int(ms) => {
                                taking_ms = Some(*ms as u64);
                                self.bump();
                                if let TokenKind::Ident(s) = self.peek() {
                                    if causm_core::symbol::resolve(*s) == "ms" {
                                        self.bump();
                                    }
                                }
                            }
                            TokenKind::Duration(ms) => {
                                taking_ms = Some(*ms);
                                self.bump();
                            }
                            TokenKind::Ident(_) => {
                                self.bump();
                            }
                            _ => {}
                        }
                        if has_taking_paren && self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                        if self.peek() == &TokenKind::Routine
                            || self.peek() == &TokenKind::RBrace
                            || self.peek() == &TokenKind::Semi
                        {
                            break;
                        }
                        continue;
                    }
                    if self.peek() == &TokenKind::Where {
                        self.bump();
                        let mut var_sym = causm_core::symbol::intern("self");
                        if let TokenKind::Ident(vs) = self.peek() {
                            var_sym = *vs;
                            self.bump();
                        }
                        if self.peek() == &TokenKind::Dot {
                            self.bump();
                            if self.peek() == &TokenKind::State {
                                self.bump();
                            } else if let TokenKind::Ident(s) = self.peek() {
                                if causm_core::symbol::resolve(*s) == "state" {
                                    self.bump();
                                }
                            }
                        }
                        if self.peek() == &TokenKind::EqEq {
                            self.bump();
                        }
                        let st_sym = match self.peek() {
                            TokenKind::Valid => {
                                self.bump();
                                causm_core::symbol::intern("Valid")
                            }
                            TokenKind::Decayed => {
                                self.bump();
                                causm_core::symbol::intern("Decayed")
                            }
                            TokenKind::Pending => {
                                self.bump();
                                causm_core::symbol::intern("Pending")
                            }
                            TokenKind::Consumed => {
                                self.bump();
                                causm_core::symbol::intern("Consumed")
                            }
                            TokenKind::Ident(s) => {
                                let sym = *s;
                                self.bump();
                                sym
                            }
                            _ => causm_core::symbol::intern("Valid"),
                        };
                        state_constraint = Some((var_sym, st_sym));
                        continue;
                    }
                    self.bump();
                }

                let body = if self.peek() == &TokenKind::FatArrow {
                    self.bump();
                    let expr = self.parse_expression(0)?;
                    let start = self.arena.stmt_pool.len();
                    let ret_id = self.arena.alloc_stmt(
                        StmtNode::Return(Some(expr)),
                        r_tok.span.clone(),
                    );
                    self.arena.stmt_pool.push(ret_id);
                    let end = self.arena.stmt_pool.len();
                    SliceRange::new(start, end)
                } else if self.peek() == &TokenKind::LBrace {
                    self.parse_block()?
                } else {
                    if self.peek() == &TokenKind::Semi {
                        self.bump();
                    }
                    SliceRange::new(0, 0)
                };

                let p_start = self.arena.symbol_pool.len();
                for sym in params_vec {
                    self.arena.symbol_pool.push(sym);
                }
                let p_end = self.arena.symbol_pool.len();

                let id = self.arena.alloc_stmt(
                    StmtNode::RoutineDef {
                        name,
                        params: SliceRange::new(p_start, p_end),
                        return_type,
                        taking_ms,
                        state_constraint,
                        required_capabilities,
                        body,
                    },
                    r_tok.span,
                );
                Ok(Some(id))
    }

    pub fn parse_isolate_stmt(&mut self) -> Result<Option<StmtId>, String> {
let iso_tok = self.bump();
                let is_actor = iso_tok.kind == TokenKind::Actor;
                let name = match self.peek() {
                    TokenKind::Ident(sym) => *sym,
                    _ => causm_core::symbol::intern("anonymous"),
                };
                if matches!(self.peek(), TokenKind::Ident(_)) {
                    self.bump();
                }
                let body = if is_actor {
                    let actor_name = causm_core::symbol::resolve(name);
                    let mut stmts = Vec::new();
                    if self.peek() == &TokenKind::LBrace {
                        self.bump();
                        while self.peek() != &TokenKind::RBrace
                            && self.peek() != &TokenKind::Eof
                        {
                            if self.peek() == &TokenKind::On {
                                let on_tok = self.bump();
                                let mut pat_parts = Vec::new();
                                if let TokenKind::Ident(s) = self.peek() {
                                    pat_parts.push(causm_core::symbol::resolve(*s));
                                    self.bump();
                                    while self.peek() == &TokenKind::DoubleColon {
                                        self.bump();
                                        if let TokenKind::Ident(next_s) = self.peek()
                                        {
                                            pat_parts.push(
                                                causm_core::symbol::resolve(*next_s),
                                            );
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
                                    } else if let TokenKind::Duration(ms) =
                                        self.peek()
                                    {
                                        taking_ms = Some(*ms);
                                        self.bump();
                                    }
                                }
                                let handler_body = self.parse_block()?;
                                let full_handler_name = format!(
                                    "{}::{}",
                                    actor_name,
                                    pat_parts.join("::")
                                );
                                let handler_sym =
                                    causm_core::symbol::intern(&full_handler_name);
                                let h_id = self.arena.alloc_stmt(
                                    StmtNode::RoutineDef {
                                        name: handler_sym,
                                        params: SliceRange::new(0, 0),
                                        return_type: None,
                                        taking_ms,
                                        state_constraint: None,
                                        required_capabilities: Vec::new(),
                                        body: handler_body,
                                    },
                                    on_tok.span,
                                );
                                stmts.push(h_id);
                            } else if self.peek() == &TokenKind::Slice {
                                let s_tok = self.bump();
                                let mut duration_ms = 0u64;
                                match self.peek() {
                                    TokenKind::Duration(d) => {
                                        duration_ms = *d;
                                        self.bump();
                                    }
                                    TokenKind::Int(i) => {
                                        duration_ms = *i as u64;
                                        self.bump();
                                    }
                                    _ => {}
                                }
                                let s_id = self.arena.alloc_stmt(
                                    StmtNode::EnableResource {
                                        resource: causm_core::symbol::intern(
                                            "slice",
                                        ),
                                        amount: duration_ms,
                                        unit: Some(causm_core::symbol::intern("ms")),
                                    },
                                    s_tok.span,
                                );
                                stmts.push(s_id);
                            } else if let Some(stmt_id) = self.parse_statement()? {
                                stmts.push(stmt_id);
                            }
                        }
                        if self.peek() == &TokenKind::RBrace {
                            self.bump();
                        }
                    }
                    let start = self.arena.stmt_pool.len();
                    for sid in stmts {
                        self.arena.stmt_pool.push(sid);
                    }
                    let end = self.arena.stmt_pool.len();
                    SliceRange::new(start, end)
                } else {
                    self.parse_block()?
                };
                let id = self
                    .arena
                    .alloc_stmt(StmtNode::Isolate { name, body }, iso_tok.span);
                Ok(Some(id))
    }

    pub fn parse_state_stmt(&mut self) -> Result<Option<StmtId>, String> {
let s_tok = self.bump();
                let next = self.peek();
                if next == &TokenKind::Eq {
                    self.bump();
                    let value = self.parse_expression(0)?;
                    let id = self.arena.alloc_stmt(
                        StmtNode::Assign {
                            target: causm_core::symbol::intern("state"),
                            value,
                        },
                        s_tok.span,
                    );
                    return Ok(Some(id));
                }
                let name = match self.peek() {
                    TokenKind::Ident(s) => *s,
                    _ => causm_core::symbol::intern(""),
                };
                if matches!(self.peek(), TokenKind::Ident(_)) {
                    self.bump();
                }
                if self.peek() == &TokenKind::Colon {
                    self.bump();
                    while self.peek() != &TokenKind::Eq
                        && self.peek() != &TokenKind::Semi
                        && self.peek() != &TokenKind::Eof
                    {
                        self.bump();
                    }
                }
                if self.peek() == &TokenKind::Eq {
                    self.bump();
                }
                let value = self.parse_expression(0)?;
                let id = self
                    .arena
                    .alloc_stmt(StmtNode::State { name, value }, s_tok.span);
                Ok(Some(id))
    }

}

