//! Expression parser. Precedence, loosest first: OR, AND, NOT, comparisons, `+ -`, `* /`,
//! unary minus, `**`. Binary operators of one level associate to the left, `**` included.

use crate::ast::{BinaryOp, Expr, UnaryOp};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser, is_reserved};

const OPERAND: [&str; 4] = ["a value", "a variable", "a function", "'('"];

/// Binary levels, loosest first. NOT sits between levels 1 and 2, unary minus below level 4.
fn binary_op(kind: &TokenKind, level: u8) -> Option<BinaryOp> {
    let op = match kind {
        TokenKind::Punct(p) => match p {
            Punct::Or => BinaryOp::Or,
            Punct::And => BinaryOp::And,
            Punct::Eq => BinaryOp::Eq,
            Punct::Ne => BinaryOp::Ne,
            Punct::Lt => BinaryOp::Lt,
            Punct::Le => BinaryOp::Le,
            Punct::Gt => BinaryOp::Gt,
            Punct::Ge => BinaryOp::Ge,
            Punct::Plus => BinaryOp::Add,
            Punct::Minus => BinaryOp::Sub,
            Punct::Star => BinaryOp::Mul,
            Punct::Slash => BinaryOp::Div,
            _ => return None,
        },
        TokenKind::Word(w) => match w.to_ascii_uppercase().as_str() {
            "OR" => BinaryOp::Or,
            "AND" => BinaryOp::And,
            "EQ" => BinaryOp::Eq,
            "NE" => BinaryOp::Ne,
            "LT" => BinaryOp::Lt,
            "LE" => BinaryOp::Le,
            "GT" => BinaryOp::Gt,
            "GE" => BinaryOp::Ge,
            _ => return None,
        },
        _ => return None,
    };
    let op_level = match op {
        BinaryOp::Or => 0,
        BinaryOp::And => 1,
        BinaryOp::Add | BinaryOp::Sub => 3,
        BinaryOp::Mul | BinaryOp::Div => 4,
        BinaryOp::Pow => return None,
        _ => 2,
    };
    (op_level == level).then_some(op)
}

impl Parser<'_> {
    pub fn expr(&mut self) -> PResult<Expr> {
        self.nested(|p| p.binary(0))
    }

    fn binary(&mut self, level: u8) -> PResult<Expr> {
        let mut left = self.operand(level)?;
        while let Some(op) = self.peek().and_then(|kind| binary_op(kind, level)) {
            self.count_operator()?;
            self.pos += 1;
            let right = self.operand(level)?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn operand(&mut self, level: u8) -> PResult<Expr> {
        match level {
            1 => self.not(),
            4 => self.unary(),
            _ => self.binary(level + 1),
        }
    }

    fn not(&mut self) -> PResult<Expr> {
        if self.eat(Punct::Not) || self.eat_word("NOT") {
            let operand = Box::new(self.nested(Self::not)?);
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                operand,
            });
        }
        self.binary(2)
    }

    fn unary(&mut self) -> PResult<Expr> {
        if self.eat(Punct::Minus) {
            let operand = Box::new(self.nested(Self::unary)?);
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                operand,
            });
        }
        if self.eat(Punct::Plus) {
            return self.nested(Self::unary);
        }
        let mut base = self.primary()?;
        while self.eat(Punct::Pow) {
            self.count_operator()?;
            base = Expr::Binary {
                op: BinaryOp::Pow,
                left: Box::new(base),
                right: Box::new(self.exponent()?),
            };
        }
        Ok(base)
    }

    /// The right side of `**` may carry its own sign, as in `2 ** -1`.
    fn exponent(&mut self) -> PResult<Expr> {
        if self.eat(Punct::Minus) {
            let operand = Box::new(self.nested(Self::exponent)?);
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                operand,
            });
        }
        self.primary()
    }

    fn primary(&mut self) -> PResult<Expr> {
        match self.peek() {
            Some(TokenKind::Number(n)) => {
                self.pos += 1;
                Ok(Expr::Number(*n))
            }
            Some(TokenKind::Str(s)) => {
                self.pos += 1;
                Ok(Expr::String(s.clone()))
            }
            Some(TokenKind::Punct(Punct::LParen)) => {
                self.pos += 1;
                let inner = self.expr()?;
                self.expect(Punct::RParen)?;
                Ok(inner)
            }
            Some(TokenKind::Word(w)) if !is_reserved(w) => {
                self.pos += 1;
                if !self.eat(Punct::LParen) {
                    return Ok(Expr::Variable(w.clone()));
                }
                let mut args = Vec::new();
                if !self.eat(Punct::RParen) {
                    loop {
                        args.push(self.argument()?);
                        if !self.eat(Punct::Comma) {
                            break;
                        }
                    }
                    if !self.eat(Punct::RParen) {
                        return Err(self.unexpected(&["','", "')'", "an operator"]));
                    }
                }
                Ok(Expr::Call {
                    name: w.clone(),
                    args,
                })
            }
            _ => Err(self.unexpected(&OPERAND)),
        }
    }

    fn argument(&mut self) -> PResult<Expr> {
        if let (
            Some(TokenKind::Word(first)),
            Some(TokenKind::Word(to)),
            Some(TokenKind::Word(last)),
        ) = (self.peek(), self.peek_at(1), self.peek_at(2))
            && to.eq_ignore_ascii_case("TO")
            && !is_reserved(first)
            && !is_reserved(last)
        {
            self.pos += 3;
            return Ok(Expr::Range(first.clone(), last.clone()));
        }
        self.expr()
    }
}
