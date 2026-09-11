pub mod lower;
pub mod statements;

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
            TokenKind::At => self.parse_at_stmt(),
            TokenKind::Let => self.parse_let_stmt(),
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
            TokenKind::Pub | TokenKind::Routine => self.parse_pub_stmt(),
            TokenKind::Import => self.parse_import_stmt(),
            TokenKind::Require => self.parse_require_stmt(),
            TokenKind::Foreign => self.parse_foreign_stmt(),
            TokenKind::On => self.parse_on_stmt(),
            TokenKind::Isolate | TokenKind::Actor => self.parse_isolate_stmt(),
            TokenKind::Enable => self.parse_enable_stmt(),
            TokenKind::Split => self.parse_split_stmt(),
            TokenKind::Merge => self.parse_merge_stmt(),
            TokenKind::Send => self.parse_send_stmt(),
            TokenKind::If => self.parse_if_stmt(),
            TokenKind::Print => self.parse_print_stmt(),
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
            TokenKind::OnDecay => self.parse_ondecay_stmt(),
            TokenKind::Using => self.parse_using_stmt(),
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
            TokenKind::Loop => self.parse_loop_stmt(),
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
                Ok(Some(id))
            }
            TokenKind::Interface => self.parse_interface_stmt(),
            TokenKind::Type | TokenKind::Struct => self.parse_type_stmt(),
            TokenKind::Enum => self.parse_enum_stmt(),
            TokenKind::Macro => self.parse_macro_stmt(),
            TokenKind::From => self.parse_from_stmt(),
            TokenKind::Match => self.parse_match_stmt(),
            TokenKind::For => self.parse_for_stmt(),
            TokenKind::Lease => self.parse_lease_stmt(),
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
            TokenKind::State => self.parse_state_stmt(),
            TokenKind::Policy => self.parse_policy_stmt(),
            TokenKind::Select => self.parse_select_stmt(),
            TokenKind::Entangle => self.parse_entangle_stmt(),
            TokenKind::Speculate => self.parse_speculate_stmt(),
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
            TokenKind::Ident(sym)
                if sym.0 == causm_core::symbol::intern("slice").0 =>
            {
                let s_tok = self.bump();
                let expr = self.parse_expression(0)?;
                let id = self.arena.alloc_stmt(StmtNode::Slice(expr), s_tok.span);
                Ok(Some(id))
            }
            TokenKind::AssertTime => self.parse_asserttime_stmt(),
            _ => self.parse_default_stmt(),
        }
    }

    pub fn parse_resolution_strategy(
        &mut self,
    ) -> Result<causm_core::ResolutionStrategy, String> {
        let is_topology = match self.peek() {
            TokenKind::Ident(s) => {
                let name = causm_core::symbol::resolve(*s);
                name == "topology_union" || name == "topology_intersect"
            }
            _ => false,
        };

        if is_topology {
            let is_union = match self.peek() {
                TokenKind::Ident(s) => {
                    causm_core::symbol::resolve(*s) == "topology_union"
                }
                _ => true,
            };
            self.bump(); // topology_union or topology_intersect
            if self.peek() == &TokenKind::LBrace {
                self.bump();
            }
            let mut rules = std::collections::HashMap::new();
            let mut default = Box::new(causm_core::ResolutionStrategy::Decay);
            let mut on_invalid = None;

            while self.peek() != &TokenKind::RBrace && self.peek() != &TokenKind::Eof
            {
                if matches!(self.peek(), TokenKind::Ident(s) if causm_core::symbol::resolve(*s) == "on_invalid")
                {
                    self.bump();
                    if self.peek() == &TokenKind::Colon {
                        self.bump();
                    }
                    if matches!(self.peek(), TokenKind::Ident(s) if causm_core::symbol::resolve(*s) == "rewind")
                    {
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
                    on_invalid =
                        Some(causm_core::CausalReversion { branch, anchor });
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
                    if self.peek() == &TokenKind::Colon
                        || self.peek() == &TokenKind::Eq
                    {
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
                Ok(causm_core::ResolutionStrategy::Custom(format!(
                    "{:?}",
                    tok.kind
                )))
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
            "struct" => {
                causm_core::TypeName::Builtin(causm_core::BuiltinType::Struct)
            }
            "array" => causm_core::TypeName::Builtin(causm_core::BuiltinType::Array),
            _ => causm_core::TypeName::Custom(s.to_string()),
        }
    }
}
