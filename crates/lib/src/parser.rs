//! Pratt parsing

use crate::tokens::{Lexer, ParsingError, TokenType};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
enum BindingPower {
    MinPower,
    TernL, TernR,
    LogOrL, LogOrR,
    LogAndL, LogAndR,
    BitOrL, BitOrR,
    BitXorL, BitXorR,
    BitAndL, BitAndR,
    EqL, EqR,
    CmpL, CmpR,
    ShiftL, ShiftR,
    SumL, SumR,
    ProdL, ProdR,
    Unary
}

impl TokenType {
    const fn infix_power(&self) -> Option<(BindingPower, BindingPower)> {
        use BindingPower::*;
        Some(match self {
            TokenType::Question => (TernL, TernR),
            TokenType::Bar => (BitOrL, BitOrR),
            TokenType::Carat => (BitXorL, BitXorR),
            TokenType::Ampersand => (BitAndL, BitAndR),
            TokenType::DoubleAmpersand => (LogAndL, LogAndR),
            TokenType::DoubleBar => (LogOrL, LogOrR),
            TokenType::DoubleEqual | TokenType::BangEqual => (EqL, EqR),
            TokenType::Less | TokenType::LessEqual | TokenType::Greater | TokenType::GreaterEqual => (CmpL, CmpR),
            TokenType::LeftShift | TokenType::RightShift => (ShiftL, ShiftR),
            TokenType::Plus | TokenType::Minus => (SumL, SumR),
            TokenType::Asterisk | TokenType::Slash | TokenType::Percent => (ProdL, ProdR),
            _ => return None
        })
    }
    const fn is_unary(&self) -> bool {
        matches!(self, TokenType::Tilde | TokenType::Bang | TokenType::Minus | TokenType::Plus)
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum BinaryOperandType {
    Add, Subtract, Multiply, Divide, Modulus, Eq, NotEq,
    Less, LessEq, Greater, GreaterEq, LeftShift, RightShift,
    BinaryAnd, BinaryXor, BinaryOr,
    Ternary(Box<Expression>)
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct BinaryOperand {
    pub ty: BinaryOperandType,
    pub index: usize
}

impl std::fmt::Display for BinaryOperand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.ty.fmt(f)
    }
}

impl std::fmt::Display for BinaryOperandType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryOperandType::Add => write!(f, "+"),
            BinaryOperandType::Subtract => write!(f, "-"),
            BinaryOperandType::Multiply => write!(f, "*"),
            BinaryOperandType::Divide => write!(f, "/"),
            BinaryOperandType::Modulus => write!(f, "%"),
            BinaryOperandType::Eq => write!(f, "=="),
            BinaryOperandType::NotEq => write!(f, "!="),
            BinaryOperandType::Less => write!(f, "<"),
            BinaryOperandType::LessEq => write!(f, "<="),
            BinaryOperandType::Greater => write!(f, ">"),
            BinaryOperandType::GreaterEq => write!(f, ">="),
            BinaryOperandType::LeftShift => write!(f, "<<"),
            BinaryOperandType::RightShift => write!(f, ">>"),
            BinaryOperandType::BinaryAnd => write!(f, "&"),
            BinaryOperandType::BinaryXor => write!(f, "^"),
            BinaryOperandType::BinaryOr => write!(f, "|"),
            BinaryOperandType::Ternary(expression) => 
                write!(f, "?: {expression}"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnaryOperandType { Not, Negate, Invert, Identity }

impl std::fmt::Display for UnaryOperandType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            UnaryOperandType::Not => "!",
            UnaryOperandType::Negate => "-",
            UnaryOperandType::Invert => "~",
            UnaryOperandType::Identity => "+",
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnaryOperand {
    pub ty: UnaryOperandType,
    pub index: usize
}

impl std::fmt::Display for UnaryOperand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.ty.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Expression {
    Binary {
        lhs: Box<Expression>,
        operand: BinaryOperand,
        rhs: Box<Expression>
    },
    Unary {
        prefix: UnaryOperand,
        value: Box<Expression>
    },
    Integer { value: u32, index: usize },
    Float { value: f32, index: usize },
    Double { value: f64, index: usize },
    T { index: usize }
}

impl std::fmt::Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expression::Binary { lhs, operand: BinaryOperand { ty: BinaryOperandType::Ternary(expr), ..}, rhs } => 
                write!(f, "(?: {lhs} {expr} {rhs})"),
            Expression::Binary { lhs, operand, rhs } => 
                write!(f, "({operand} {lhs} {rhs})"),
            Expression::Unary { prefix, value } =>
                write!(f, "({prefix} {value})"),
            Expression::Integer { value, .. } => write!(f, "{value:?}"),
            Expression::Float { value, .. } => write!(f, "{value:?}f"),
            Expression::Double { value, .. } => write!(f, "{value:?}"),
            Expression::T  { .. } => write!(f, "t"),
        }
    }
}

pub const STACK_REDZONE: usize = 32 * 1024;
pub const STACK_NEWSIZE: usize = 1024 * 1024;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ParsingMode {
    Root, Ternary, Parentheses,
}

pub fn parse<'str>(toks: &mut Lexer<'str>) -> Result<Expression, ParsingError> {
    parse_expr(toks, BindingPower::MinPower, ParsingMode::Root)
}

fn parse_expr<'str>(toks: &mut Lexer<'str>, min_power: BindingPower, parsing_mode: ParsingMode) -> Result<Expression, ParsingError> {
    let tok = toks.next()?;
    let mut lhs = match tok.ty {
        TokenType::Integer(value) => Expression::Integer { value, index: tok.index },
        TokenType::Float(value) => Expression::Float { value: value.into(), index: tok.index },
        TokenType::Double(value) => Expression::Double { value: value.into(), index: tok.index },
        TokenType::T => Expression::T { index: tok.index },
        TokenType::OpenParen => {
            let expr = stacker::maybe_grow(STACK_REDZONE, STACK_NEWSIZE, || parse_expr(toks, BindingPower::MinPower, ParsingMode::Parentheses))?;
            let t = toks.next()?;
            if t.ty != TokenType::CloseParen {
                return Err(ParsingError::expected_close(t));
            }
            expr
        }
        t if t.is_unary() => {
            let operand = match t {
                TokenType::Plus => UnaryOperand { ty: UnaryOperandType::Identity, index: tok.index },
                TokenType::Minus => UnaryOperand { ty: UnaryOperandType::Negate, index: tok.index },
                TokenType::Tilde => UnaryOperand { ty: UnaryOperandType::Invert, index: tok.index },
                TokenType::Bang => UnaryOperand { ty: UnaryOperandType::Not, index: tok.index },
                _ => unreachable!()
            };
            Expression::Unary {
                prefix: operand,
                value: Box::new(
                    stacker::maybe_grow(STACK_REDZONE, STACK_NEWSIZE, || parse_expr(toks, BindingPower::Unary, parsing_mode))?
                )
            }
        }
        _ => return Err(ParsingError::expected_atom(tok))
    };

    loop {
        let operand = toks.peek()?;
        if operand.ty == TokenType::Eof { break }
        if operand.ty == TokenType::Colon && parsing_mode == ParsingMode::Ternary { break }
        if operand.ty == TokenType::CloseParen && parsing_mode == ParsingMode::Parentheses { break }
        let (lbp, rbp) = operand.ty.infix_power()
            .ok_or(ParsingError::expected_binary(operand))?;   
        if lbp < min_power { break }
        toks.next()?;

        let opr = match operand.ty {
            TokenType::Plus => BinaryOperandType::Add,
            TokenType::Minus => BinaryOperandType::Subtract,
            TokenType::Asterisk => BinaryOperandType::Multiply,
            TokenType::Slash => BinaryOperandType::Divide,
            TokenType::Percent => BinaryOperandType::Modulus,
            TokenType::Ampersand => BinaryOperandType::BinaryAnd,
            TokenType::DoubleAmpersand => {
                let idx = operand.index;
                let rhs = stacker::maybe_grow(STACK_REDZONE, STACK_NEWSIZE, || parse_expr(toks, rbp, parsing_mode))?;
                let opr = BinaryOperand { ty: BinaryOperandType::Ternary(Box::new(rhs)), index: idx };
                lhs = Expression::Binary { lhs: Box::new(lhs), operand: opr, rhs: Box::new(Expression::Integer { value: 0, index: idx }) };
                continue;
            },
            TokenType::Bar => BinaryOperandType::BinaryOr,
            TokenType::DoubleBar => BinaryOperandType::Ternary(
                Box::new(Expression::Integer { value: 1, index: operand.index })
            ),
            TokenType::Carat => BinaryOperandType::BinaryXor,
            TokenType::Less => BinaryOperandType::Less,
            TokenType::LessEqual => BinaryOperandType::LessEq,
            TokenType::LeftShift => BinaryOperandType::LeftShift,
            TokenType::Greater => BinaryOperandType::Greater,
            TokenType::GreaterEqual => BinaryOperandType::GreaterEq,
            TokenType::RightShift => BinaryOperandType::RightShift,
            TokenType::DoubleEqual => BinaryOperandType::Eq,
            TokenType::BangEqual => BinaryOperandType::NotEq,
            TokenType::Question => {
                let expr = stacker::maybe_grow(STACK_REDZONE, STACK_NEWSIZE, || parse_expr(toks, BindingPower::MinPower, ParsingMode::Ternary))?;
                let t = toks.next()?;
                if t.ty != TokenType::Colon {
                    return Err(ParsingError::expected_close(t));
                }
                BinaryOperandType::Ternary(Box::new(expr))
            },
            _ => return Err(ParsingError::expected_binary(operand))
        };

        let opr = BinaryOperand { ty: opr, index: operand.index };

        let rhs = stacker::maybe_grow(STACK_REDZONE, STACK_NEWSIZE, || parse_expr(toks, rbp, parsing_mode))?;
        lhs = Expression::Binary { lhs: Box::new(lhs), operand: opr, rhs: Box::new(rhs) };
    }

    Ok(lhs)
}