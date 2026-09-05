use super::lexer::{Token, TokenKind, TokenStream};
use causm_core::arena::{AstArena, ExprId, ExprNode, LiteralKind, SliceRange};
use causm_core::{BinaryOperator, Span, UnaryOperator};

pub struct PrattParser<'a, 'b> {
    stream: TokenStream<'a>,
    current: Token,
    prev_span: causm_core::Span,
    arena: &'b mut AstArena,
    pub disallow_struct_lit: bool,
}

impl<'a, 'b> PrattParser<'a, 'b> {
    pub fn new(source: &'a str, arena: &'b mut AstArena) -> Self {
        let mut stream = TokenStream::new(source);
        let current = stream.next_token();
        let prev_span = current.span.clone();
        Self {
            stream,
            current,
            prev_span,
            arena,
            disallow_struct_lit: false,
        }
    }

    pub fn from_stream(
        stream: TokenStream<'a>,
        current: Token,
        arena: &'b mut AstArena,
    ) -> Self {
        let prev_span = current.span.clone();
        Self {
            stream,
            current,
            prev_span,
            arena,
            disallow_struct_lit: false,
        }
    }

    pub fn into_parts(self) -> (TokenStream<'a>, Token) {
        (self.stream, self.current)
    }

    fn bump(&mut self) -> Token {
        let tok = std::mem::replace(&mut self.current, self.stream.next_token());
        self.prev_span = tok.span.clone();
        tok
    }

    fn extract_qual_name(&self, expr_id: ExprId) -> String {
        match &self.arena.expressions[expr_id.0 as usize] {
            ExprNode::Identifier(s) => causm_core::symbol::resolve(*s),
            ExprNode::FieldAccess { target, field } => {
                let prefix = self.extract_qual_name(*target);
                format!("{}.{}", prefix, causm_core::symbol::resolve(*field))
            }
            _ => "unknown".to_string(),
        }
    }

    fn has_newline_before_peek(&self) -> bool {
        let prev_end = self.prev_span.end;
        let next_start = self.current.span.start;
        if next_start >= prev_end && next_start <= self.stream.source.len() {
            self.stream.source[prev_end..next_start].contains('\n')
        } else {
            false
        }
    }

    fn peek(&self) -> &TokenKind {
        &self.current.kind
    }

    fn prefix_binding_power(op: &TokenKind) -> Option<u8> {
        match op {
            TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Amp
            | TokenKind::Bang
            | TokenKind::Tilde => Some(17),
            _ => None,
        }
    }

    fn infix_binding_power(op: &TokenKind) -> Option<(u8, u8)> {
        match op {
            TokenKind::Dot | TokenKind::DoubleColon | TokenKind::Question => {
                Some((19, 20))
            }
            TokenKind::As => Some((17, 18)),
            TokenKind::StarStar => Some((16, 15)),
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => {
                Some((15, 16))
            }
            TokenKind::Plus | TokenKind::Minus => Some((13, 14)),
            TokenKind::Shl | TokenKind::Shr => Some((11, 12)),
            TokenKind::Lt | TokenKind::LtEq | TokenKind::Gt | TokenKind::GtEq => {
                Some((9, 10))
            }
            TokenKind::EqEq | TokenKind::BangEq => Some((7, 8)),
            TokenKind::Amp | TokenKind::Caret | TokenKind::Pipe => Some((6, 7)),
            TokenKind::AmpAmp | TokenKind::PipePipe | TokenKind::NullCoalesce => {
                Some((5, 6))
            }
            TokenKind::Pipeline => Some((4, 5)),
            _ => None,
        }
    }

    pub fn parse_expression(&mut self, min_bp: u8) -> Result<ExprId, String> {
        let mut lhs = match self.peek().clone() {
            TokenKind::Int(val) => {
                let tok = self.bump();
                self.arena.alloc_expr(
                    ExprNode::Literal(LiteralKind::Integer(val)),
                    tok.span,
                )
            }
            TokenKind::Duration(val) => {
                let tok = self.bump();
                self.arena.alloc_expr(
                    ExprNode::Literal(LiteralKind::Duration(val)),
                    tok.span,
                )
            }
            TokenKind::Float(ref s) => {
                let val = s.clone();
                let tok = self.bump();
                self.arena
                    .alloc_expr(ExprNode::Literal(LiteralKind::Float(val)), tok.span)
            }
            TokenKind::Str(ref s) => {
                let val = s.clone();
                let tok = self.bump();
                self.arena.alloc_expr(
                    ExprNode::Literal(LiteralKind::String(val)),
                    tok.span,
                )
            }
            TokenKind::FStr(ref s) => {
                let raw_str = s.clone();
                let tok = self.bump();
                let start = self.arena.fstring_parts_pool.len();
                let mut chars = raw_str.chars().peekable();
                let mut current_lit = String::new();
                while let Some(ch) = chars.next() {
                    if ch == '{' {
                        if !current_lit.is_empty() {
                            let unescaped =
                                crate::parser::expressions::unescape_raw_text(
                                    &current_lit,
                                );
                            self.arena.fstring_parts_pool.push(
                                causm_core::arena::FStringPartNode::Text(unescaped),
                            );
                            current_lit.clear();
                        }
                        let mut expr_str = String::new();
                        while let Some(&inner_c) = chars.peek() {
                            if inner_c == '}' {
                                chars.next();
                                break;
                            } else {
                                expr_str.push(chars.next().unwrap());
                            }
                        }
                        let mut sub_parser =
                            super::arena_parser::ArenaParser::new(&expr_str);
                        if let Ok(eid) = sub_parser.parse_expression(0) {
                            let converted_ast =
                                to_ast_expression(&sub_parser.arena, eid);
                            let sub_eid = match converted_ast {
                                causm_core::Expression::Identifier(sym) => {
                                    self.arena.alloc_expr(
                                        ExprNode::Identifier(
                                            causm_core::symbol::intern(&sym),
                                        ),
                                        tok.span.clone(),
                                    )
                                }
                                causm_core::Expression::BinaryOp {
                                    left,
                                    right,
                                    op,
                                } => {
                                    let l_eid = match *left {
                                        causm_core::Expression::Identifier(s) => {
                                            self.arena.alloc_expr(
                                                ExprNode::Identifier(
                                                    causm_core::symbol::intern(&s),
                                                ),
                                                tok.span.clone(),
                                            )
                                        }
                                        _ => self.arena.alloc_expr(
                                            ExprNode::Literal(LiteralKind::Integer(
                                                0,
                                            )),
                                            tok.span.clone(),
                                        ),
                                    };
                                    let r_eid = match *right {
                                        causm_core::Expression::Identifier(s) => {
                                            self.arena.alloc_expr(
                                                ExprNode::Identifier(
                                                    causm_core::symbol::intern(&s),
                                                ),
                                                tok.span.clone(),
                                            )
                                        }
                                        _ => self.arena.alloc_expr(
                                            ExprNode::Literal(LiteralKind::Integer(
                                                0,
                                            )),
                                            tok.span.clone(),
                                        ),
                                    };
                                    self.arena.alloc_expr(
                                        ExprNode::BinaryOp {
                                            left: l_eid,
                                            right: r_eid,
                                            op,
                                        },
                                        tok.span.clone(),
                                    )
                                }
                                _ => self.arena.alloc_expr(
                                    ExprNode::Literal(LiteralKind::Integer(0)),
                                    tok.span.clone(),
                                ),
                            };
                            self.arena.fstring_parts_pool.push(
                                causm_core::arena::FStringPartNode::Expr(sub_eid),
                            );
                        }
                    } else {
                        current_lit.push(ch);
                    }
                }
                if !current_lit.is_empty() {
                    let unescaped =
                        crate::parser::expressions::unescape_raw_text(&current_lit);
                    self.arena
                        .fstring_parts_pool
                        .push(causm_core::arena::FStringPartNode::Text(unescaped));
                }
                let end = self.arena.fstring_parts_pool.len();
                self.arena.alloc_expr(
                    ExprNode::FString(SliceRange::new(start, end)),
                    tok.span,
                )
            }
            TokenKind::ByteStr(ref bytes) | TokenKind::HexByteStr(ref bytes) => {
                let bytes_clone = bytes.clone();
                let tok = self.bump();
                let start = self.arena.expr_pool.len();
                for b in bytes_clone {
                    let eid = self.arena.alloc_expr(
                        ExprNode::Literal(LiteralKind::Integer(b as i64)),
                        tok.span.clone(),
                    );
                    self.arena.expr_pool.push(eid);
                }
                let end = self.arena.expr_pool.len();
                self.arena.alloc_expr(
                    ExprNode::ArrayLit(SliceRange::new(start, end)),
                    tok.span,
                )
            }
            TokenKind::Bool(val) => {
                let tok = self.bump();
                self.arena.alloc_expr(
                    ExprNode::Literal(LiteralKind::Boolean(val)),
                    tok.span,
                )
            }
            TokenKind::Null => {
                let tok = self.bump();
                self.arena
                    .alloc_expr(ExprNode::Literal(LiteralKind::Null), tok.span)
            }
            TokenKind::Ident(sym) => {
                let tok = self.bump();
                if self.peek() == &TokenKind::DoubleColon {
                    if self.stream.peek_token().kind == TokenKind::Lt {
                        self.bump(); // consume DoubleColon
                        self.bump(); // consume Lt
                        while self.peek() != &TokenKind::Gt
                            && self.peek() != &TokenKind::Eof
                        {
                            self.bump();
                        }
                        if self.peek() == &TokenKind::Gt {
                            self.bump();
                        }
                        let mut args_vec = Vec::new();
                        if self.peek() == &TokenKind::LParen {
                            self.bump();
                            while self.peek() != &TokenKind::RParen
                                && self.peek() != &TokenKind::Eof
                            {
                                args_vec.push(self.parse_expression(0)?);
                                if self.peek() == &TokenKind::Comma {
                                    self.bump();
                                }
                            }
                            if self.peek() == &TokenKind::RParen {
                                self.bump();
                            }
                        }
                        let a_start = self.arena.expr_pool.len();
                        for a in args_vec {
                            self.arena.expr_pool.push(a);
                        }
                        let a_end = self.arena.expr_pool.len();
                        let routine = self
                            .arena
                            .alloc_expr(ExprNode::Identifier(sym), tok.span.clone());
                        return Ok(self.arena.alloc_expr(
                            ExprNode::Call {
                                routine,
                                args: SliceRange::new(a_start, a_end),
                            },
                            tok.span,
                        ));
                    }
                }
                if self.peek() == &TokenKind::Lt {
                    let mut clone = self.stream.clone();
                    let mut is_generic_static = false;
                    while clone.peek_token().kind != TokenKind::Eof {
                        if clone.peek_token().kind == TokenKind::Gt {
                            clone.next_token();
                            if clone.peek_token().kind == TokenKind::DoubleColon {
                                is_generic_static = true;
                            }
                            break;
                        }
                        clone.next_token();
                    }
                    if is_generic_static {
                        self.bump(); // consume <
                        while self.peek() != &TokenKind::Gt
                            && self.peek() != &TokenKind::Eof
                        {
                            self.bump();
                        }
                        if self.peek() == &TokenKind::Gt {
                            self.bump();
                        }
                        if self.peek() == &TokenKind::DoubleColon {
                            self.bump();
                        }
                        let method_sym = match self.peek() {
                            TokenKind::Ident(m) => *m,
                            _ => causm_core::symbol::intern("new"),
                        };
                        if matches!(self.peek(), TokenKind::Ident(_)) {
                            self.bump();
                        }
                        let mut args_vec = Vec::new();
                        if self.peek() == &TokenKind::LParen {
                            self.bump();
                            while self.peek() != &TokenKind::RParen
                                && self.peek() != &TokenKind::Eof
                            {
                                args_vec.push(self.parse_expression(0)?);
                                if self.peek() == &TokenKind::Comma {
                                    self.bump();
                                }
                            }
                            if self.peek() == &TokenKind::RParen {
                                self.bump();
                            }
                        }
                        let type_name = causm_core::symbol::resolve(sym);
                        let method = causm_core::symbol::resolve(method_sym);
                        let routine_name = format!("{}.{}", type_name, method);
                        let routine_sym = causm_core::symbol::intern(&routine_name);
                        let routine = self.arena.alloc_expr(
                            ExprNode::Identifier(routine_sym),
                            tok.span.clone(),
                        );
                        let a_start = self.arena.expr_pool.len();
                        for a in args_vec {
                            self.arena.expr_pool.push(a);
                        }
                        let a_end = self.arena.expr_pool.len();
                        return Ok(self.arena.alloc_expr(
                            ExprNode::Call {
                                routine,
                                args: SliceRange::new(a_start, a_end),
                            },
                            tok.span,
                        ));
                    }
                }
                if self.peek() == &TokenKind::Bang {
                    self.bump();
                    if self.peek() == &TokenKind::LParen {
                        self.bump();
                        let m_name = causm_core::symbol::resolve(sym);
                        let routine_sym = causm_core::symbol::intern(&format!(
                            "__macro_call__{}",
                            m_name
                        ));
                        let routine = self.arena.alloc_expr(
                            ExprNode::Identifier(routine_sym),
                            tok.span.clone(),
                        );
                        let mut args_vec = Vec::new();
                        while self.peek() != &TokenKind::RParen
                            && self.peek() != &TokenKind::Eof
                        {
                            let mut arg_str = String::new();
                            let mut depth = 0;
                            while self.peek() != &TokenKind::Eof {
                                if self.peek() == &TokenKind::LParen {
                                    depth += 1;
                                }
                                if self.peek() == &TokenKind::RParen {
                                    if depth == 0 {
                                        break;
                                    }
                                    depth -= 1;
                                }
                                if self.peek() == &TokenKind::Comma && depth == 0 {
                                    break;
                                }
                                let t = self.bump();
                                let piece = match t.kind {
                                    TokenKind::Ident(s) => {
                                        causm_core::symbol::resolve(s)
                                    }
                                    TokenKind::Int(i) => i.to_string(),
                                    TokenKind::Float(f) => f.to_string(),
                                    TokenKind::Str(s) => format!("\"{}\"", s),
                                    TokenKind::Plus => "+".into(),
                                    TokenKind::Minus => "-".into(),
                                    TokenKind::Star => "*".into(),
                                    TokenKind::Slash => "/".into(),
                                    _ => "".into(),
                                };
                                if !piece.is_empty() {
                                    if !arg_str.is_empty() {
                                        arg_str.push(' ');
                                    }
                                    arg_str.push_str(&piece);
                                }
                            }
                            let arg_id = self.arena.alloc_expr(
                                ExprNode::Literal(LiteralKind::String(
                                    arg_str.trim().to_string(),
                                )),
                                tok.span.clone(),
                            );
                            args_vec.push(arg_id);
                            if self.peek() == &TokenKind::Comma {
                                self.bump();
                            }
                        }
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                        let a_start = self.arena.expr_pool.len();
                        for a in args_vec {
                            self.arena.expr_pool.push(a);
                        }
                        let a_end = self.arena.expr_pool.len();
                        return Ok(self.arena.alloc_expr(
                            ExprNode::Call {
                                routine,
                                args: SliceRange::new(a_start, a_end),
                            },
                            tok.span,
                        ));
                    }
                }
                if !self.disallow_struct_lit && self.peek() == &TokenKind::LBrace && !self.has_newline_before_peek() {
                    let next2 = self.stream.peek_token();
                    let is_struct_field = match next2.kind {
                        TokenKind::Ident(_) | TokenKind::Str(_) => {
                            let mut clone = self.stream.clone();
                            clone.next_token();
                            matches!(
                                clone.peek_token().kind,
                                TokenKind::Colon | TokenKind::Comma | TokenKind::Eq
                            )
                        }
                        TokenKind::RBrace => true,
                        _ => false,
                    };
                    if is_struct_field {
                        self.bump(); // LBrace
                        let mut local_fields = Vec::new();
                        while self.peek() != &TokenKind::RBrace
                            && self.peek() != &TokenKind::Eof
                        {
                            let mut key_sym_opt = None;
                            if let TokenKind::Ident(f_sym) = self.peek() {
                                key_sym_opt = Some(*f_sym);
                                self.bump();
                            } else if let TokenKind::Str(s_str) = self.peek() {
                                key_sym_opt = Some(causm_core::symbol::intern(s_str));
                                self.bump();
                            } else if let Some(sym) = self.peek().as_ident_symbol() {
                                key_sym_opt = Some(sym);
                                self.bump();
                            }

                            if let Some(field) = key_sym_opt {
                                let expr = if self.peek() == &TokenKind::Colon
                                    || self.peek() == &TokenKind::Eq
                                {
                                    self.bump();
                                    self.parse_expression(0)?
                                } else {
                                    self.arena.alloc_expr(
                                        ExprNode::Identifier(field),
                                        tok.span.clone(),
                                    )
                                };
                                local_fields.push(
                                    causm_core::arena::FieldAssignNode {
                                        field,
                                        expr,
                                        type_name: None,
                                        is_const: false,
                                    },
                                );
                                if self.peek() == &TokenKind::Comma {
                                    self.bump();
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        if self.peek() == &TokenKind::RBrace {
                            self.bump();
                        }
                        let f_start = self.arena.field_assigns_pool.len();
                        for field_assign in local_fields {
                            self.arena.field_assigns_pool.push(field_assign);
                        }
                        let f_end = self.arena.field_assigns_pool.len();
                        self.arena.alloc_expr(
                            ExprNode::StructLit {
                                type_sym: Some(sym),
                                fields: SliceRange::new(f_start, f_end),
                            },
                            tok.span,
                        )
                    } else {
                        self.arena.alloc_expr(ExprNode::Identifier(sym), tok.span)
                    }
                } else {
                    self.arena.alloc_expr(ExprNode::Identifier(sym), tok.span)
                }
            }
            TokenKind::Send => {
                let tok = self.bump();
                let sym = causm_core::symbol::intern("send");
                self.arena.alloc_expr(ExprNode::Identifier(sym), tok.span)
            }
            TokenKind::State => {
                let tok = self.bump();
                let sym = causm_core::symbol::intern("state");
                self.arena.alloc_expr(ExprNode::Identifier(sym), tok.span)
            }
            TokenKind::Slice => {
                let tok = self.bump();
                let sym = causm_core::symbol::intern("slice");
                self.arena.alloc_expr(ExprNode::Identifier(sym), tok.span)
            }
            TokenKind::Step => {
                let tok = self.bump();
                let sym = causm_core::symbol::intern("step");
                self.arena.alloc_expr(ExprNode::Identifier(sym), tok.span)
            }
            TokenKind::Struct => {
                let tok = self.bump();
                if self.peek() == &TokenKind::LBrace {
                    self.bump();
                    let mut local_fields = Vec::new();
                    while self.peek() != &TokenKind::RBrace
                        && self.peek() != &TokenKind::Eof
                    {
                    let mut key_sym_opt = None;
                    if let TokenKind::Ident(f_sym) = self.peek() {
                        key_sym_opt = Some(*f_sym);
                        self.bump();
                    } else if let TokenKind::Str(s) = self.peek() {
                        key_sym_opt = Some(causm_core::symbol::intern(s));
                        self.bump();
                    } else if let Some(sym) = self.peek().as_ident_symbol() {
                        key_sym_opt = Some(sym);
                        self.bump();
                    }

                    if let Some(field) = key_sym_opt {
                        let expr = if self.peek() == &TokenKind::Colon
                            || self.peek() == &TokenKind::Eq
                        {
                            self.bump();
                            self.parse_expression(0)?
                        } else {
                            self.arena.alloc_expr(
                                ExprNode::Identifier(field),
                                tok.span.clone(),
                            )
                        };
                        local_fields.push(
                            causm_core::arena::FieldAssignNode {
                                field,
                                expr,
                                type_name: None,
                                is_const: false,
                            },
                        );
                        if self.peek() == &TokenKind::Comma {
                            self.bump();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                if self.peek() == &TokenKind::RBrace {
                    self.bump();
                }
                let f_start = self.arena.field_assigns_pool.len();
                for field_assign in local_fields {
                    self.arena.field_assigns_pool.push(field_assign);
                }
                let f_end = self.arena.field_assigns_pool.len();
                self.arena.alloc_expr(
                    ExprNode::StructLit {
                        type_sym: None,
                        fields: SliceRange::new(f_start, f_end),
                    },
                    tok.span,
                )
            } else {
                self.arena
                    .alloc_expr(ExprNode::Literal(LiteralKind::Null), tok.span)
            }
        }

            TokenKind::LBrace => {
                let tok = self.bump();
                let f_start = self.arena.field_assigns_pool.len();
                while self.peek() != &TokenKind::RBrace
                    && self.peek() != &TokenKind::Eof
                {
                    if let TokenKind::Ident(f_sym) = self.peek() {
                        let field = *f_sym;
                        self.bump();
                        let expr = if self.peek() == &TokenKind::Colon
                            || self.peek() == &TokenKind::Eq
                        {
                            self.bump();
                            self.parse_expression(0)?
                        } else {
                            self.arena.alloc_expr(
                                ExprNode::Identifier(field),
                                tok.span.clone(),
                            )
                        };
                        self.arena.field_assigns_pool.push(
                            causm_core::arena::FieldAssignNode {
                                field,
                                expr,
                                type_name: None,
                                is_const: false,
                            },
                        );
                        if self.peek() == &TokenKind::Comma {
                            self.bump();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                if self.peek() == &TokenKind::RBrace {
                    self.bump();
                }
                let f_end = self.arena.field_assigns_pool.len();
                self.arena.alloc_expr(
                    ExprNode::StructLit {
                        type_sym: None,
                        fields: SliceRange::new(f_start, f_end),
                    },
                    tok.span,
                )
            }
            TokenKind::If => {
                let tok = self.bump();
                let cond = self.parse_expression(0)?;
                let then_expr = if self.peek() == &TokenKind::LBrace {
                    self.bump();
                    let e = self.parse_expression(0)?;
                    if self.peek() == &TokenKind::RBrace {
                        self.bump();
                    }
                    e
                } else {
                    self.parse_expression(0)?
                };
                let then_stmt = self.arena.alloc_stmt(
                    causm_core::arena::StmtNode::Expr(then_expr),
                    tok.span.clone(),
                );
                let else_stmt = if self.peek() == &TokenKind::Else {
                    self.bump();
                    let e = if self.peek() == &TokenKind::LBrace {
                        self.bump();
                        let inner = self.parse_expression(0)?;
                        if self.peek() == &TokenKind::RBrace {
                            self.bump();
                        }
                        inner
                    } else {
                        self.parse_expression(0)?
                    };
                    Some(self.arena.alloc_stmt(
                        causm_core::arena::StmtNode::Expr(e),
                        tok.span.clone(),
                    ))
                } else {
                    None
                };
                self.arena.alloc_expr(
                    ExprNode::If {
                        cond,
                        then_branch: then_stmt,
                        else_branch: else_stmt,
                    },
                    tok.span,
                )
            }
            TokenKind::Match => {
                let tok = self.bump();
                let target = self.parse_expression(0)?;
                let a_start = self.arena.match_arms_pool.len();
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
                            let at_top = paren_depth == 0 && brace_depth == 0 && bracket_depth == 0;
                            if at_top && (self.peek() == &TokenKind::FatArrow
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
                                TokenKind::Valid => pat_str.push_str("Valid"),
                                TokenKind::Decayed => pat_str.push_str("Decayed"),
                                TokenKind::Pending => pat_str.push_str("Pending"),
                                TokenKind::Consumed => pat_str.push_str("Consumed"),
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
                                _ => {
                                    pat_str.push('_');
                                }
                            }
                            self.bump();
                        }
                        let pat = causm_core::symbol::intern(&pat_str);
                        if self.peek() == &TokenKind::FatArrow
                            || self.peek() == &TokenKind::Colon
                        {
                            self.bump();
                        }
                        let body_expr = self.parse_expression(0)?;
                        let s_start = self.arena.stmt_pool.len();
                        let body_sid = self.arena.alloc_stmt(
                            causm_core::arena::StmtNode::Expr(body_expr),
                            tok.span.clone(),
                        );
                        self.arena.stmt_pool.push(body_sid);
                        let s_end = self.arena.stmt_pool.len();
                        self.arena.match_arms_pool.push(
                            causm_core::arena::MatchArmNode {
                                pattern: pat,
                                guard: None,
                                body: SliceRange::new(s_start, s_end),
                            },
                        );
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
                let a_end = self.arena.match_arms_pool.len();
                self.arena.alloc_expr(
                    ExprNode::Match {
                        target,
                        arms: SliceRange::new(a_start, a_end),
                    },
                    tok.span,
                )
            }
            TokenKind::Syscall => {
                let tok = self.bump();
                let mut target = causm_core::symbol::intern("0");
                let start = self.arena.expr_pool.len();
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    match self.peek() {
                        TokenKind::Str(s) => {
                            target = causm_core::symbol::intern(s);
                            self.bump();
                        }
                        TokenKind::Int(i) => {
                            target = causm_core::symbol::intern(&i.to_string());
                            self.bump();
                        }
                        _ => {}
                    }
                    while self.peek() == &TokenKind::Comma {
                        self.bump();
                        let arg = self.parse_expression(0)?;
                        self.arena.expr_pool.push(arg);
                    }
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                }
                let end = self.arena.expr_pool.len();
                let mut duration_ms = None;
                if self.peek() == &TokenKind::Taking {
                    self.bump();
                    if let TokenKind::Duration(ms) = self.peek() {
                        duration_ms = Some(*ms);
                        self.bump();
                    } else if let TokenKind::Int(ms) = self.peek() {
                        duration_ms = Some(*ms as u64);
                        self.bump();
                    }
                }
                self.arena.alloc_expr(
                    ExprNode::Syscall {
                        target,
                        args: SliceRange::new(start, end),
                        duration_ms,
                    },
                    tok.span,
                )
            }
            TokenKind::ChanRecv => {
                let tok = self.bump();
                let mut target = causm_core::symbol::intern("");
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    if let TokenKind::Ident(sym) = self.peek() {
                        target = *sym;
                        self.bump();
                    }
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                }
                self.arena.alloc_expr(ExprNode::ChanRecv(target), tok.span)
            }
            TokenKind::Arena => {
                let tok = self.bump();
                let mut kind = causm_core::symbol::intern("remaining");
                if self.peek() == &TokenKind::Dot {
                    self.bump();
                    if let TokenKind::Ident(k) = self.peek() {
                        kind = *k;
                        self.bump();
                    }
                    if self.peek() == &TokenKind::LParen {
                        self.bump();
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                    }
                }
                self.arena
                    .alloc_expr(ExprNode::ArenaIntrospect(kind), tok.span)
            }
            TokenKind::Defer => {
                let tok = self.bump();
                let mut cap_parts = Vec::new();
                while let TokenKind::Ident(s) = self.peek() {
                    cap_parts.push(causm_core::symbol::resolve(*s));
                    self.bump();
                    if self.peek() == &TokenKind::Dot {
                        self.bump();
                    } else {
                        break;
                    }
                }
                let cap = causm_core::symbol::intern(&cap_parts.join("."));
                let _p_start = self.arena.field_assigns_pool.len();
                if self.peek() == &TokenKind::LParen {
                    self.bump();
                    while self.peek() != &TokenKind::RParen
                        && self.peek() != &TokenKind::Eof
                    {
                        if let TokenKind::Ident(f_sym) = self.peek() {
                            let field = *f_sym;
                            self.bump();
                            let expr = if self.peek() == &TokenKind::Eq
                                || self.peek() == &TokenKind::Colon
                            {
                                self.bump();
                                self.parse_expression(0)?
                            } else {
                                self.arena.alloc_expr(
                                    ExprNode::Identifier(field),
                                    tok.span.clone(),
                                )
                            };
                            self.arena.field_assigns_pool.push(
                                causm_core::arena::FieldAssignNode {
                                    field,
                                    expr,
                                    type_name: None,
                                    is_const: false,
                                },
                            );
                            if self.peek() == &TokenKind::Comma {
                                self.bump();
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                }
                if self.peek() == &TokenKind::Taking || self.peek() == &TokenKind::For {
                    self.bump();
                } else if let TokenKind::Ident(s) = self.peek() {
                    if causm_core::symbol::resolve(*s) == "deadline" {
                        self.bump();
                    }
                }
                let mut duration_ms = 0;
                if let TokenKind::Duration(ms) = self.peek() {
                    duration_ms = *ms;
                    self.bump();
                } else if let TokenKind::Int(ms) = self.peek() {
                    duration_ms = *ms as u64;
                    self.bump();
                }
                self.arena.alloc_expr(
                    ExprNode::Defer {
                        capability: cap,
                        duration_ms,
                    },
                    tok.span,
                )
            }
            TokenKind::LBracket => {
                let tok = self.bump();
                let start = self.arena.expr_pool.len();
                if self.peek() == &TokenKind::RBracket {
                    self.bump();
                    let end = self.arena.expr_pool.len();
                    self.arena.alloc_expr(
                        ExprNode::ArrayLit(SliceRange::new(start, end)),
                        tok.span,
                    )
                } else {
                    let first = self.parse_expression(0)?;
                    if self.peek() == &TokenKind::Semi {
                        self.bump();
                        let count = self.parse_expression(0)?;
                        if self.peek() == &TokenKind::RBracket {
                            self.bump();
                        }
                        self.arena.alloc_expr(
                            ExprNode::ArrayRepeat {
                                value: first,
                                count,
                            },
                            tok.span,
                        )
                    } else {
                        self.arena.expr_pool.push(first);
                        while self.peek() == &TokenKind::Comma {
                            self.bump();
                            if self.peek() == &TokenKind::RBracket {
                                break;
                            }
                            let elem = self.parse_expression(0)?;
                            self.arena.expr_pool.push(elem);
                        }
                        if self.peek() == &TokenKind::RBracket {
                            self.bump();
                        }
                        let end = self.arena.expr_pool.len();
                        self.arena.alloc_expr(
                            ExprNode::ArrayLit(SliceRange::new(start, end)),
                            tok.span,
                        )
                    }
                }
            }
            TokenKind::LParen => {
                let tok = self.bump();
                if self.peek() == &TokenKind::RParen {
                    self.bump();
                    let start = self.arena.expr_pool.len();
                    let end = start;
                    self.arena.alloc_expr(
                        ExprNode::Tuple(SliceRange::new(start, end)),
                        tok.span,
                    )
                } else {
                    let first = self.parse_expression(0)?;
                    if self.peek() == &TokenKind::Comma {
                        let start = self.arena.expr_pool.len();
                        self.arena.expr_pool.push(first);
                        while self.peek() == &TokenKind::Comma {
                            self.bump();
                            if self.peek() == &TokenKind::RParen {
                                break;
                            }
                            let next_expr = self.parse_expression(0)?;
                            self.arena.expr_pool.push(next_expr);
                        }
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                        let end = self.arena.expr_pool.len();
                        self.arena.alloc_expr(
                            ExprNode::Tuple(SliceRange::new(start, end)),
                            tok.span,
                        )
                    } else {
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                        first
                    }
                }
            }
            ref op if Self::prefix_binding_power(op).is_some() => {
                let r_bp = Self::prefix_binding_power(op).unwrap();
                let op_tok = self.bump();
                let rhs = self.parse_expression(r_bp)?;
                if op_tok.kind == TokenKind::Amp {
                    self.arena.alloc_expr(ExprNode::Ref(rhs), op_tok.span)
                } else {
                    let unary_op = match op_tok.kind {
                        TokenKind::Minus => UnaryOperator::Neg,
                        TokenKind::Bang => UnaryOperator::Not,
                        TokenKind::Tilde => UnaryOperator::BitwiseNot,
                        _ => UnaryOperator::Neg,
                    };
                    self.arena.alloc_expr(
                        ExprNode::UnaryOp {
                            expr: rhs,
                            op: unary_op,
                        },
                        op_tok.span,
                    )
                }
            }
            _ => {
                return Err(format!(
                    "Unexpected token: {:?} at {}-{}",
                    self.peek(),
                    self.current.span.start,
                    self.current.span.end
                ));
            }
        };

        loop {
            if self.peek() == &TokenKind::LBrace {
                break;
            }
            if self.peek() == &TokenKind::Question {
                let q_tok = self.bump();
                lhs = self.arena.alloc_expr(ExprNode::Try(lhs), q_tok.span);
                continue;
            }

            if self.peek() == &TokenKind::LParen {
                if self.has_newline_before_peek()
                    || self.stream.peek_token().kind == TokenKind::Max
                    || self.stream.peek_token().kind == TokenKind::Step
                    || self.stream.peek_token().kind == TokenKind::Taking
                {
                    break;
                }
                let p_tok = self.bump();
                let mut args_vec = Vec::new();
                while self.peek() != &TokenKind::RParen
                    && self.peek() != &TokenKind::Eof
                {
                    let arg = self.parse_expression(0)?;
                    args_vec.push(arg);
                    if self.peek() == &TokenKind::Comma {
                        self.bump();
                    } else {
                        break;
                    }
                }
                if self.peek() == &TokenKind::RParen {
                    self.bump();
                }
                let start = self.arena.expr_pool.len();
                for a in args_vec {
                    self.arena.expr_pool.push(a);
                }
                let end = self.arena.expr_pool.len();
                lhs = self.arena.alloc_expr(
                    ExprNode::Call {
                        routine: lhs,
                        args: SliceRange::new(start, end),
                    },
                    p_tok.span,
                );
                continue;
            }

            if self.peek() == &TokenKind::LBracket {
                if self.has_newline_before_peek() {
                    break;
                }
                let b_tok = self.bump();
                let mut start = None;
                if self.peek() != &TokenKind::DotDot
                    && self.peek() != &TokenKind::RBracket
                {
                    start = Some(self.parse_expression(0)?);
                }
                if self.peek() == &TokenKind::DotDot || self.peek() == &TokenKind::DotDotEq {
                    let is_inclusive = self.peek() == &TokenKind::DotDotEq;
                    self.bump();
                    let mut end = None;
                    if self.peek() != &TokenKind::RBracket
                        && self.peek() != &TokenKind::Eof
                    {
                        end = Some(self.parse_expression(0)?);
                    }
                    if self.peek() == &TokenKind::RBracket {
                        self.bump();
                    }
                    lhs = self.arena.alloc_expr(
                        ExprNode::ArraySlice {
                            target: lhs,
                            start,
                            end,
                            inclusive: is_inclusive,
                        },
                        b_tok.span,
                    );
                    continue;
                }
                let index = start.unwrap_or(causm_core::arena::ExprId(0));
                if self.peek() == &TokenKind::RBracket {
                    self.bump();
                }
                lhs = self.arena.alloc_expr(
                    ExprNode::IndexAccess { target: lhs, index },
                    b_tok.span,
                );
                continue;
            }

            if self.peek() == &TokenKind::Dot {
                let dot_tok = self.bump();
                if self.peek() == &TokenKind::LParen {
                    self.bump(); // LParen
                    let typ_sym = match self.peek() {
                        TokenKind::Ident(s) => *s,
                        TokenKind::Type => causm_core::symbol::intern("type"),
                        _ => {
                            let s = format!("{:?}", self.peek());
                            causm_core::symbol::intern(&s)
                        }
                    };
                    self.bump();
                    if self.peek() == &TokenKind::RParen {
                        self.bump();
                    }
                    lhs = self.arena.alloc_expr(
                        ExprNode::TypeAssertion {
                            target: lhs,
                            cast_type: typ_sym,
                        },
                        dot_tok.span,
                    );
                    continue;
                }
                let member_sym = match self.peek() {
                    TokenKind::Ident(s) => Some(*s),
                    TokenKind::Send => Some(causm_core::symbol::intern("send")),
                    TokenKind::Type => Some(causm_core::symbol::intern("type")),
                    TokenKind::Auto => Some(causm_core::symbol::intern("auto")),
                    _ => None,
                };
                if let Some(method_sym) = member_sym {
                    let id_tok = self.bump();
                    if self.peek() == &TokenKind::LParen {
                        self.bump();
                        let mut args_vec = Vec::new();
                        while self.peek() != &TokenKind::RParen
                            && self.peek() != &TokenKind::Eof
                        {
                            let arg = self.parse_expression(0)?;
                            args_vec.push(arg);
                            if self.peek() == &TokenKind::Comma {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                        let start = self.arena.expr_pool.len();
                        for a in args_vec {
                            self.arena.expr_pool.push(a);
                        }
                        let end = self.arena.expr_pool.len();
                        lhs = self.arena.alloc_expr(
                            ExprNode::MethodCall {
                                target: lhs,
                                method: method_sym,
                                args: SliceRange::new(start, end),
                            },
                            id_tok.span,
                        );
                        continue;
                    } else {
                        lhs = self.arena.alloc_expr(
                            ExprNode::FieldAccess {
                                target: lhs,
                                field: method_sym,
                            },
                            id_tok.span,
                        );
                        continue;
                    }
                }
            }

            if self.peek() == &TokenKind::Lt {
                let mut clone = self.stream.clone();
                let mut is_generic_static = false;
                while clone.peek_token().kind != TokenKind::Eof {
                    if clone.peek_token().kind == TokenKind::Gt {
                        clone.next_token();
                        if clone.peek_token().kind == TokenKind::DoubleColon {
                            is_generic_static = true;
                        }
                        break;
                    }
                    clone.next_token();
                }
                if is_generic_static {
                    let lt_tok = self.bump(); // consume <
                    while self.peek() != &TokenKind::Gt && self.peek() != &TokenKind::Eof {
                        self.bump();
                    }
                    if self.peek() == &TokenKind::Gt {
                        self.bump();
                    }
                    if self.peek() == &TokenKind::DoubleColon {
                        self.bump();
                    }
                    let method_sym = match self.peek() {
                        TokenKind::Ident(m) => *m,
                        _ => causm_core::symbol::intern("new"),
                    };
                    if matches!(self.peek(), TokenKind::Ident(_)) {
                        self.bump();
                    }
                    let mut args_vec = Vec::new();
                    if self.peek() == &TokenKind::LParen {
                        self.bump();
                        while self.peek() != &TokenKind::RParen && self.peek() != &TokenKind::Eof {
                            args_vec.push(self.parse_expression(0)?);
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
                    let full_target_name = self.extract_qual_name(lhs);
                    let method = causm_core::symbol::resolve(method_sym);
                    let routine_name = format!("{}.{}", full_target_name, method);
                    let routine_sym = causm_core::symbol::intern(&routine_name);
                    let routine = self.arena.alloc_expr(
                        ExprNode::Identifier(routine_sym),
                        lt_tok.span.clone(),
                    );
                    let a_start = self.arena.expr_pool.len();
                    for a in args_vec {
                        self.arena.expr_pool.push(a);
                    }
                    let a_end = self.arena.expr_pool.len();
                    lhs = self.arena.alloc_expr(
                        ExprNode::Call {
                            routine,
                            args: SliceRange::new(a_start, a_end),
                        },
                        lt_tok.span,
                    );
                    continue;
                }
            }

            if self.peek() == &TokenKind::DoubleColon {
                self.bump();
                let v_sym_opt = match self.peek().clone() {
                    TokenKind::Ident(variant_sym) => Some(variant_sym),
                    TokenKind::Valid => Some(causm_core::symbol::intern("Valid")),
                    TokenKind::Lease => Some(causm_core::symbol::intern("Leased")),
                    TokenKind::Decayed => {
                        Some(causm_core::symbol::intern("Decayed"))
                    }
                    TokenKind::Pending => {
                        Some(causm_core::symbol::intern("Pending"))
                    }
                    TokenKind::Consumed => {
                        Some(causm_core::symbol::intern("Consumed"))
                    }
                    _ => None,
                };
                if let Some(variant_sym) = v_sym_opt {
                    let id_tok = self.bump();
                    let enum_str = match &self.arena.expressions[lhs.0 as usize] {
                        ExprNode::Identifier(s) => causm_core::symbol::resolve(*s),
                        _ => String::new(),
                    };
                    if self.peek() == &TokenKind::LParen {
                        self.bump();
                        let mut args_vec = Vec::new();
                        while self.peek() != &TokenKind::RParen
                            && self.peek() != &TokenKind::Eof
                        {
                            let arg = self.parse_expression(0)?;
                            args_vec.push(arg);
                            if self.peek() == &TokenKind::Comma {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                        if self.peek() == &TokenKind::RParen {
                            self.bump();
                        }
                        let start = self.arena.expr_pool.len();
                        for a in args_vec {
                            self.arena.expr_pool.push(a);
                        }
                        let end = self.arena.expr_pool.len();
                        lhs = self.arena.alloc_expr(
                            ExprNode::EnumVariant {
                                enum_name: causm_core::symbol::intern(&enum_str),
                                variant_name: variant_sym,
                                args: SliceRange::new(start, end),
                            },
                            id_tok.span,
                        );
                        continue;
                    } else {
                        lhs = self.arena.alloc_expr(
                            ExprNode::EnumVariant {
                                enum_name: causm_core::symbol::intern(&enum_str),
                                variant_name: variant_sym,
                                args: SliceRange::new(0, 0),
                            },
                            id_tok.span,
                        );
                        continue;
                    }
                }
            }

            let (l_bp, r_bp) = match Self::infix_binding_power(self.peek()) {
                Some(bps) => bps,
                None => break,
            };

            if l_bp < min_bp {
                break;
            }

            let op_tok = self.bump();
            if op_tok.kind == TokenKind::As {
                let typ_sym = match self.peek() {
                    TokenKind::Ident(s) => *s,
                    _ => causm_core::symbol::intern("int"),
                };
                if matches!(self.peek(), TokenKind::Ident(_)) {
                    self.bump();
                }
                lhs = self.arena.alloc_expr(
                    ExprNode::TypeCast {
                        expr: lhs,
                        target_type: typ_sym,
                    },
                    op_tok.span,
                );
                continue;
            }
            if op_tok.kind == TokenKind::Pipeline {
                let rhs = self.parse_expression(r_bp)?;
                match self.arena.expressions[rhs.0 as usize].clone() {
                    ExprNode::Call { routine, args } => {
                        let mut new_args = vec![lhs];
                        new_args.extend_from_slice(
                            &self.arena.expr_pool[args.as_range()],
                        );
                        let start = self.arena.expr_pool.len();
                        for a in new_args {
                            self.arena.expr_pool.push(a);
                        }
                        let end = self.arena.expr_pool.len();
                        lhs = self.arena.alloc_expr(
                            ExprNode::Call {
                                routine,
                                args: SliceRange::new(start, end),
                            },
                            op_tok.span,
                        );
                    }
                    ExprNode::Identifier(sym) => {
                        let routine = self.arena.alloc_expr(
                            ExprNode::Identifier(sym),
                            op_tok.span.clone(),
                        );
                        let start = self.arena.expr_pool.len();
                        self.arena.expr_pool.push(lhs);
                        let end = self.arena.expr_pool.len();
                        lhs = self.arena.alloc_expr(
                            ExprNode::Call {
                                routine,
                                args: SliceRange::new(start, end),
                            },
                            op_tok.span,
                        );
                    }
                    ExprNode::MethodCall {
                        target,
                        method,
                        args,
                    } => {
                        let mut new_args = vec![lhs];
                        new_args.extend_from_slice(
                            &self.arena.expr_pool[args.as_range()],
                        );
                        let start = self.arena.expr_pool.len();
                        for a in new_args {
                            self.arena.expr_pool.push(a);
                        }
                        let end = self.arena.expr_pool.len();
                        lhs = self.arena.alloc_expr(
                            ExprNode::MethodCall {
                                target,
                                method,
                                args: SliceRange::new(start, end),
                            },
                            op_tok.span,
                        );
                    }
                    _ => {
                        lhs = self.arena.alloc_expr(
                            ExprNode::Pipeline {
                                target: lhs,
                                stage: rhs,
                            },
                            op_tok.span,
                        );
                    }
                }
                continue;
            }

            let bin_op = match op_tok.kind {
                TokenKind::Plus => BinaryOperator::Add,
                TokenKind::Minus => BinaryOperator::Sub,
                TokenKind::Star => BinaryOperator::Mul,
                TokenKind::StarStar => BinaryOperator::Pow,
                TokenKind::Slash => BinaryOperator::Div,
                TokenKind::Percent => BinaryOperator::Rem,
                TokenKind::Shl => BinaryOperator::Shl,
                TokenKind::Shr => BinaryOperator::Shr,
                TokenKind::Lt => BinaryOperator::Lt,
                TokenKind::LtEq => BinaryOperator::Le,
                TokenKind::Gt => BinaryOperator::Gt,
                TokenKind::GtEq => BinaryOperator::Ge,
                TokenKind::EqEq => BinaryOperator::Eq,
                TokenKind::BangEq => BinaryOperator::Neq,
                TokenKind::Amp => BinaryOperator::BitwiseAnd,
                TokenKind::Pipe => BinaryOperator::BitwiseOr,
                TokenKind::Caret => BinaryOperator::BitwiseXor,
                TokenKind::AmpAmp => BinaryOperator::LogicalAnd,
                TokenKind::PipePipe => BinaryOperator::LogicalOr,
                TokenKind::NullCoalesce => BinaryOperator::NullCoalesce,
                _ => BinaryOperator::Add,
            };

            let rhs = self.parse_expression(r_bp)?;
            let span = Span { start: 0, end: 0 };
            lhs = self.arena.alloc_expr(
                ExprNode::BinaryOp {
                    left: lhs,
                    right: rhs,
                    op: bin_op,
                },
                span,
            );
        }

        Ok(lhs)
    }
}

pub fn to_ast_expression(arena: &AstArena, id: ExprId) -> causm_core::Expression {
    match &arena.expressions[id.0 as usize] {
        ExprNode::Literal(LiteralKind::Integer(val)) => {
            causm_core::Expression::Integer(*val)
        }
        ExprNode::Literal(LiteralKind::Duration(val)) => {
            causm_core::Expression::Integer(*val as i64)
        }
        ExprNode::Literal(LiteralKind::Float(ref s)) => {
            let val = s.parse::<f64>().unwrap_or(0.0);
            causm_core::Expression::Float(val.to_bits())
        }
        ExprNode::Literal(LiteralKind::String(ref s)) => {
            causm_core::Expression::Literal(s.clone())
        }
        ExprNode::Literal(LiteralKind::Boolean(b)) => {
            causm_core::Expression::Boolean(*b)
        }
        ExprNode::Literal(LiteralKind::Null) => causm_core::Expression::Null,
        ExprNode::Identifier(sym) => {
            let s = causm_core::symbol::resolve(*sym);
            if let Some((enum_name, variant_name)) = s.split_once("::") {
                causm_core::Expression::EnumVariant {
                    enum_name: enum_name.to_string(),
                    variant_name: variant_name.to_string(),
                    args: Vec::new(),
                }
            } else {
                causm_core::Expression::Identifier(s)
            }
        }
        ExprNode::BinaryOp { left, right, op } => causm_core::Expression::BinaryOp {
            left: Box::new(to_ast_expression(arena, *left)),
            op: *op,
            right: Box::new(to_ast_expression(arena, *right)),
        },
        ExprNode::UnaryOp { expr, op } => causm_core::Expression::UnaryOp {
            op: *op,
            expr: Box::new(to_ast_expression(arena, *expr)),
        },
        ExprNode::FieldAccess { target, field } => {
            causm_core::Expression::FieldAccess {
                target: Box::new(to_ast_expression(arena, *target)),
                field: causm_core::symbol::resolve(*field),
            }
        }
        ExprNode::MethodCall {
            target,
            method,
            args,
        } => {
            let parsed_args = arena.expr_pool[args.as_range()]
                .iter()
                .map(|&arg_id| to_ast_expression(arena, arg_id))
                .collect();
            let method_name = causm_core::symbol::resolve(*method);
            if let ExprNode::Identifier(tgt_sym) =
                &arena.expressions[target.0 as usize]
            {
                let tgt_str = causm_core::symbol::resolve(*tgt_sym);
                if tgt_str
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false)
                {
                    let qualified = format!("{}.{}", tgt_str, method_name);
                    return causm_core::Expression::Call {
                        routine: qualified,
                        args: parsed_args,
                    };
                }
            }
            causm_core::Expression::MethodCall {
                target: Box::new(to_ast_expression(arena, *target)),
                method: method_name,
                args: parsed_args,
                resolved_routine: std::cell::RefCell::new(None),
                resolved_budget: std::cell::RefCell::new(None),
            }
        }
        ExprNode::Call { routine, args } => {
            let parsed_args: Vec<causm_core::Expression> = arena.expr_pool
                [args.as_range()]
            .iter()
            .map(|&arg_id| to_ast_expression(arena, arg_id))
            .collect();
            let routine_name = match &arena.expressions[routine.0 as usize] {
                ExprNode::Identifier(sym) => causm_core::symbol::resolve(*sym),
                _ => "unknown".to_string(),
            };
            if routine_name == "len" && parsed_args.len() == 1 {
                causm_core::Expression::Len(Box::new(
                    parsed_args.into_iter().next().unwrap(),
                ))
            } else if routine_name == "str_bytes" && parsed_args.len() == 1 {
                causm_core::Expression::StrBytes(Box::new(
                    parsed_args.into_iter().next().unwrap(),
                ))
            } else if routine_name == "to_str" && parsed_args.len() == 1 {
                causm_core::Expression::ToStr(Box::new(
                    parsed_args.into_iter().next().unwrap(),
                ))
            } else if routine_name == "clone" && parsed_args.len() == 1 {
                if let causm_core::Expression::Identifier(ref id_str) =
                    parsed_args[0]
                {
                    causm_core::Expression::CloneOp(id_str.clone())
                } else {
                    causm_core::Expression::Call {
                        routine: routine_name,
                        args: parsed_args,
                    }
                }
            } else if routine_name == "capability" && !parsed_args.is_empty() {
                let cap_path = match &parsed_args[0] {
                    causm_core::Expression::Identifier(s) => s.clone(),
                    causm_core::Expression::FieldAccess { .. } => {
                        let mut curr = &parsed_args[0];
                        let mut parts = Vec::new();
                        while let causm_core::Expression::FieldAccess {
                            target,
                            field,
                        } = curr
                        {
                            parts.push(field.clone());
                            curr = target;
                        }
                        if let causm_core::Expression::Identifier(s) = curr {
                            parts.push(s.clone());
                        }
                        parts.reverse();
                        parts.join(".")
                    }
                    _ => "System.Log".into(),
                };
                causm_core::Expression::CapabilityCheck(causm_core::Capability {
                    path: cap_path,
                    parameters: std::collections::HashMap::new(),
                })
            } else if let Some((enum_name, variant_name)) =
                routine_name.split_once("::")
            {
                causm_core::Expression::EnumVariant {
                    enum_name: enum_name.to_string(),
                    variant_name: variant_name.to_string(),
                    args: parsed_args,
                }
            } else {
                causm_core::Expression::Call {
                    routine: routine_name,
                    args: parsed_args,
                }
            }
        }
        ExprNode::EnumVariant {
            enum_name,
            variant_name,
            args,
        } => {
            let parsed_args = arena.expr_pool[args.as_range()]
                .iter()
                .map(|&eid| to_ast_expression(arena, eid))
                .collect();
            causm_core::Expression::EnumVariant {
                enum_name: causm_core::symbol::resolve(*enum_name),
                variant_name: causm_core::symbol::resolve(*variant_name),
                args: parsed_args,
            }
        }
        ExprNode::TypeAssertion { target, cast_type } => {
            let t_str = causm_core::symbol::resolve(*cast_type);
            let typ = match t_str.as_str() {
                "int" => {
                    causm_core::TypeName::Builtin(causm_core::BuiltinType::Integer)
                }
                "float" => causm_core::TypeName::Builtin(causm_core::BuiltinType::Float),
                "string" => causm_core::TypeName::Builtin(causm_core::BuiltinType::String),
                "bool" => causm_core::TypeName::Builtin(causm_core::BuiltinType::Bool),
                _ => causm_core::TypeName::Custom(t_str),
            };
            causm_core::Expression::TypeAssertion {
                target: Box::new(to_ast_expression(arena, *target)),
                cast_type: typ,
            }
        }
        ExprNode::TypeCast { expr, target_type } => {
            let t_str = causm_core::symbol::resolve(*target_type);
            let typ = match t_str.as_str() {
                "int" => {
                    causm_core::TypeName::Builtin(causm_core::BuiltinType::Integer)
                }
                "float" => {
                    causm_core::TypeName::Builtin(causm_core::BuiltinType::Float)
                }
                "bool" => {
                    causm_core::TypeName::Builtin(causm_core::BuiltinType::Bool)
                }
                "string" => {
                    causm_core::TypeName::Builtin(causm_core::BuiltinType::String)
                }
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
                _ => causm_core::TypeName::Custom(t_str),
            };
            causm_core::Expression::TypeCast {
                expr: Box::new(to_ast_expression(arena, *expr)),
                target_type: typ,
            }
        }
        ExprNode::Try(inner) => causm_core::Expression::TryUnwrap(Box::new(
            to_ast_expression(arena, *inner),
        )),
        ExprNode::ArrayLit(elems) => {
            let parsed_elems = arena.expr_pool[elems.as_range()]
                .iter()
                .map(|&elem_id| to_ast_expression(arena, elem_id))
                .collect();
            causm_core::Expression::ArrayLiteral(parsed_elems)
        }
        ExprNode::IndexAccess { target, index } => {
            causm_core::Expression::IndexAccess {
                target: Box::new(to_ast_expression(arena, *target)),
                index: Box::new(to_ast_expression(arena, *index)),
            }
        }
        ExprNode::ArraySlice { target, start, end, inclusive } => {
            causm_core::Expression::ArraySlice {
                target: Box::new(to_ast_expression(arena, *target)),
                start: start.map(|s| Box::new(to_ast_expression(arena, s))),
                end: end.map(|e| Box::new(to_ast_expression(arena, e))),
                inclusive: *inclusive,
            }
        }
        ExprNode::FString(parts) => {
            let parsed_parts = arena.fstring_parts_pool[parts.as_range()]
                .iter()
                .map(|p| match p {
                    causm_core::arena::FStringPartNode::Text(t) => {
                        causm_core::FStringPart::Text(t.clone())
                    }
                    causm_core::arena::FStringPartNode::Expr(eid) => {
                        causm_core::FStringPart::Expr(to_ast_expression(arena, *eid))
                    }
                })
                .collect();
            causm_core::Expression::FString(parsed_parts)
        }
        ExprNode::Ref(inner) => {
            causm_core::Expression::RefOp(Box::new(to_ast_expression(arena, *inner)))
        }
        ExprNode::StructLit { type_sym, fields } => {
            let mut field_map = std::collections::HashMap::new();
            for f in &arena.field_assigns_pool[fields.as_range()] {
                let expr = to_ast_expression(arena, f.expr);
                field_map.insert(causm_core::symbol::resolve(f.field), expr);
            }
            if let Some(sym) = type_sym {
                if causm_core::symbol::resolve(*sym) == "topology" {
                    return causm_core::Expression::TopologyLit(field_map);
                }
            }
            let t_name = type_sym.map(causm_core::symbol::resolve);
            causm_core::Expression::StructLit(
                std::cell::RefCell::new(t_name),
                field_map,
            )
        }
        ExprNode::ArrayRepeat { value, count } => {
            causm_core::Expression::ArrayRepeat {
                value: Box::new(to_ast_expression(arena, *value)),
                count: Box::new(to_ast_expression(arena, *count)),
            }
        }
        ExprNode::Tuple(elems) => {
            let parsed_elems = arena.expr_pool[elems.as_range()]
                .iter()
                .map(|&elem_id| to_ast_expression(arena, elem_id))
                .collect();
            causm_core::Expression::Tuple(parsed_elems)
        }
        ExprNode::Syscall {
            target,
            args,
            duration_ms,
        } => {
            let parsed_args = arena.expr_pool[args.as_range()]
                .iter()
                .map(|&arg_id| to_ast_expression(arena, arg_id))
                .collect();
            let tgt_str = causm_core::symbol::resolve(*target);
            let s_target = if let Ok(n) = tgt_str.parse::<i64>() {
                causm_core::SyscallTarget::Number(n)
            } else {
                causm_core::SyscallTarget::Symbol(tgt_str)
            };
            causm_core::Expression::Syscall {
                target: s_target,
                args: parsed_args,
                duration_ms: *duration_ms,
            }
        }
        ExprNode::ChanRecv(sym) => {
            causm_core::Expression::ChannelReceive(causm_core::symbol::resolve(*sym))
        }
        ExprNode::ArenaIntrospect(sym) => {
            let s = causm_core::symbol::resolve(*sym);
            let kind = match s.as_str() {
                "remaining" => causm_core::ArenaIntrospect::Remaining,
                "used_bytes" => causm_core::ArenaIntrospect::UsedBytes,
                "capacity" => causm_core::ArenaIntrospect::Capacity,
                _ => causm_core::ArenaIntrospect::Remaining,
            };
            causm_core::Expression::ArenaIntrospect(kind)
        }
        ExprNode::Defer {
            capability,
            duration_ms,
        } => causm_core::Expression::Deferred {
            capability: causm_core::symbol::resolve(*capability),
            params: std::collections::HashMap::new(),
            deadline_ms: *duration_ms,
        },
        ExprNode::Match { target, arms } => {
            let match_arms = arena.match_arms_pool[arms.as_range()]
                .iter()
                .map(|arm| {
                    let pat_str = causm_core::symbol::resolve(arm.pattern);
                    let body_expr = if arm.body.len() > 0 {
                        let sid = arena.stmt_pool[arm.body.start as usize];
                        match &arena.statements[sid.0 as usize] {
                            causm_core::arena::StmtNode::Expr(eid) => {
                                to_ast_expression(arena, *eid)
                            }
                            _ => causm_core::Expression::Null,
                        }
                    } else {
                        causm_core::Expression::Null
                    };
                    causm_core::MatchExprArm {
                        pattern: parse_pattern_from_str(&pat_str),
                        guard: None,
                        body: body_expr,
                    }
                })
                .collect();
            causm_core::Expression::Match {
                target: Box::new(to_ast_expression(arena, *target)),
                arms: match_arms,
            }
        }
        ExprNode::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let cond_expr = to_ast_expression(arena, *cond);
            let then_expr = match &arena.statements[then_branch.0 as usize] {
                causm_core::arena::StmtNode::Expr(eid) => {
                    to_ast_expression(arena, *eid)
                }
                _ => causm_core::Expression::Null,
            };
            let else_expr = else_branch
                .as_ref()
                .map(|sid| match &arena.statements[sid.0 as usize] {
                    causm_core::arena::StmtNode::Expr(eid) => {
                        to_ast_expression(arena, *eid)
                    }
                    _ => causm_core::Expression::Null,
                })
                .unwrap_or(causm_core::Expression::Null);
            causm_core::Expression::If {
                condition: Box::new(cond_expr),
                then_branch: Box::new(then_expr),
                else_branch: Box::new(else_expr),
            }
        }
        _ => causm_core::Expression::Literal("void".to_string()),
    }
}

pub(crate) fn parse_pattern_from_str(pat_str: &str) -> causm_core::Pattern {
    let pat_str = pat_str.trim();
    if pat_str == "_" || pat_str.is_empty() {
        causm_core::Pattern::Wildcard
    } else if pat_str == "true" {
        causm_core::Pattern::Literal(causm_core::Expression::Boolean(true))
    } else if pat_str == "false" {
        causm_core::Pattern::Literal(causm_core::Expression::Boolean(false))
    } else if let Ok(val) = pat_str.parse::<i64>() {
        causm_core::Pattern::Literal(causm_core::Expression::Integer(val))
    } else if pat_str.starts_with('"')
        && pat_str.ends_with('"')
        && pat_str.len() >= 2
    {
        causm_core::Pattern::Literal(causm_core::Expression::Literal(
            pat_str[1..pat_str.len() - 1].to_string(),
        ))
    } else if pat_str.starts_with('(')
        && pat_str.ends_with(')')
        && !pat_str.contains("::")
    {
        let inside = pat_str[1..pat_str.len() - 1].trim();
        let mut pats = Vec::new();
        if !inside.is_empty() {
            let mut depth = 0;
            let mut current = String::new();
            for ch in inside.chars() {
                match ch {
                    '(' => {
                        depth += 1;
                        current.push(ch);
                    }
                    ')' => {
                        depth -= 1;
                        current.push(ch);
                    }
                    ',' if depth == 0 => {
                        pats.push(parse_pattern_from_str(current.trim()));
                        current.clear();
                    }
                    _ => current.push(ch),
                }
            }
            if !current.trim().is_empty() {
                pats.push(parse_pattern_from_str(current.trim()));
            }
        }
        causm_core::Pattern::Tuple(pats)
    } else if pat_str.contains("::") || pat_str.contains('(') {
        let (enum_name, rest) = if let Some(idx) = pat_str.find("::") {
            (Some(pat_str[..idx].trim().to_string()), &pat_str[idx + 2..])
        } else {
            (None, pat_str)
        };
        let (variant_name, args) = if let Some(open) = rest.find('(') {
            let close = rest.rfind(')').unwrap_or(rest.len());
            let v_name = rest[..open].trim().to_string();
            let inside = rest[open + 1..close].trim();
            let mut arg_pats = Vec::new();
            if !inside.is_empty() {
                for arg_s in inside.split(',') {
                    arg_pats.push(parse_pattern_from_str(arg_s.trim()));
                }
            }
            (v_name, arg_pats)
        } else {
            (rest.trim().to_string(), Vec::new())
        };
        causm_core::Pattern::EnumVariant {
            enum_name,
            variant_name,
            args,
        }
    } else {
        causm_core::Pattern::Identifier(pat_str.to_string())
    }
}
