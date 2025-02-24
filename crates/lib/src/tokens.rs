use std::str::FromStr;
use ordered_float::NotNan;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum TokenType {
    Integer(u32),
    Float(NotNan<f32>),
    Double(NotNan<f64>),
    OpenParen,
    CloseParen,
    T,
    LeftShift,
    RightShift,
    Plus,
    Minus,
    Asterisk,
    Slash,
    Percent,
    DoubleAmpersand,
    DoubleBar,
    Ampersand,
    Bar,
    Tilde,
    Question,
    Colon,
    Carat,
    LessEqual,
    GreaterEqual,
    Less,
    Greater,
    DoubleEqual,
    BangEqual,
    Bang,
    Eof,
    Unknown(char)
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct Token {
    pub ty: TokenType,
    pub index: usize
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.ty.fmt(f)
    }
}

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenType::Integer(n) => write!(f, "{n}"),
            TokenType::Float(not_nan) => write!(f, "{not_nan}f"),
            TokenType::Double(not_nan) => write!(f, "{not_nan}"),
            TokenType::OpenParen => write!(f, "("),
            TokenType::CloseParen => write!(f, ")"),
            TokenType::T => write!(f, "t"),
            TokenType::LeftShift => write!(f, "<<"),
            TokenType::RightShift => write!(f, ">>"),
            TokenType::Plus => write!(f, "+"),
            TokenType::Minus => write!(f, "-"),
            TokenType::Asterisk => write!(f, "*"),
            TokenType::Slash => write!(f, "/"),
            TokenType::Percent => write!(f, "%"),
            TokenType::DoubleAmpersand => write!(f, "&&"),
            TokenType::DoubleBar => write!(f, "||"),
            TokenType::Ampersand => write!(f, "&"),
            TokenType::Bar => write!(f, "|"),
            TokenType::Tilde => write!(f, "~"),
            TokenType::Question => write!(f, "?"),
            TokenType::Colon => write!(f, ":"),
            TokenType::Carat => write!(f, "^"),
            TokenType::LessEqual => write!(f, "<="),
            TokenType::GreaterEqual => write!(f, ">="),
            TokenType::Less => write!(f, "<"),
            TokenType::Greater => write!(f, ">"),
            TokenType::DoubleEqual => write!(f, "=="),
            TokenType::BangEqual => write!(f, "!="),
            TokenType::Bang => write!(f, "!"),
            TokenType::Eof => write!(f, "<eof>"),
            TokenType::Unknown(chr) => write!(f, "{chr}")
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExpectedTokenType {
    Binary, Atom, CloseParen
}

impl std::fmt::Display for ExpectedTokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpectedTokenType::Binary => write!(f, "binary operator"),
            ExpectedTokenType::Atom => write!(f, "unary operator, number, t, or parenthesized expression"),
            ExpectedTokenType::CloseParen => write!(f, "closing parenthesis"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ParsingErrorType {
    UnopenedParenthesis,
    UnclosedParenthesis(usize),
    BadToken(ExpectedTokenType, TokenType),
    BrokenEqual,
    InvalidLiteral
}

impl std::fmt::Display for ParsingErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParsingErrorType::UnopenedParenthesis => write!(f, "unopened parenthesis"),
            ParsingErrorType::UnclosedParenthesis(start) => write!(f, "unclosed parenthesis; opened at index {start}"),
            ParsingErrorType::BrokenEqual => write!(f, "broken double equal operator"),
            ParsingErrorType::BadToken(ty, tok) => write!(f, "invalid token `{tok}`; expected a {ty}"),
            ParsingErrorType::InvalidLiteral => write!(f, "invalid literal"),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ParsingError {
    pub ty: ParsingErrorType,
    pub index: usize
}

impl ParsingError {
    #[inline] pub const fn invalid_literal(index: usize) -> Self {
        Self {
            ty: ParsingErrorType::InvalidLiteral,
            index
        }
    }
    #[inline] pub const fn expected_close(tok: Token) -> Self {
        Self {
            ty: ParsingErrorType::BadToken(ExpectedTokenType::CloseParen, tok.ty),
            index: tok.index
        }
    }
    #[inline] pub const fn expected_binary(tok: Token) -> Self {
        Self {
            ty: ParsingErrorType::BadToken(ExpectedTokenType::Binary, tok.ty),
            index: tok.index
        }
    }
    #[inline] pub const fn expected_atom(tok: Token) -> Self {
        Self {
            ty: ParsingErrorType::BadToken(ExpectedTokenType::Atom, tok.ty),
            index: tok.index
        }
    }
    #[inline] pub const fn unclosed_paren(index: usize, start: usize) -> Self {
        Self {
            ty: ParsingErrorType::UnclosedParenthesis(start),
            index
        }
    }
    #[inline] pub const fn broken_equal(index: usize) -> Self {
        Self {
            ty: ParsingErrorType::BrokenEqual,
            index
        }
    }
}

impl std::fmt::Display for ParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error at index {}: {}", self.index, self.ty)
    }
}

impl std::error::Error for ParsingError {}

// Recursive descent parser, but only for trees and tokens
// As simple as possible, the actual binary operations come from Pratt
pub fn parse_tokens(string: &str) -> Lexer<'_> {
    Lexer { buf: string, index: 0 }
}

#[derive(Debug, Copy, Clone)]
pub struct Lexer<'str> {
    buf: &'str str,
    pub index: usize
}

impl<'str> Lexer<'str> {
    pub fn next(&mut self) -> Result<Token, ParsingError> {
        // Here we can abuse the fact that we don't have any non-ASCII characters
        let mut iter = self.buf[self.index..].chars();
        let Some(mut chr) = iter.next() else { return Ok(Token{ ty: TokenType::Eof, index: self.index }) };
        while chr.is_ascii_whitespace() {
            self.index += 1;
            let Some(next) = iter.next() else { return Ok(Token{ ty: TokenType::Eof, index: self.index }) };
            chr = next;
        }
        let ty = match chr {
            't' => TokenType::T,
            '(' => TokenType::OpenParen,
            ')' => TokenType::CloseParen,
            '+' => TokenType::Plus,
            '-' => TokenType::Minus,
            '*' => TokenType::Asterisk,
            '/' => TokenType::Slash,
            '%' => TokenType::Percent,
            '^' => TokenType::Carat,
            '~' => TokenType::Tilde,
            '?' => TokenType::Question,
            ':' => TokenType::Colon,
            '<' => {let v = iter.next(); if Some('=') == v {
                self.index += 1;
                TokenType::LessEqual
            } else if Some('<') == v {
                self.index += 1;
                TokenType::LeftShift
            } else { TokenType::Less }},
            '>' => {let v = iter.next(); if Some('=') == v {
                self.index += 1;
                TokenType::GreaterEqual
            } else if Some('>') == v {
                self.index += 1;
                TokenType::RightShift
            } else { TokenType::Greater }},
            '=' => {
                if Some('=') != iter.next() { return Err(ParsingError::broken_equal(self.index)) }
                self.index += 1;
                TokenType::DoubleEqual
            },
            '|' => if Some('|') == iter.next() {
                self.index += 1;
                TokenType::DoubleBar
            } else { TokenType::Bar },
            '&' => if Some('&') == iter.next() {
                self.index += 1;
                TokenType::DoubleAmpersand
            } else { TokenType::Ampersand },
            '!' => if Some('!') == iter.next() {
                self.index += 1;
                TokenType::BangEqual
            } else { TokenType::Bang },
            n if n.is_ascii_digit() || n == '.' => return self.parse_number(),
            chr => TokenType::Unknown(chr)
        };
        let index = self.index;
        self.index += 1;
        Ok(Token { ty, index })
    }
}

impl<'str> Lexer<'str> {
    pub fn peek(&self) -> Result<Token, ParsingError> {
        let mut v = *self;
        v.next()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Format {
    Decimal, Hex, Octal, Binary, Double, DoubleExp, Float, FloatExp
}

impl<'str> Lexer<'str> {
    fn parse_number(&mut self) -> Result<Token, ParsingError> {
        let str = &self.buf[self.index..];
        let mut format = Format::Decimal;
        let mut last = '\0';
        let mut last_idx = 0;
        for (i, chr) in str.chars().chain(std::iter::once('\0')).enumerate() {
            last_idx = i;
            match chr {
                'x' | 'X' if i == 1 && last == '0'
                    => format = Format::Hex,
                'b' | 'B' if i == 1 && last == '0'
                    => format = Format::Binary,
                'o' | 'O' if i == 1 && last == '0'
                    => format = Format::Octal,
                '.' if format == Format::Decimal
                    => format = Format::Double,
                'e' | 'E' if matches!(format, Format::Double | Format::Decimal) =>
                    format = Format::DoubleExp,
                '-' | '+' if format == Format::DoubleExp && matches!(last, 'e' | 'E') => {},
                'f' if matches!(format, Format::Double | Format::Decimal) => format = Format::Float,
                'f' if format == Format::DoubleExp => format = Format::FloatExp,
                c if c.is_ascii_hexdigit() && format == Format::Hex
                    => {}
                c if c.is_ascii_digit() && matches!(format, Format::Decimal | Format::Double | Format::DoubleExp)
                    => {},
                c if matches!(c, '0' ..= '7') && format == Format::Octal
                    => {},
                c if matches!(c, '0' | '1') && format == Format::Binary
                    => {},
                _ => break
            }
            last = chr;
        }
        match format {
            Format::Decimal => u32::from_str(&str[..last_idx])
                .map(TokenType::Integer)
                .map_err(|_| ParsingError::invalid_literal(self.index)),
            Format::Hex => u32::from_str_radix(&str[2..last_idx], 16)
                .map(TokenType::Integer)
                .map_err(|_| ParsingError::invalid_literal(self.index)),
            Format::Binary => u32::from_str_radix(&str[2..last_idx], 2)
                .map(TokenType::Integer)
                .map_err(|_| ParsingError::invalid_literal(self.index)),
            Format::Octal => u32::from_str_radix(&str[2..last_idx], 8)
                .map(TokenType::Integer)
                .map_err(|_| ParsingError::invalid_literal(self.index)),
            Format::Double | Format::DoubleExp => f64::from_str(&str[..last_idx]).ok()
                .and_then(|n| NotNan::new(n).map(TokenType::Double).ok())
                .ok_or(ParsingError::invalid_literal(self.index)),
            Format::Float | Format::FloatExp => f32::from_str(&str[..last_idx - 1]).ok()
                .and_then(|n| NotNan::new(n).map(TokenType::Float).ok())
                .ok_or(ParsingError::invalid_literal(self.index)),
        }
        .map(|ty| Token { ty, index: self.index })
        .inspect(|_| self.index += last_idx)
    }
}