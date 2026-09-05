use super::lexer::{Token, TokenKind, TokenStream};
use super::pratt::PrattParser;
use causm_core::arena::{AstArena, SliceRange, StmtId, StmtNode};

pub struct ArenaParser<'a> {
    stream: TokenStream<'a>,
    current: Token,
    pub arena: AstArena,
    pub pending_attributes: Vec<causm_core::Attribute>,
}

impl<'a> ArenaParser<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut stream = TokenStream::new(source);
        let current = stream.next_token();
        Self {
            stream,
            current,
            arena: AstArena::new(),
            pending_attributes: Vec::new(),
        }
    }

    fn bump(&mut self) -> Token {
        std::mem::replace(&mut self.current, self.stream.next_token())
    }

    pub fn parse_expression(
        &mut self,
        min_bp: u8,
    ) -> Result<causm_core::arena::ExprId, String> {
        self.parse_expression_opt(min_bp, false)
    }

    pub fn parse_expression_no_struct_lit(
        &mut self,
        min_bp: u8,
    ) -> Result<causm_core::arena::ExprId, String> {
        self.parse_expression_opt(min_bp, true)
    }

    fn parse_expression_opt(
        &mut self,
        min_bp: u8,
        disallow_struct_lit: bool,
    ) -> Result<causm_core::arena::ExprId, String> {
        let dummy_tok = Token {
            kind: TokenKind::Eof,
            span: causm_core::Span { start: 0, end: 0 },
        };
        let stream = std::mem::replace(&mut self.stream, TokenStream::new(""));
        let current = std::mem::replace(&mut self.current, dummy_tok);

        let mut pratt = PrattParser::from_stream(stream, current, &mut self.arena);
        pratt.disallow_struct_lit = disallow_struct_lit;
        let res = pratt.parse_expression(min_bp);
        let (new_stream, new_current) = pratt.into_parts();
        self.stream = new_stream;
        self.current = new_current;
        res
    }

    fn peek(&self) -> &TokenKind {
        &self.current.kind
    }

    pub fn parse_program(&mut self) -> Result<SliceRange<StmtId>, String> {
        let start = self.arena.root_statements.len();
        while self.peek() != &TokenKind::Eof {
            if let Some(stmt_id) = self.parse_statement()? {
                self.arena.root_statements.push(stmt_id);
            }
        }
        let end = self.arena.root_statements.len();
        Ok(SliceRange::new(start, end))
    }

    pub fn parse_statement(&mut self) -> Result<Option<StmtId>, String> {
        let res = self.parse_statement_inner()?;
        if let Some(id) = res {
            if !self.pending_attributes.is_empty() {
                let attrs = std::mem::take(&mut self.pending_attributes);
                self.arena.stmt_attributes.insert(id.0, attrs);
            }
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }

    fn parse_statement_inner(&mut self) -> Result<Option<StmtId>, String> {
        while self.peek() == &TokenKind::Semi {
            self.bump();
        }

        match self.peek() {
            TokenKind::Eof | TokenKind::RBrace | TokenKind::Else => Ok(None),
            TokenKind::At => {
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
                                Some(causm_core::TimeCoordinate::Relative(*ms as u64))
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
                                        coord = Some(causm_core::TimeCoordinate::Periodic(*ms));
                                        self.bump();
                                    } else if let TokenKind::Int(ms) = self.peek() {
                                        coord = Some(causm_core::TimeCoordinate::Periodic(*ms as u64));
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
            TokenKind::Let => {
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
                        if angle_depth == 0 && !t_str.is_empty() {
                            if !matches!(
                                self.peek(),
                                TokenKind::Lt | TokenKind::LBracket
                            ) {
                                break;
                            }
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
            TokenKind::Return => {
                let ret_tok = self.bump();
                let expr = if self.peek() != &TokenKind::Semi
                    && self.peek() != &TokenKind::RBrace
                    && self.peek() != &TokenKind::Eof
                {
                    Some(self.parse_expression(0)?)
                } else {
                    None
                };
                let id = self.arena.alloc_stmt(StmtNode::Return(expr), ret_tok.span);
                Ok(Some(id))
            }
            TokenKind::Yield => {
                let y_tok = self.bump();
                let expr = self.parse_expression(0)?;
                let id = self.arena.alloc_stmt(StmtNode::Yield(expr), y_tok.span);
                Ok(Some(id))
            }
            TokenKind::Pub | TokenKind::Routine => {
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
                                name_str.push_str(&causm_core::symbol::resolve(*sym));
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
                                    full_type.push_str(&causm_core::symbol::resolve(*typ_sym));
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
                                            full_type.push_str(&causm_core::symbol::resolve(*s));
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
            TokenKind::Import => {
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
            TokenKind::Require => {
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
                    while self.peek() != &TokenKind::RParen
                        && self.peek() != &TokenKind::Eof
                    {
                        if let TokenKind::Ident(k) = self.peek().clone() {
                            let k_str = causm_core::symbol::resolve(k);
                            self.bump();
                            if self.peek() == &TokenKind::Eq {
                                self.bump();
                                let v_str = match self.peek() {
                                    TokenKind::Str(s) => s.clone(),
                                    TokenKind::Ident(s) => {
                                        causm_core::symbol::resolve(*s)
                                    }
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
            TokenKind::Foreign => {
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
                    while self.peek() != &TokenKind::RBrace
                        && self.peek() != &TokenKind::Eof
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
            TokenKind::On => {
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
            TokenKind::Isolate | TokenKind::Actor => {
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
                        while self.peek() != &TokenKind::RBrace && self.peek() != &TokenKind::Eof {
                            if self.peek() == &TokenKind::On {
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
                                let handler_body = self.parse_block()?;
                                let full_handler_name = format!("{}::{}", actor_name, pat_parts.join("::"));
                                let handler_sym = causm_core::symbol::intern(&full_handler_name);
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
                                        resource: causm_core::symbol::intern("slice"),
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
            TokenKind::Enable => {
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
            TokenKind::Split => {
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
            TokenKind::Merge => {
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
                if self.peek() == &TokenKind::Taking || self.peek() == &TokenKind::For {
                    self.bump();
                    if let Some(ms) = self.parse_optional_duration_limit() {
                        taking_ms = Some(ms);
                    }
                } else if matches!(self.peek(), TokenKind::Duration(_) | TokenKind::Int(_)) {
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
                                if self.peek() == &TokenKind::Colon || self.peek() == &TokenKind::Eq {
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
            TokenKind::Send => {
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
                        if matches!(name.as_str(), "consume" | "clone" | "decay" | "peek") {
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
            TokenKind::If => {
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
                                if paren_depth > 0 {
                                    paren_depth -= 1;
                                }
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
            TokenKind::Print => {
                let p_tok = self.bump();
                let start = self.arena.expr_pool.len();
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    while self.peek() != &TokenKind::RParen
                        && self.peek() != &TokenKind::Eof
                    {
                        let arg = self.parse_expression(0)?;
                        self.arena.expr_pool.push(arg);
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
                let end = self.arena.expr_pool.len();
                let id = self.arena.alloc_stmt(
                    StmtNode::Print(SliceRange::new(start, end)),
                    p_tok.span,
                );
                Ok(Some(id))
            }
            TokenKind::Debug | TokenKind::Log => {
                let d_tok = self.bump();
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                }
                let expr = self.parse_expression(0)?;
                if self.peek() == &TokenKind::RParen {
                    self.bump();
                }
                let id = self.arena.alloc_stmt(StmtNode::Debug(expr), d_tok.span);
                Ok(Some(id))
            }
            TokenKind::OnDecay => {
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
            TokenKind::Using => {
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
            TokenKind::Break => {
                let tok = self.bump();
                Ok(Some(self.arena.alloc_stmt(StmtNode::Break, tok.span)))
            }
            TokenKind::Continue => {
                let tok = self.bump();
                Ok(Some(self.arena.alloc_stmt(StmtNode::Continue, tok.span)))
            }
            TokenKind::Collapse => {
                let tok = self.bump();
                Ok(Some(self.arena.alloc_stmt(StmtNode::Collapse, tok.span)))
            }
            TokenKind::Loop => {
                let loop_tok = self.bump();
                if self.peek() == &TokenKind::On {
                    self.bump();
                    let target = self.parse_expression(0)?;
                    let body = self.parse_block()?;
                    let id = self.arena.alloc_stmt(
                        StmtNode::LoopOn {
                            target,
                            body,
                        },
                        loop_tok.span,
                    );
                    return Ok(Some(id));
                }
                let (max_ms, step_ms, _has_step, explicit_tick) =
                    self.parse_optional_loop_modifiers();
                let body = self.parse_block()?;
                let is_tick = explicit_tick || (max_ms.is_none() && step_ms.is_none());
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
            TokenKind::While => {
                let while_tok = self.bump();
                let cond = self.parse_expression(0)?;
                let (max_ms, step_ms, _, _) = self.parse_optional_loop_modifiers();
                let body = self.parse_block()?;
                let id = self.arena.alloc_stmt(
                    StmtNode::While {
                        cond,
                        max_ms,
                        step_ms,
                        body,
                    },
                    while_tok.span,
                );
                Ok(Some(id))
            }
            TokenKind::Struct
                if self.stream.peek_token().kind == TokenKind::LBrace =>
            {
                let expr = self.parse_expression(0)?;
                let id = self
                    .arena
                    .alloc_stmt(StmtNode::Expr(expr), self.current.span.clone());
                return Ok(Some(id));
            }
            TokenKind::Interface => {
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
                        if matches!(self.peek(), TokenKind::Duration(_) | TokenKind::Int(_)) {
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
                    while self.peek() != &TokenKind::RBrace
                        && self.peek() != &TokenKind::Eof
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
                return Ok(Some(id));
            }
            TokenKind::Type | TokenKind::Struct => {
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
                    self.arena.field_assigns_pool.push(
                        causm_core::arena::FieldAssignNode {
                            field: causm_core::symbol::intern("value"),
                            expr: causm_core::arena::ExprId(0),
                            type_name: None,
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
                                let lib_name = if let TokenKind::Str(s) = self.peek()
                                {
                                    s.clone()
                                } else {
                                    String::new()
                                };
                                self.bump();
                                if self.peek() == &TokenKind::Comma {
                                    self.bump();
                                }
                                let routine_name =
                                    if let TokenKind::Str(s) = self.peek() {
                                        s.clone()
                                    } else {
                                        String::new()
                                    };
                                self.bump();
                                if self.peek() == &TokenKind::Comma {
                                    self.bump();
                                }
                                let field_name =
                                    if let TokenKind::Ident(s) = self.peek() {
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
                            let routine_name = if let TokenKind::Str(s) = self.peek()
                            {
                                s.clone()
                            } else {
                                String::new()
                            };
                            self.bump();
                            if self.peek() == &TokenKind::Comma {
                                self.bump();
                            }
                            let field_name = if let TokenKind::Ident(s) = self.peek()
                            {
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
                    while self.peek() != &TokenKind::RBrace
                        && self.peek() != &TokenKind::Eof
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
                                let lib_name = if let TokenKind::Str(s) = self.peek()
                                {
                                    s.clone()
                                } else {
                                    String::new()
                                };
                                self.bump();
                                if self.peek() == &TokenKind::Comma {
                                    self.bump();
                                }
                                let routine_name =
                                    if let TokenKind::Str(s) = self.peek() {
                                        s.clone()
                                    } else {
                                        String::new()
                                    };
                                self.bump();
                                if self.peek() == &TokenKind::Comma {
                                    self.bump();
                                }
                                let field_name =
                                    if let TokenKind::Ident(s) = self.peek() {
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
                            let routine_name = if let TokenKind::Str(s) = self.peek()
                            {
                                s.clone()
                            } else {
                                String::new()
                            };
                            self.bump();
                            if self.peek() == &TokenKind::Comma {
                                self.bump();
                            }
                            let field_name = if let TokenKind::Ident(s) = self.peek()
                            {
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
            TokenKind::Enum => {
                let enum_tok = self.bump();
                let name = match self.peek() {
                    TokenKind::Ident(sym) => *sym,
                    _ => return Err("Expected enum name".into()),
                };
                self.bump();
                let mut variants_vec = Vec::new();
                if self.peek() == &TokenKind::LBrace {
                    self.bump();
                    while self.peek() != &TokenKind::RBrace
                        && self.peek() != &TokenKind::Eof
                    {
                        let v_name = match self.peek() {
                            TokenKind::Ident(sym) => {
                                Some(causm_core::symbol::resolve(*sym))
                            }
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
                                        let t_name =
                                            causm_core::symbol::resolve(*t_sym);
                                        let typ = match t_name.as_str() {
                                            "int" => causm_core::TypeName::Builtin(
                                                causm_core::BuiltinType::Integer,
                                            ),
                                            "float" => {
                                                causm_core::TypeName::Builtin(
                                                    causm_core::BuiltinType::Float,
                                                )
                                            }
                                            "bool" => causm_core::TypeName::Builtin(
                                                causm_core::BuiltinType::Bool,
                                            ),
                                            "string" => {
                                                causm_core::TypeName::Builtin(
                                                    causm_core::BuiltinType::String,
                                                )
                                            }
                                            _ => {
                                                causm_core::TypeName::Custom(t_name)
                                            }
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
            TokenKind::Macro => {
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
                                        "literal" => {
                                            causm_core::MacroParamKind::Literal
                                        }
                                        _ => causm_core::MacroParamKind::Expr,
                                    };
                                    self.bump();
                                }
                            }
                            params
                                .push(causm_core::MacroParam { name: p_name, kind });
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
                        body_template =
                            self.stream.source[start_idx..end_idx].to_string();
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
            TokenKind::From => {
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
                                    item_name = format!("{} as {}", item_name, causm_core::symbol::resolve(*alias_s));
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
            TokenKind::Match => {
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
                                    if paren_depth > 0 {
                                        paren_depth -= 1;
                                    }
                                    pat_str.push(')');
                                }
                                TokenKind::LBrace => {
                                    brace_depth += 1;
                                    pat_str.push('{');
                                }
                                TokenKind::RBrace => {
                                    if brace_depth > 0 {
                                        brace_depth -= 1;
                                    }
                                    pat_str.push('}');
                                }
                                TokenKind::LBracket => {
                                    bracket_depth += 1;
                                    pat_str.push('[');
                                }
                                TokenKind::RBracket => {
                                    if bracket_depth > 0 {
                                        bracket_depth -= 1;
                                    }
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
            TokenKind::For => {
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
            TokenKind::Lease => {
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
            TokenKind::Anchor => {
                let a_tok = self.bump();
                let name = match self.peek() {
                    TokenKind::Ident(s) => *s,
                    _ => causm_core::symbol::intern("anchor"),
                };
                self.bump();
                let id = self.arena.alloc_stmt(StmtNode::Anchor(name), a_tok.span);
                Ok(Some(id))
            }
            TokenKind::RewindTo => {
                let r_tok = self.bump();
                let mut name = causm_core::symbol::intern("");
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    if let TokenKind::Ident(s) = self.peek() {
                        name = *s;
                        self.bump();
                    }
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                }
                let id = self.arena.alloc_stmt(StmtNode::RewindTo(name), r_tok.span);
                Ok(Some(id))
            }
            TokenKind::State => {
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
            TokenKind::Policy => {
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
            TokenKind::Select => {
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
                        while self.peek() != &TokenKind::RParen && self.peek() != &TokenKind::Eof {
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
            TokenKind::Entangle => {
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
            TokenKind::Speculate => {
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
            TokenKind::Commit => {
                let com_tok = self.bump();
                let body = self.parse_block()?;
                let id = self.arena.alloc_stmt(StmtNode::Commit(body), com_tok.span);
                Ok(Some(id))
            }
            TokenKind::Slice => {
                let s_tok = self.bump();
                let expr = self.parse_expression(0)?;
                let id = self.arena.alloc_stmt(StmtNode::Slice(expr), s_tok.span);
                Ok(Some(id))
            }
            TokenKind::Ident(sym) if sym.0 == causm_core::symbol::intern("slice").0 => {
                let s_tok = self.bump();
                let expr = self.parse_expression(0)?;
                let id = self.arena.alloc_stmt(StmtNode::Slice(expr), s_tok.span);
                Ok(Some(id))
            }
            TokenKind::AssertTime => {
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
            _ => {
                let start_span = self.current.span.clone();
                let next_tok = self.stream.peek_token();
                if let TokenKind::Ident(s) = self.peek() {
                    if causm_core::symbol::resolve(*s) == "state" && matches!(next_tok.kind, TokenKind::Ident(_)) {
                        let s_tok = self.bump();
                        let name = match self.peek() {
                            TokenKind::Ident(s) => *s,
                            _ => causm_core::symbol::intern(""),
                        };
                        self.bump();
                        if self.peek() == &TokenKind::Colon {
                            self.bump();
                            if matches!(self.peek(), TokenKind::Ident(_)) {
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
                        return Ok(Some(id));
                    }
                }
                if let TokenKind::Ident(target_sym) = self.peek() {
                    let is_assign = match next_tok.kind {
                        TokenKind::Eq
                        | TokenKind::PlusEq
                        | TokenKind::MinusEq
                        | TokenKind::StarEq
                        | TokenKind::SlashEq
                        | TokenKind::PercentEq
                        | TokenKind::ShlEq
                        | TokenKind::ShrEq
                        | TokenKind::AmpEq
                        | TokenKind::PipeEq
                        | TokenKind::CaretEq => true,
                        _ => false,
                    };

                    if is_assign {
                        let target = *target_sym;
                        self.bump(); // target
                        let op_tok = self.bump(); // compound or eq
                        let val_expr = self.parse_expression(0)?;

                        let final_expr = match op_tok.kind {
                            TokenKind::Eq => val_expr,
                            compound_op => {
                                let target_expr = self.arena.alloc_expr(
                                    causm_core::arena::ExprNode::Identifier(target),
                                    start_span.clone(),
                                );
                                let bin_op = match compound_op {
                                    TokenKind::PlusEq => {
                                        causm_core::BinaryOperator::Add
                                    }
                                    TokenKind::MinusEq => {
                                        causm_core::BinaryOperator::Sub
                                    }
                                    TokenKind::StarEq => {
                                        causm_core::BinaryOperator::Mul
                                    }
                                    TokenKind::SlashEq => {
                                        causm_core::BinaryOperator::Div
                                    }
                                    TokenKind::PercentEq => {
                                        causm_core::BinaryOperator::Rem
                                    }
                                    TokenKind::ShlEq => {
                                        causm_core::BinaryOperator::Shl
                                    }
                                    TokenKind::ShrEq => {
                                        causm_core::BinaryOperator::Shr
                                    }
                                    TokenKind::AmpEq => {
                                        causm_core::BinaryOperator::BitwiseAnd
                                    }
                                    TokenKind::PipeEq => {
                                        causm_core::BinaryOperator::BitwiseOr
                                    }
                                    TokenKind::CaretEq => {
                                        causm_core::BinaryOperator::BitwiseXor
                                    }
                                    _ => causm_core::BinaryOperator::Add,
                                };
                                self.arena.alloc_expr(
                                    causm_core::arena::ExprNode::BinaryOp {
                                        left: target_expr,
                                        right: val_expr,
                                        op: bin_op,
                                    },
                                    start_span.clone(),
                                )
                            }
                        };

                        let id = self.arena.alloc_stmt(
                            StmtNode::Assign {
                                target,
                                value: final_expr,
                            },
                            start_span,
                        );
                        return Ok(Some(id));
                    }
                }

                let expr_id = self.parse_expression(0)?;
                if self.peek() == &TokenKind::Eq {
                    self.bump();
                    let val_expr_id = self.parse_expression(0)?;
                    let id = match &self.arena.expressions[expr_id.0 as usize] {
                        causm_core::arena::ExprNode::Identifier(sym) => {
                            self.arena.alloc_stmt(
                                StmtNode::Assign {
                                    target: *sym,
                                    value: val_expr_id,
                                },
                                start_span,
                            )
                        }
                        causm_core::arena::ExprNode::FieldAccess { target, field } => {
                            self.arena.alloc_stmt(
                                StmtNode::FieldUpdate {
                                    target: *target,
                                    field: *field,
                                    value: val_expr_id,
                                },
                                start_span,
                            )
                        }
                        causm_core::arena::ExprNode::IndexAccess { .. } => {
                            self.arena.alloc_stmt(
                                StmtNode::FieldUpdate {
                                    target: expr_id,
                                    field: causm_core::symbol::intern(""),
                                    value: val_expr_id,
                                },
                                start_span,
                            )
                        }
                        _ => {
                            self.arena.alloc_stmt(StmtNode::Expr(val_expr_id), start_span)
                        }
                    };
                    return Ok(Some(id));
                }
                let id = self.arena.alloc_stmt(StmtNode::Expr(expr_id), start_span);
                Ok(Some(id))
            }
        }
    }

    pub fn parse_resolution_strategy(&mut self) -> Result<causm_core::ResolutionStrategy, String> {
        let is_topology = match self.peek() {
            TokenKind::Ident(s) => {
                let name = causm_core::symbol::resolve(*s);
                name == "topology_union" || name == "topology_intersect"
            }
            _ => false,
        };

        if is_topology {
            let is_union = match self.peek() {
                TokenKind::Ident(s) => causm_core::symbol::resolve(*s) == "topology_union",
                _ => true,
            };
            self.bump(); // topology_union or topology_intersect
            if self.peek() == &TokenKind::LBrace {
                self.bump();
            }
            let mut rules = std::collections::HashMap::new();
            let mut default = Box::new(causm_core::ResolutionStrategy::Decay);
            let mut on_invalid = None;

            while self.peek() != &TokenKind::RBrace && self.peek() != &TokenKind::Eof {
                if matches!(self.peek(), TokenKind::Ident(s) if causm_core::symbol::resolve(*s) == "on_invalid") {
                    self.bump();
                    if self.peek() == &TokenKind::Colon {
                        self.bump();
                    }
                    if matches!(self.peek(), TokenKind::Ident(s) if causm_core::symbol::resolve(*s) == "rewind") {
                        self.bump();
                    }
                    let branch = match self.peek() {
                        TokenKind::Ident(s) => causm_core::symbol::resolve(*s),
                        _ => String::new(),
                    };
                    if matches!(self.peek(), TokenKind::Ident(_)) {
                        self.bump();
                    }
                    if self.peek() == &TokenKind::To {
                        self.bump();
                    }
                    let anchor = match self.peek() {
                        TokenKind::Ident(s) => causm_core::symbol::resolve(*s),
                        _ => String::new(),
                    };
                    if matches!(self.peek(), TokenKind::Ident(_)) {
                        self.bump();
                    }
                    on_invalid = Some(causm_core::CausalReversion { branch, anchor });
                    if self.peek() == &TokenKind::Comma {
                        self.bump();
                    }
                    continue;
                }

                let mut key_opt = None;
                if let TokenKind::Ident(k_sym) = self.peek() {
                    key_opt = Some(causm_core::symbol::resolve(*k_sym));
                    self.bump();
                } else if let TokenKind::Str(s) = self.peek() {
                    key_opt = Some(s.clone());
                    self.bump();
                }

                if let Some(key) = key_opt {
                    if self.peek() == &TokenKind::Colon || self.peek() == &TokenKind::Eq {
                        self.bump();
                        let strat = self.parse_resolution_strategy()?;
                        if key == "_" {
                            default = Box::new(strat);
                        } else {
                            rules.insert(key, strat);
                        }
                    }
                } else {
                    self.bump();
                }

                if self.peek() == &TokenKind::Comma {
                    self.bump();
                }
            }

            if self.peek() == &TokenKind::RBrace {
                self.bump();
            }

            return if is_union {
                Ok(causm_core::ResolutionStrategy::TopologyUnion {
                    key_rules: rules,
                    default,
                    on_invalid,
                })
            } else {
                Ok(causm_core::ResolutionStrategy::TopologyIntersect {
                    key_rules: rules,
                    default,
                    on_invalid,
                })
            };
        }

        match self.peek() {
            TokenKind::Ident(s) => {
                let name = causm_core::symbol::resolve(*s);
                if name == "priority" {
                    self.bump();
                    let mut branch_name = String::new();
                    if self.peek() == &TokenKind::LParen {
                        self.bump();
                        if let TokenKind::Ident(b_sym) = self.peek() {
                            branch_name = causm_core::symbol::resolve(*b_sym);
                            self.bump();
                        }
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                    }
                    Ok(causm_core::ResolutionStrategy::Priority(branch_name))
                } else if name == "first_wins" {
                    self.bump();
                    Ok(causm_core::ResolutionStrategy::FirstWins)
                } else if name == "decay" {
                    self.bump();
                    Ok(causm_core::ResolutionStrategy::Decay)
                } else if name == "auto" {
                    self.bump();
                    Ok(causm_core::ResolutionStrategy::Auto)
                } else {
                    self.bump();
                    Ok(causm_core::ResolutionStrategy::Priority(name))
                }
            }
            TokenKind::Auto => {
                self.bump();
                Ok(causm_core::ResolutionStrategy::Auto)
            }
            _ => {
                let tok = self.bump();
                Ok(causm_core::ResolutionStrategy::Custom(format!("{:?}", tok.kind)))
            }
        }
    }

    pub fn parse_optional_duration_limit(&mut self) -> Option<u64> {
        let has_paren = if self.peek() == &TokenKind::LParen {
            self.bump();
            true
        } else {
            false
        };

        if self.peek() == &TokenKind::Max
            || self.peek() == &TokenKind::Taking
            || self.peek() == &TokenKind::For
        {
            self.bump();
        }

        let mut duration_ms = None;
        match self.peek() {
            TokenKind::Duration(ms) => {
                duration_ms = Some(*ms);
                self.bump();
            }
            TokenKind::Int(ms) => {
                duration_ms = Some(*ms as u64);
                self.bump();
            }
            TokenKind::Ident(s)
                if causm_core::symbol::resolve(*s) == "_"
                    || causm_core::symbol::resolve(*s) == "?" =>
            {
                duration_ms = Some(u64::MAX);
                self.bump();
            }
            _ => {}
        }

        if has_paren && self.peek() == &TokenKind::RParen {
            self.bump();
        }

        duration_ms
    }

    pub fn parse_optional_loop_modifiers(
        &mut self,
    ) -> (Option<u64>, Option<u64>, bool, bool) {
        let mut max_ms = None;
        let mut step_ms = None;
        let mut has_step_or_pacing = false;
        let mut is_tick = false;

        loop {
            if self.peek() == &TokenKind::Tick {
                self.bump();
                is_tick = true;
                step_ms = Some(1);
            } else if self.peek() == &TokenKind::Max
                || self.peek() == &TokenKind::Taking
            {
                max_ms = self.parse_optional_duration_limit();
            } else if self.peek() == &TokenKind::Step
                || matches!(self.peek(), TokenKind::Ident(s) if causm_core::symbol::resolve(*s) == "pacing")
            {
                self.bump();
                has_step_or_pacing = true;
                let dur = self.parse_optional_duration_limit();
                step_ms = if dur == Some(u64::MAX) { None } else { dur };
            } else if self.peek() == &TokenKind::LParen {
                let next_k = self.stream.peek_token().kind;
                if next_k == TokenKind::Max || next_k == TokenKind::Taking {
                    max_ms = self.parse_optional_duration_limit();
                } else if next_k == TokenKind::Step
                    || matches!(&next_k, TokenKind::Ident(s) if causm_core::symbol::resolve(*s) == "pacing")
                {
                    self.bump();
                    has_step_or_pacing = true;
                    let dur = self.parse_optional_duration_limit();
                    step_ms = if dur == Some(u64::MAX) { None } else { dur };
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        (max_ms, step_ms, has_step_or_pacing, is_tick)
    }

    pub fn parse_block(&mut self) -> Result<SliceRange<StmtId>, String> {
        if self.peek() == &TokenKind::LBrace {
            self.bump();
            let mut stmts = Vec::new();
            while self.peek() != &TokenKind::RBrace && self.peek() != &TokenKind::Eof
            {
                if let Some(stmt_id) = self.parse_statement()? {
                    stmts.push(stmt_id);
                }
            }
            if self.peek() == &TokenKind::RBrace {
                self.bump();
            }
            let start = self.arena.stmt_pool.len();
            for sid in stmts {
                self.arena.stmt_pool.push(sid);
            }
            let end = self.arena.stmt_pool.len();
            Ok(SliceRange::new(start, end))
        } else {
            let mut stmts = Vec::new();
            if let Some(stmt_id) = self.parse_statement()? {
                stmts.push(stmt_id);
            }
            let start = self.arena.stmt_pool.len();
            for sid in stmts {
                self.arena.stmt_pool.push(sid);
            }
            let end = self.arena.stmt_pool.len();
            Ok(SliceRange::new(start, end))
        }
    }
}

pub fn parse_type_name_str(s: &str) -> causm_core::TypeName {
    let s = s.trim();
    if let Some((base, rest)) = s.split_once('<') {
        let inner = rest.trim_end_matches('>').trim();
        let mut params = Vec::new();
        for part in inner.split(',') {
            let part = part.trim();
            if let Some(dur_str) = part.strip_suffix("ms") {
                if let Ok(n) = dur_str.trim().parse::<u64>() {
                    params.push(causm_core::TypeParam::Duration(n));
                    continue;
                }
            }
            if let Ok(n) = part.parse::<u64>() {
                params.push(causm_core::TypeParam::Amount(n));
            } else if part.contains('<') {
                params.push(causm_core::TypeParam::Type(parse_type_name_str(part)));
            } else {
                params.push(causm_core::TypeParam::Type(
                    causm_core::TypeName::from_str_name(part),
                ));
            }
        }
        causm_core::TypeName::Generic(base.to_string(), params)
    } else {
        match s {
            "int" => causm_core::TypeName::Builtin(causm_core::BuiltinType::Integer),
            "float" => causm_core::TypeName::Builtin(causm_core::BuiltinType::Float),
            "bool" => causm_core::TypeName::Builtin(causm_core::BuiltinType::Bool),
            "string" => causm_core::TypeName::Builtin(causm_core::BuiltinType::String),
            "u8" => causm_core::TypeName::Builtin(causm_core::BuiltinType::U8),
            "u16" => causm_core::TypeName::Builtin(causm_core::BuiltinType::U16),
            "u32" => causm_core::TypeName::Builtin(causm_core::BuiltinType::U32),
            "u64" => causm_core::TypeName::Builtin(causm_core::BuiltinType::U64),
            "i8" => causm_core::TypeName::Builtin(causm_core::BuiltinType::I8),
            "i16" => causm_core::TypeName::Builtin(causm_core::BuiltinType::I16),
            "i32" => causm_core::TypeName::Builtin(causm_core::BuiltinType::I32),
            "i64" => causm_core::TypeName::Builtin(causm_core::BuiltinType::I64),
            "f32" => causm_core::TypeName::Builtin(causm_core::BuiltinType::F32),
            "f64" => causm_core::TypeName::Builtin(causm_core::BuiltinType::F64),
            "struct" => causm_core::TypeName::Builtin(causm_core::BuiltinType::Struct),
            "array" => causm_core::TypeName::Builtin(causm_core::BuiltinType::Array),
            _ => causm_core::TypeName::Custom(s.to_string()),
        }
    }
}

pub fn to_ast_statement(
    arena: &AstArena,
    id: StmtId,
) -> causm_core::SpannedStatement {
    let span = arena
        .stmt_spans
        .get(id.0 as usize)
        .cloned()
        .unwrap_or(causm_core::Span { start: 0, end: 0 });
    let stmt = match &arena.statements[id.0 as usize] {
        StmtNode::Expr(eid) => {
            let expr = super::pratt::to_ast_expression(arena, *eid);
            match expr {
                causm_core::Expression::Call { routine, mut args } if routine == "await" && !args.is_empty() => {
                    let target_name = match args.remove(0) {
                        causm_core::Expression::Identifier(name) => name,
                        other => format!("{:?}", other),
                    };
                    causm_core::Statement::Await(target_name)
                }
                other => causm_core::Statement::Expression(other),
            }
        }
        StmtNode::Let {
            target,
            is_mut,
            type_annotation,
            init,
            lifetime,
        } => {
            let expr = init
                .map(|eid| super::pratt::to_ast_expression(arena, eid))
                .unwrap_or(causm_core::Expression::Null);
            let var_type = type_annotation.map(|sym| {
                let s = causm_core::symbol::resolve(sym);
                parse_type_name_str(&s)
            });
            causm_core::Statement::Assignment {
                target: causm_core::symbol::resolve(*target),
                mutable: *is_mut,
                lifetime: lifetime.clone(),
                var_type,
                expr,
            }
        }
        StmtNode::Destructure { fields, expr } => {
            let mut pairs = Vec::new();
            let slice = &arena.symbol_pool[fields.as_range()];
            let mut i = 0;
            while i + 1 < slice.len() {
                let f_str = causm_core::symbol::resolve(slice[i]);
                let t_str = causm_core::symbol::resolve(slice[i + 1]);
                pairs.push((f_str, t_str));
                i += 2;
            }
            causm_core::Statement::DestructureAssignment {
                fields: pairs,
                expr: super::pratt::to_ast_expression(arena, *expr),
                mutable: false,
            }
        }
        StmtNode::Assign { target, value } => {
            let expr = super::pratt::to_ast_expression(arena, *value);
            if let causm_core::Expression::Match { target: ref match_tgt, ref arms } = expr {
                if let causm_core::Expression::Call { ref routine, ref args } = **match_tgt {
                    if routine == "entropy" && !args.is_empty() {
                        let lhs_name = causm_core::symbol::resolve(*target);
                        let mut valid_branch = None;
                        let mut decayed_branch = None;
                        let mut pending_branch = None;
                        let mut consumed_branch = None;
                        for arm in arms {
                            let (pat_name, binding) = match &arm.pattern {
                                causm_core::Pattern::EnumVariant { variant_name, args, .. } => {
                                    let b = if let Some(causm_core::Pattern::Identifier(id)) = args.first() {
                                        id.clone()
                                    } else {
                                        String::new()
                                    };
                                    (variant_name.as_str(), b)
                                }
                                causm_core::Pattern::Identifier(id) => (id.as_str(), String::new()),
                                _ => ("", String::new()),
                            };
                            let body_stmts = vec![causm_core::SpannedStatement::new(
                                causm_core::Statement::Assignment {
                                    target: lhs_name.clone(),
                                    mutable: false,
                                    var_type: None,
                                    lifetime: None,
                                    expr: arm.body.clone(),
                                },
                                span.clone(),
                            )];
                            match pat_name {
                                "Valid" => {
                                    valid_branch = Some((
                                        causm_core::DecayedPattern::Binding(binding),
                                        arm.guard.clone(),
                                        body_stmts,
                                    ));
                                }
                                "Decayed" => {
                                    decayed_branch = Some((
                                        causm_core::DecayedPattern::Binding(binding),
                                        arm.guard.clone(),
                                        body_stmts,
                                    ));
                                }
                                "Pending" => {
                                    pending_branch = Some((
                                        causm_core::DecayedPattern::Binding(binding),
                                        arm.guard.clone(),
                                        body_stmts,
                                    ));
                                }
                                "Consumed" => {
                                    consumed_branch = Some((arm.guard.clone(), body_stmts));
                                }
                                _ => {}
                            }
                        }
                        return causm_core::SpannedStatement::with_attributes(
                            causm_core::Statement::MatchEntropy {
                                target: args[0].clone(),
                                valid_branch,
                                decayed_branch,
                                pending_branch,
                                consumed_branch,
                            },
                            span,
                            arena.stmt_attributes.get(&id.0).cloned().unwrap_or_default(),
                        );
                    }
                }
            }
            causm_core::Statement::Assignment {
                target: causm_core::symbol::resolve(*target),
                mutable: true,
                lifetime: None,
                var_type: None,
                expr,
            }
        }
        StmtNode::FieldUpdate { target, field, value } => {
            let target_expr = super::pratt::to_ast_expression(arena, *target);
            let val_expr = super::pratt::to_ast_expression(arena, *value);
            let field_str = causm_core::symbol::resolve(*field);
            causm_core::Statement::FieldUpdate {
                target: target_expr,
                field: field_str,
                value: val_expr,
            }
        }
        StmtNode::Return(val) => {
            let expr = val.map(|eid| super::pratt::to_ast_expression(arena, eid));
            causm_core::Statement::Return(expr)
        }
        StmtNode::Yield(eid) => causm_core::Statement::Yield(Some(
            super::pratt::to_ast_expression(arena, *eid),
        )),
        StmtNode::RoutineDef {
            name,
            params,
            return_type,
            taking_ms,
            state_constraint,
            required_capabilities,
            body,
        } => {
            let r_name = causm_core::symbol::resolve(*name);
            let receiver_type = if let Some((type_name, _)) = r_name.split_once('.')
            {
                Some(causm_core::TypeName::Custom(type_name.to_string()))
            } else {
                None
            };
            let mut param_decls = Vec::new();
            let p_slice = &arena.symbol_pool[params.as_range()];
            for chunk in p_slice.chunks(3) {
                if chunk.len() == 3 {
                    let mode_str = causm_core::symbol::resolve(chunk[0]);
                    let mode = match mode_str.as_str() {
                        "consume" => causm_core::ParamMode::Consume,
                        "clone" => causm_core::ParamMode::Clone,
                        "decay" => causm_core::ParamMode::Decay,
                        "lease" => causm_core::ParamMode::Lease,
                        _ => causm_core::ParamMode::Peek,
                    };
                    let type_str = causm_core::symbol::resolve(chunk[2]);
                    let typ = parse_type_name_str(&type_str);
                    let param_name = causm_core::symbol::resolve(chunk[1]);
                    let final_typ = if param_name == "self" && type_str.is_empty() {
                        receiver_type.clone()
                    } else if type_str.is_empty() {
                        None
                    } else {
                        Some(typ)
                    };
                    param_decls.push(causm_core::ParamDecl {
                        mode,
                        name: param_name,
                        typ: final_typ,
                    });
                }
            }
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let ret_type = return_type.map(|rt| {
                causm_core::TypeName::from_str_name(&causm_core::symbol::resolve(rt))
            });
            let sc = state_constraint.map(|(v, s)| {
                (causm_core::symbol::resolve(v), causm_core::symbol::resolve(s))
            });
            causm_core::Statement::RoutineDef {
                name: causm_core::symbol::resolve(*name),
                params: param_decls,
                return_type: ret_type,
                taking_ms: *taking_ms,
                state_constraint: sc,
                required_capabilities: required_capabilities
                    .iter()
                    .map(|&c| causm_core::Capability {
                        path: causm_core::symbol::resolve(c),
                        parameters: std::collections::HashMap::new(),
                    })
                    .collect(),
                body: body_stmts,
            }
        }
        StmtNode::Isolate { name, body } => {
            let mut manifest = causm_core::Manifest::default();
            let mut body_stmts = Vec::new();
            let mut in_manifest = true;
            for &sid in &arena.stmt_pool[body.as_range()] {
                let stmt_node = &arena.statements[sid.0 as usize];
                if in_manifest {
                    match stmt_node {
                        StmtNode::Capability(cap) => {
                            manifest.capabilities.push(cap.clone());
                            continue;
                        }
                        StmtNode::EnableResource {
                            resource,
                            amount,
                            unit,
                        } => {
                            let r_name = causm_core::symbol::resolve(*resource);
                            let u_str = unit.map(|u| causm_core::symbol::resolve(u));
                            match r_name.as_str() {
                                "slice" => {
                                    manifest.slice_ms = Some(*amount);
                                }
                                "cpu" => {
                                    manifest.cpu_budget_ms = Some(*amount);
                                }
                                "memory" => {
                                    let mult = match u_str.as_deref() {
                                        Some("KB") => 1024,
                                        Some("MB") => 1024 * 1024,
                                        Some("GB") => 1024 * 1024 * 1024,
                                        _ => 1,
                                    };
                                    manifest.memory_budget_bytes =
                                        Some(*amount * mult);
                                }
                                _ => {
                                    manifest
                                        .resource_budgets
                                        .insert(r_name, *amount);
                                }
                            }
                            continue;
                        }
                        _ => {
                            in_manifest = false;
                        }
                    }
                }
                body_stmts.push(to_ast_statement(arena, sid));
            }
            causm_core::Statement::Isolate(causm_core::IsolateBlock {
                name: Some(causm_core::symbol::resolve(*name)),
                manifest,
                body: body_stmts,
            })
        }
        StmtNode::Capability(cap) => causm_core::Statement::Capability(cap.clone()),
        StmtNode::TimelineBlock {
            coord,
            directives,
            body,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            if !directives.is_empty() {
                causm_core::Statement::DirectiveBlock {
                    directives: directives.clone(),
                    body: body_stmts,
                }
            } else {
                causm_core::Statement::RelativisticBlock {
                    time: coord.clone(),
                    body: body_stmts,
                }
            }
        }
        StmtNode::Send { target, payload: _ } => causm_core::Statement::Send {
            value_id: "__send_value".to_string(),
            target_branch: causm_core::symbol::resolve(*target),
        },
        StmtNode::Split { parent, branches } => {
            let b_list = arena.symbol_pool[branches.as_range()]
                .iter()
                .map(|&b| causm_core::symbol::resolve(b))
                .collect();
            causm_core::Statement::Split {
                parent: causm_core::symbol::resolve(*parent),
                branches: b_list,
            }
        }
        StmtNode::Merge {
            branches,
            target,
            resolutions,
        } => {
            let b_list = arena.symbol_pool[branches.as_range()]
                .iter()
                .map(|&b| causm_core::symbol::resolve(b))
                .collect();
            let fallback = resolutions.fallback.map(|fb| {
                arena.stmt_pool[fb.as_range()]
                    .iter()
                    .map(|&s| to_ast_statement(arena, s))
                    .collect()
            });
            causm_core::Statement::Merge {
                branches: b_list,
                target: causm_core::symbol::resolve(*target),
                resolutions: causm_core::MergeResolution {
                    rules: resolutions.rules.clone(),
                    auto: resolutions.auto,
                    fallback,
                    taking_ms: resolutions.taking_ms,
                },
            }
        }
        StmtNode::Import { path, alias } => causm_core::Statement::Import {
            path: causm_core::symbol::resolve(*path),
            alias: alias.map(causm_core::symbol::resolve),
        },
        StmtNode::FromImport { path, symbols } => {
            let sym_list = arena.symbol_pool[symbols.as_range()]
                .iter()
                .map(|&s| {
                    let s_str = causm_core::symbol::resolve(s);
                    if let Some((name, alias)) = s_str.split_once(" as ") {
                        (name.trim().to_string(), Some(alias.trim().to_string()))
                    } else {
                        (s_str, None)
                    }
                })
                .collect();
            causm_core::Statement::FromImport {
                path: causm_core::symbol::resolve(*path),
                symbols: sym_list,
            }
        }
        StmtNode::TypeDecl {
            name,
            extends,
            fields,
            decay_after_ms,
            auto_drop,
        } => {
            let mut field_map = std::collections::HashMap::new();
            for f in &arena.field_assigns_pool[fields.as_range()] {
                let default_val = if f.expr.0 != u32::MAX {
                    Some(super::pratt::to_ast_expression(arena, f.expr))
                } else {
                    None
                };
                let type_str = f
                    .type_name
                    .map(causm_core::symbol::resolve)
                    .unwrap_or_else(|| "any".into());
                let typ = causm_core::TypeName::from_str_name(&type_str);
                field_map.insert(
                    causm_core::symbol::resolve(f.field),
                    causm_core::TypeFieldDef {
                        typ,
                        is_const: f.is_const,
                        default_value: default_val,
                    },
                );
            }
            causm_core::Statement::TypeDecl {
                name: causm_core::symbol::resolve(*name),
                extends: extends.map(causm_core::symbol::resolve),
                fields: field_map,
                decay_after_ms: *decay_after_ms,
                auto_drop: auto_drop.clone(),
                scoped_branch: None,
            }
        }
        StmtNode::InterfaceDecl {
            name,
            extends,
            methods,
        } => {
            let extends_vec: Vec<String> = arena.symbol_pool[extends.as_range()]
                .iter()
                .map(|&s| causm_core::symbol::resolve(s))
                .collect();
            let mut iface_methods = Vec::new();
            for &mid in &arena.stmt_pool[methods.as_range()] {
                let spanned_stmt = to_ast_statement(arena, mid);
                if let causm_core::Statement::RoutineDef {
                    name,
                    params,
                    return_type,
                    taking_ms,
                    state_constraint,
                    required_capabilities,
                    body,
                } = spanned_stmt.stmt
                {
                    let default_body =
                        if body.is_empty() { None } else { Some(body) };
                    iface_methods.push(causm_core::InterfaceMethod {
                        name,
                        params,
                        return_type,
                        taking_ms,
                        default_body,
                        state_constraint,
                        required_capabilities,
                    });
                }
            }
            causm_core::Statement::InterfaceDecl {
                name: causm_core::symbol::resolve(*name),
                extends: extends_vec,
                methods: iface_methods,
            }
        }
        StmtNode::EnumDecl { name, variants } => causm_core::Statement::EnumDecl {
            name: causm_core::symbol::resolve(*name),
            variants: variants.clone(),
        },
        StmtNode::MacroDef {
            name,
            params,
            body_template,
        } => causm_core::Statement::MacroDef {
            name: causm_core::symbol::resolve(*name),
            params: params.clone(),
            body_template: body_template.clone(),
        },
        StmtNode::If {
            cond,
            then_branch,
            else_branch,
            reconcile_auto,
        } => {
            let then_stmts = arena.stmt_pool[then_branch.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let else_stmts = else_branch.map(|eb| {
                arena.stmt_pool[eb.as_range()]
                    .iter()
                    .map(|&sid| to_ast_statement(arena, sid))
                    .collect()
            });
            let reconcile = if *reconcile_auto {
                Some(causm_core::MergeResolution {
                    rules: std::collections::HashMap::new(),
                    auto: true,
                    fallback: None,
                    taking_ms: None,
                })
            } else {
                None
            };
            causm_core::Statement::If {
                binding: None,
                condition: super::pratt::to_ast_expression(arena, *cond),
                then_branch: then_stmts,
                else_branch: else_stmts,
                reconcile,
            }
        }
        StmtNode::IfLet {
            pattern,
            expr,
            then_branch,
            else_branch,
            reconcile_auto,
        } => {
            let pat_str = causm_core::symbol::resolve(*pattern);
            let then_stmts = arena.stmt_pool[then_branch.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let else_stmts = else_branch.map(|eb| {
                arena.stmt_pool[eb.as_range()]
                    .iter()
                    .map(|&sid| to_ast_statement(arena, sid))
                    .collect()
            });
            let reconcile = if *reconcile_auto {
                Some(causm_core::MergeResolution {
                    rules: std::collections::HashMap::new(),
                    auto: true,
                    fallback: None,
                    taking_ms: None,
                })
            } else {
                None
            };
            let ast_expr = super::pratt::to_ast_expression(arena, *expr);
            if let causm_core::Expression::TypeAssertion { .. } = &ast_expr {
                let pat = super::pratt::parse_pattern_from_str(&pat_str);
                if let causm_core::Pattern::Identifier(binding_id) = pat {
                    causm_core::Statement::If {
                        condition: ast_expr,
                        binding: Some(binding_id),
                        then_branch: then_stmts,
                        else_branch: else_stmts,
                        reconcile,
                    }
                } else {
                    causm_core::Statement::IfLet {
                        pattern: super::pratt::parse_pattern_from_str(&pat_str),
                        expr: ast_expr,
                        then_branch: then_stmts,
                        else_branch: else_stmts,
                        reconcile,
                    }
                }
            } else {
                causm_core::Statement::IfLet {
                    pattern: super::pratt::parse_pattern_from_str(&pat_str),
                    expr: ast_expr,
                    then_branch: then_stmts,
                    else_branch: else_stmts,
                    reconcile,
                }
            }
        }
        StmtNode::Loop {
            max_ms,
            step_ms,
            is_tick,
            body,
        } => {
            let mut body_stmts: Vec<_> = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            if *is_tick {
                causm_core::Statement::LoopTick { body: body_stmts }
            } else {
                if let Some(step) = step_ms {
                    body_stmts.push(causm_core::SpannedStatement {
                        stmt: causm_core::Statement::Slice {
                            milliseconds: *step,
                        },
                        span: span.clone(),
                        attributes: Vec::new(),
                    });
                }
                causm_core::Statement::Loop {
                    max_ms: max_ms.unwrap_or(0),
                    body: body_stmts,
                }
            }
        }
        StmtNode::LoopOn { target, body } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::LoopOn {
                target: super::pratt::to_ast_expression(arena, *target),
                body: body_stmts,
            }
        }
        StmtNode::While {
            cond,
            max_ms,
            step_ms,
            body,
        } => {
            let mut body_stmts: Vec<_> = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            if let Some(step) = step_ms {
                body_stmts.push(causm_core::SpannedStatement {
                    stmt: causm_core::Statement::Slice {
                        milliseconds: *step,
                    },
                    span: span.clone(),
                    attributes: Vec::new(),
                });
            }
            let raw_cond = super::pratt::to_ast_expression(arena, *cond);
            let (cond_expr, is_valid_check) = match raw_cond {
                causm_core::Expression::Call { ref routine, ref args }
                    if routine == "valid" && args.len() == 1 =>
                {
                    (args[0].clone(), true)
                }
                other => (other, false),
            };
            causm_core::Statement::While {
                condition: cond_expr,
                is_valid_check,
                max_ms: max_ms.unwrap_or(0),
                body: body_stmts,
            }
        }
        StmtNode::Using {
            binding,
            resource,
            body,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::Using {
                binding: causm_core::symbol::resolve(*binding),
                resource: super::pratt::to_ast_expression(arena, *resource),
                body: body_stmts,
            }
        }
        StmtNode::Print(args) => {
            let arg_exprs = arena.expr_pool[args.as_range()]
                .iter()
                .map(|&eid| super::pratt::to_ast_expression(arena, eid))
                .collect();
            causm_core::Statement::Print(arg_exprs)
        }
        StmtNode::Break => causm_core::Statement::Break,
        StmtNode::Continue => {
            causm_core::Statement::Expression(causm_core::Expression::Null)
        }
        StmtNode::Collapse => causm_core::Statement::Collapse,
        StmtNode::Slice(eid) => {
            let ms = match &arena.expressions[eid.0 as usize] {
                causm_core::arena::ExprNode::Literal(
                    causm_core::arena::LiteralKind::Duration(d),
                ) => *d,
                causm_core::arena::ExprNode::Literal(
                    causm_core::arena::LiteralKind::Integer(i),
                ) => *i as u64,
                _ => 0,
            };
            causm_core::Statement::Slice { milliseconds: ms }
        }
        StmtNode::AssertTime {
            operator,
            limit_ms,
            fallback,
        } => {
            let fb = fallback.as_ref().map(|b| {
                arena.stmt_pool[b.as_range()]
                    .iter()
                    .map(|&sid| to_ast_statement(arena, sid))
                    .collect()
            });
            causm_core::Statement::AssertTime {
                operator: *operator,
                limit_ms: *limit_ms,
                fallback: fb,
            }
        }
        StmtNode::Speculate {
            max_ms,
            body,
            fallback,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let fb = fallback.as_ref().map(|b| {
                arena.stmt_pool[b.as_range()]
                    .iter()
                    .map(|&sid| to_ast_statement(arena, sid))
                    .collect()
            });
            causm_core::Statement::Speculate {
                max_ms: *max_ms,
                body: body_stmts,
                fallback: fb,
            }
        }
        StmtNode::Commit(body) => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::Commit(body_stmts)
        }
        StmtNode::ForeignBlock {
            lib_name,
            abi,
            routines,
        } => {
            let r_stmts = arena.stmt_pool[routines.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::ForeignBlock {
                lib_name: causm_core::symbol::resolve(*lib_name),
                abi: causm_core::symbol::resolve(*abi),
                routines: r_stmts,
            }
        }
        StmtNode::For {
            var_name,
            mode,
            iter_expr,
            pacing_ms,
            max_ms,
            body,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::For {
                item_name: causm_core::symbol::resolve(*var_name),
                mode: mode.clone(),
                source: match &arena.expressions[iter_expr.0 as usize] {
                    causm_core::arena::ExprNode::Identifier(s) => {
                        causm_core::symbol::resolve(*s)
                    }
                    _ => "__iter".into(),
                },
                pacing_ms: *pacing_ms,
                max_ms: *max_ms,
                body: body_stmts,
            }
        }
        StmtNode::Entangle(range) => {
            let vars = arena.symbol_pool[range.as_range()]
                .iter()
                .map(|sym| causm_core::symbol::resolve(*sym))
                .collect();
            causm_core::Statement::Entangle { variables: vars }
        }
        StmtNode::Debug(eid) => causm_core::Statement::Debug(
            super::pratt::to_ast_expression(arena, *eid),
        ),
        StmtNode::DecayHandler { type_name, body } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::DecayHandler {
                type_name: causm_core::symbol::resolve(*type_name),
                body: body_stmts,
            }
        }
        StmtNode::ForStep {
            var_name,
            start_expr,
            end_expr,
            step_ms,
            body,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let source_expr = if start_expr.0 == end_expr.0 {
                super::pratt::to_ast_expression(arena, *start_expr)
            } else {
                match (
                    &arena.expressions[start_expr.0 as usize],
                    &arena.expressions[end_expr.0 as usize],
                ) {
                (
                    causm_core::arena::ExprNode::Literal(
                        causm_core::arena::LiteralKind::Integer(s),
                    ),
                    causm_core::arena::ExprNode::Literal(
                        causm_core::arena::LiteralKind::Integer(e),
                    ),
                ) => {
                    let elems: Vec<causm_core::Expression> =
                        (*s..*e).map(causm_core::Expression::Integer).collect();
                    causm_core::Expression::ArrayLiteral(elems)
                }
                (
                    causm_core::arena::ExprNode::Literal(
                        causm_core::arena::LiteralKind::Duration(s),
                    ),
                    causm_core::arena::ExprNode::Literal(
                        causm_core::arena::LiteralKind::Duration(e),
                    ),
                ) => {
                    let elems: Vec<causm_core::Expression> = (*s..*e)
                        .map(|v| causm_core::Expression::Integer(v as i64))
                        .collect();
                    causm_core::Expression::ArrayLiteral(elems)
                }
                _ => super::pratt::to_ast_expression(arena, *start_expr),
                }
            };
            let final_step_ms = if *step_ms == 0 || *step_ms == u64::MAX {
                None
            } else {
                Some(*step_ms)
            };
            causm_core::Statement::ForStep {
                item_name: causm_core::symbol::resolve(*var_name),
                source: source_expr,
                step_ms: final_step_ms,
                body: body_stmts,
            }
        }
        StmtNode::Lease {
            binding,
            source,
            duration_ms,
            body,
            reconcile_auto,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let reconcile = if *reconcile_auto {
                Some(causm_core::MergeResolution {
                    rules: std::collections::HashMap::new(),
                    auto: true,
                    fallback: None,
                    taking_ms: None,
                })
            } else {
                None
            };
            causm_core::Statement::Lease {
                binding: causm_core::symbol::resolve(*binding),
                source: causm_core::symbol::resolve(*source),
                duration_ms: *duration_ms,
                body: body_stmts,
                reconcile,
            }
        }
        StmtNode::Anchor(name) => {
            causm_core::Statement::Anchor(causm_core::symbol::resolve(*name))
        }
        StmtNode::RewindTo(name) => {
            causm_core::Statement::Rewind(causm_core::symbol::resolve(*name))
        }
        StmtNode::State { name, value } => causm_core::Statement::StateDecl {
            target: causm_core::symbol::resolve(*name),
            var_type: None,
            expr: super::pratt::to_ast_expression(arena, *value),
        },
        StmtNode::Policy { target, kind } => {
            let t_str = causm_core::symbol::resolve(*target);
            let k_str = causm_core::symbol::resolve(*kind);
            let target_enum = match t_str.as_str() {
                "on_deadline_breach" => causm_core::PolicyTarget::OnDeadlineBreach,
                "on_overflow" => causm_core::PolicyTarget::OnOverflow,
                _ => causm_core::PolicyTarget::OnFull,
            };
            let kind_enum = match k_str.as_str() {
                "RingBuffer" => causm_core::SaturationPolicy::RingBuffer,
                "Throttle" => causm_core::SaturationPolicy::Throttle,
                "FailFast" => causm_core::SaturationPolicy::FailFast,
                _ => causm_core::SaturationPolicy::EvictDecayed,
            };
            causm_core::Statement::PolicyStmt {
                target: target_enum,
                policy: kind_enum,
            }
        }
        StmtNode::Select { max_ms, cases } => {
            let mut select_cases = Vec::new();
            for &cid in &arena.stmt_pool[cases.as_range()] {
                if let StmtNode::Assign { target, value } =
                    &arena.statements[cid.0 as usize]
                {
                    select_cases.push(causm_core::SelectCase {
                        binding: causm_core::symbol::resolve(*target),
                        source: super::pratt::to_ast_expression(arena, *value),
                        body: Vec::new(),
                    });
                }
            }
            causm_core::Statement::Select {
                max_ms: *max_ms,
                cases: select_cases,
                timeout: None,
                reconcile: None,
            }
        }
        StmtNode::Match { target, arms } => {
            let mut is_entropy = false;
            let mut valid_branch = None;
            let mut decayed_branch = None;
            let mut pending_branch = None;
            let mut consumed_branch = None;
            let mut std_arms = Vec::new();

            for arm in &arena.match_arms_pool[arms.as_range()] {
                let pat_str = causm_core::symbol::resolve(arm.pattern);
                let guard_expr = arm.guard.map(|gid| super::pratt::to_ast_expression(arena, gid));
                let body_stmts = arena.stmt_pool[arm.body.as_range()]
                    .iter()
                    .map(|&sid| to_ast_statement(arena, sid))
                    .collect();
                match pat_str.as_str() {
                    s if s.starts_with("Valid") => {
                        is_entropy = true;
                        if valid_branch.is_none() {
                            let binding = s
                                .split_once(':')
                                .map(|(_, b)| b.trim().to_string())
                                .unwrap_or_else(|| {
                                    if let Some((_, rest)) = s.split_once('(') {
                                        rest.trim_end_matches(')').trim().to_string()
                                    } else {
                                        String::new()
                                    }
                                });
                            valid_branch = Some((
                                causm_core::DecayedPattern::Binding(binding),
                                guard_expr,
                                body_stmts,
                            ));
                        }
                    }
                    s if s.starts_with("Decayed") => {
                        is_entropy = true;
                        let pattern = if let Some((_, payload)) = s.split_once(':') {
                            let mut fields = std::collections::HashMap::new();
                            let tokens: Vec<&str> =
                                payload.split_whitespace().collect();
                            let mut i = 0;
                            while i < tokens.len() {
                                let key = tokens[i];
                                if !matches!(
                                    key,
                                    "Valid" | "Decayed" | "Pending" | "Consumed"
                                ) {
                                    let state_val = if i + 1 < tokens.len()
                                        && matches!(
                                            tokens[i + 1],
                                            "Valid"
                                                | "Decayed"
                                                | "Pending"
                                                | "Consumed"
                                        ) {
                                        i += 1;
                                        tokens[i]
                                    } else {
                                        "Valid"
                                    };
                                    fields.insert(
                                        key.to_string(),
                                        causm_core::PatternValue::State(
                                            state_val.to_string(),
                                        ),
                                    );
                                }
                                i += 1;
                            }
                            if fields.is_empty() {
                                causm_core::DecayedPattern::Binding(String::new())
                            } else {
                                causm_core::DecayedPattern::Fields(fields)
                            }
                        } else {
                            causm_core::DecayedPattern::Binding(String::new())
                        };
                        decayed_branch = Some((pattern, guard_expr, body_stmts));
                    }
                    "Pending" => {
                        is_entropy = true;
                        pending_branch = Some((
                            causm_core::DecayedPattern::Binding(String::new()),
                            guard_expr,
                            body_stmts,
                        ));
                    }
                    "Consumed" => {
                        is_entropy = true;
                        consumed_branch = Some((guard_expr, body_stmts));
                    }
                    _ => {
                        std_arms.push(causm_core::MatchArm {
                            pattern: super::pratt::parse_pattern_from_str(&pat_str),
                            guard: guard_expr,
                            body: body_stmts,
                        });
                    }
                }
            }

            if is_entropy {
                let tgt_expr = super::pratt::to_ast_expression(arena, *target);
                let unwrapped_tgt = match tgt_expr {
                    causm_core::Expression::Call { routine, mut args }
                        if routine == "entropy" && !args.is_empty() =>
                    {
                        args.remove(0)
                    }
                    other => other,
                };
                causm_core::Statement::MatchEntropy {
                    target: unwrapped_tgt,
                    valid_branch,
                    decayed_branch,
                    pending_branch,
                    consumed_branch,
                }
            } else {
                causm_core::Statement::Match {
                    target: super::pratt::to_ast_expression(arena, *target),
                    arms: std_arms,
                }
            }
        }
        _ => causm_core::Statement::Expression(causm_core::Expression::Null),
    };
    let attrs = arena
        .stmt_attributes
        .get(&id.0)
        .cloned()
        .unwrap_or_default();
    causm_core::SpannedStatement::with_attributes(stmt, span, attrs)
}

pub fn parse_arena_program_to_ast(
    source: &str,
) -> Result<causm_core::Program, String> {
    let mut parser = ArenaParser::new(source);
    let root = parser.parse_program()?;
    let mut timelines = Vec::new();
    let mut standalone = Vec::new();

    for &sid in &parser.arena.root_statements[root.as_range()] {
        if let StmtNode::TimelineBlock {
            coord,
            directives,
            body,
        } = &parser.arena.statements[sid.0 as usize]
        {
            let mut no_z3 = false;
            let mut entropy_mode = None;
            for dir in directives {
                match dir {
                    causm_core::BlockDirective::NoZ3 => no_z3 = true,
                    causm_core::BlockDirective::Chaos => {
                        entropy_mode = Some(causm_core::EntropyMode::Chaos);
                    }
                    causm_core::BlockDirective::Deterministic => {
                        entropy_mode = Some(causm_core::EntropyMode::Deterministic);
                    }
                }
            }
            let mut stmts = Vec::new();
            for &s in &parser.arena.stmt_pool[body.as_range()] {
                stmts.push(to_ast_statement(&parser.arena, s));
            }
            timelines.push(causm_core::TimelineBlock {
                time: coord.clone(),
                no_z3,
                entropy_mode,
                statements: stmts,
            });
        } else {
            standalone.push(to_ast_statement(&parser.arena, sid));
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
    Ok(prog)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_arena_parser_expressions_and_routines() {
        let src = r#"
            routine add(x: int, y: int) -> int {
                return x + y * 2
            }
            let a = 10
            let b = add(a, 5)
        "#;
        let prog = parse_arena_program_to_ast(src)
            .expect("should parse successfully without semicolons");
        assert!(!prog.timelines.is_empty());
        let stmts = &prog.timelines[0].statements;
        assert_eq!(stmts.len(), 3);
    }

    #[test]
    fn test_syntax_arena_parser_struct_and_enum() {
        let src = r#"
            struct Point {
                x: int = 0,
                y: int = 0
            }
            enum Status {
                Active,
                Inactive
            }
            let p = Point;
        "#;
        let prog =
            parse_arena_program_to_ast(src).expect("should parse successfully");
        assert!(!prog.timelines.is_empty());
        let stmts = &prog.timelines[0].statements;
        assert_eq!(stmts.len(), 3);
    }

    #[test]
    fn test_syntax_arena_parser_advanced_statements() {
        let src = r#"
            anchor checkpoint_1;
            rewind_to(checkpoint_1);
            lease res = source 500ms {
                print("in lease");
            }
            for i in 0..10 step 100ms {
                print(i);
            }
            let p = Point { x: 10, y: 20 };
            let rep = [0; 8];
            let tup = (1, 2, 3);
            let s = syscall(1, 42);
            let m = match x {
                1 => 10,
                _ => 20
            };
        "#;
        let prog = parse_arena_program_to_ast(src)
            .expect("should parse all advanced statements");
        assert!(!prog.timelines.is_empty());
        assert_eq!(prog.timelines[0].statements.len(), 9);
    }

    #[test]
    fn test_syntax_enum_variant_nested_call_argument_isolation() {
        let src = r#"
            let agent_opt = if (cond) {
                Option::Some(clone(extracted_id))
            } else {
                Option::None
            }
        "#;
        let prog = parse_arena_program_to_ast(src)
            .expect("should parse nested call inside enum variant without arg pool corruption");
        assert!(!prog.timelines.is_empty());
        if let causm_core::Statement::Assignment { expr, .. } = &prog.timelines[0].statements[0].stmt {
            if let causm_core::Expression::If { then_branch, .. } = expr {
                if let causm_core::Expression::EnumVariant { args, .. } = &**then_branch {
                    assert_eq!(args.len(), 1, "Option::Some should only have 1 argument");
                } else {
                    panic!("expected EnumVariant then branch");
                }
            } else {
                panic!("expected If expression");
            }
        } else {
            panic!("expected Assignment statement");
        }
    }

    #[test]
    fn test_syntax_loop_step_and_tick_modifiers() {
        let src = r#"
            loop step 10ms max 30ms {
                break
            }
            loop tick {
                break
            }
            for m in members step _ {
                let x = m
            }
            for x consume arr pacing 5ms (max 20ms) {
                let y = x
            }
        "#;
        let prog = parse_arena_program_to_ast(src)
            .expect("should parse loops with distinct step, tick, and pacing modifiers");
        assert!(!prog.timelines.is_empty());
        let stmts = &prog.timelines[0].statements;
        assert_eq!(stmts.len(), 4);
        assert!(matches!(stmts[0].stmt, causm_core::Statement::Loop { max_ms: 30, .. }));
        assert!(matches!(stmts[1].stmt, causm_core::Statement::LoopTick { .. }));
        assert!(matches!(stmts[2].stmt, causm_core::Statement::ForStep { step_ms: None, .. }));
        assert!(matches!(stmts[3].stmt, causm_core::Statement::For { pacing_ms: Some(5), max_ms: Some(20), .. }));
    }

    #[test]
    fn test_syntax_namespaced_generic_static_call() {
        let src = r#"
            let buf = Collection.Buffer<u8>::new(1024)
        "#;
        let prog = parse_arena_program_to_ast(src)
            .expect("should parse namespaced generic static call successfully");
        assert!(!prog.timelines.is_empty());
        if let causm_core::Statement::Assignment { expr, .. } = &prog.timelines[0].statements[0].stmt {
            if let causm_core::Expression::Call { routine, args } = expr {
                assert_eq!(routine, "Collection.Buffer.new");
                assert_eq!(args.len(), 1);
            } else {
                panic!("expected Call expression");
            }
        } else {
            panic!("expected Assignment statement");
        }
    }

    #[test]
    fn test_syntax_state_type_annotation() {
        let src = r#"
            state total_cycles: int = 0
            state persistent_buffer: [int] = [0, 0, 0, 0]
        "#;
        let prog = parse_arena_program_to_ast(src)
            .expect("should parse state declarations with complex types successfully");
        assert!(!prog.timelines.is_empty());
        assert_eq!(prog.timelines[0].statements.len(), 2);
        assert!(matches!(prog.timelines[0].statements[0].stmt, causm_core::Statement::StateDecl { .. }));
        assert!(matches!(prog.timelines[0].statements[1].stmt, causm_core::Statement::StateDecl { .. }));
    }

    #[test]
    fn test_syntax_select_timeout_reconcile() {
        let src = r#"
            select (taking 10ms) {
                timeout: {
                    print("Select timed out")
                }
            } reconcile auto
        "#;
        let prog = parse_arena_program_to_ast(src)
            .expect("should parse select with timeout and reconcile auto successfully");
        assert!(!prog.timelines.is_empty());
        assert!(matches!(prog.timelines[0].statements[0].stmt, causm_core::Statement::Select { max_ms: 10, .. }));
    }
}
