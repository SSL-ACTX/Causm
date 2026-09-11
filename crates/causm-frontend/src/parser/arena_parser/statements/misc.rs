use crate::parser::arena_parser::ArenaParser;
use crate::parser::lexer::TokenKind;
use causm_core::arena::{SliceRange, StmtId, StmtNode};

impl<'a> ArenaParser<'a> {
    pub fn parse_print_stmt(&mut self) -> Result<Option<StmtId>, String> {
        let p_tok = self.bump();
        let start = self.arena.expr_pool.len();
        if self.peek() == &TokenKind::LParen {
            self.bump();
            while self.peek() != &TokenKind::RParen && self.peek() != &TokenKind::Eof
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
        let id = self
            .arena
            .alloc_stmt(StmtNode::Print(SliceRange::new(start, end)), p_tok.span);
        Ok(Some(id))
    }

    pub fn parse_default_stmt(&mut self) -> Result<Option<StmtId>, String> {
        let start_span = self.current.span.clone();
        let next_tok = self.stream.peek_token();
        if let TokenKind::Ident(s) = self.peek() {
            if causm_core::symbol::resolve(*s) == "state"
                && matches!(next_tok.kind, TokenKind::Ident(_))
            {
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
            let is_assign = matches!(
                next_tok.kind,
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
                    | TokenKind::CaretEq
            );

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
                            TokenKind::PlusEq => causm_core::BinaryOperator::Add,
                            TokenKind::MinusEq => causm_core::BinaryOperator::Sub,
                            TokenKind::StarEq => causm_core::BinaryOperator::Mul,
                            TokenKind::SlashEq => causm_core::BinaryOperator::Div,
                            TokenKind::PercentEq => causm_core::BinaryOperator::Rem,
                            TokenKind::ShlEq => causm_core::BinaryOperator::Shl,
                            TokenKind::ShrEq => causm_core::BinaryOperator::Shr,
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
                _ => self
                    .arena
                    .alloc_stmt(StmtNode::Expr(val_expr_id), start_span),
            };
            return Ok(Some(id));
        }
        let id = self.arena.alloc_stmt(StmtNode::Expr(expr_id), start_span);
        Ok(Some(id))
    }
}
