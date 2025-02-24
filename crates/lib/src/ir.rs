use std::fmt::{Display, Write};
use indenter::indented;

use crate::parser::{BinaryOperand, BinaryOperandType as Bin, Expression, UnaryOperand, UnaryOperandType, STACK_NEWSIZE, STACK_REDZONE};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BinaryOperandType {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulus,
    Eq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    LeftShift,
    RightShift,
    BinaryAnd,
    BinaryXor,
    BinaryOr,
}

impl From<Bin> for BinaryOperandType {
    fn from(value: Bin) -> Self {
        match value {
            Bin::Add => Self::Add,
            Bin::Subtract => Self::Subtract,
            Bin::Multiply => Self::Multiply,
            Bin::Divide => Self::Divide,
            Bin::Modulus => Self::Modulus,
            Bin::Eq => Self::Eq,
            Bin::NotEq => Self::NotEq,
            Bin::Less => Self::Less,
            Bin::LessEq => Self::LessEq,
            Bin::Greater => Self::Greater,
            Bin::GreaterEq => Self::GreaterEq,
            Bin::LeftShift => Self::LeftShift,
            Bin::RightShift => Self::RightShift,
            Bin::BinaryAnd => Self::BinaryAnd,
            Bin::BinaryXor => Self::BinaryXor,
            Bin::BinaryOr => Self::BinaryOr,
            Bin::Ternary(_) => unreachable!("should never convert ternary into IR-level binary operation"),
        }
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value {
    Register(Register),
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    F64(f64),
    U1(bool),
}

impl Value {
    pub fn ty(&self) -> Type {
        match self {
            Value::Register(register) => register.ty(),
            Value::U8(_) => Type::U8,
            Value::U16(_) => Type::U16,
            Value::U32(_) => Type::U32,
            Value::I8(_) => Type::I8,
            Value::I16(_) => Type::I16,
            Value::I32(_) => Type::I32,
            Value::F32(_) => Type::F32,
            Value::F64(_) => Type::F64,
            Value::U1(_) => Type::U1
        }
    }
}
impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Register(register) => register.fmt(f),
            Value::U8(v) => write!(f, "{v}"),
            Value::I8(v) => write!(f, "{v}"),
            Value::U16(v) => write!(f, "{v}"),
            Value::I16(v) => write!(f, "{v}"),
            Value::U32(v) => write!(f, "{v}"),
            Value::I32(v) => write!(f, "{v}"),
            Value::F32(v) => write!(f, "{v}"),
            Value::F64(v) => write!(f, "{v}"),
            Value::U1(v) => write!(f, "{v}"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type { U1, U8, I8, U16, I16, U32, I32, F32, F64 }
impl Type {
    const fn integer_promote(&self) -> Type {
        match *self {
            Type::U1 | Type::U8 | Type::I8 | Type::U16 | Type::I16 => Type::I32,
            Type::U32 | Type::I32 | Type::F32 | Type::F64 => *self,
        }
    }
    
    const fn make_signed(&self) -> Type {
        match *self {
            Self::U8 => Self::I8,
            Self::U16 => Self::I16,
            Self::U32 => Self::I32,
            ty => ty
        }
    }
    
    const fn is_float(&self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
    
    pub(crate) const fn is_signed(&self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I32) || self.is_float()
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Type::U1 => "u1",
            Type::U8 => "u8",
            Type::U16 => "u16",
            Type::U32 => "u32",
            Type::I8 => "i8",
            Type::I16 => "i16",
            Type::I32 => "i32",
            Type::F32 => "f32",
            Type::F64 => "f64"
        })
    }
}

#[derive(Debug, Clone)]
pub enum Instruction {
    Constant { output: Register, payload: Value },
    Binary { output: Register, ty: BinaryOperandType, lhs: Register, rhs: Register },
    Unary { output: Register, ty: UnaryOperandType, payload: Register },
    Convert { output: Register, ty: Type, payload: Register },
    Ternary { output: Register, cond: Register, yes: Vec<Instruction>, lreg: Register, no: Vec<Instruction>, rreg: Register },
    Return { payload: Register },
    Deny { cond: Register }
}

impl Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Instruction::Constant { output, payload } => write!(f, "{output} := {payload};"),
            Instruction::Binary { output, ty, lhs, rhs } => write!(f, "{output} := {lhs} {ty} {rhs};"),
            Instruction::Unary { output, ty, payload } => write!(f, "{output} := {ty} {payload};"),
            Instruction::Convert { output, ty, payload } => write!(f, "{output} := {payload} as {ty};"),
            Instruction::Return { payload } => write!(f, "return {payload};"),
            Instruction::Deny { cond } => write!(f, "deny {cond};"),
            Instruction::Ternary { output, cond, yes, lreg, no, rreg } => {
                writeln!(f, "{output} := if {cond}; {lreg} @ {{")?;
                for inst in yes {
                    stacker::maybe_grow(STACK_REDZONE, STACK_NEWSIZE, || writeln!(indented(f), "{inst}"))?;
                }
                writeln!(f, "}}; else {rreg} @ {{")?;
                for inst in no {
                    stacker::maybe_grow(STACK_REDZONE, STACK_NEWSIZE, || writeln!(indented(f), "{inst}"))?;
                }
                write!(f, "}};")
            },
        }
    }
}

mod hide {
    pub struct CodeGenerator {
        register_index: u128,
        pub(super) instructions: Vec<super::Instruction>
    }

    impl CodeGenerator {
        pub const fn new() -> Self {
            Self { register_index: 0, instructions: Vec::new() }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[must_use]
    pub struct Register { pub(crate) id: u128, ty: super::Type }
    impl Register {
        pub fn ty(&self) -> super::Type {
            self.ty
        }
    }

    impl std::fmt::Display for Register {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{} %{}", self.ty, self.id)
        }
    }

    impl super::CodeGenerator {
        pub fn make_register(&mut self, ty: super::Type) -> Register {
            let v = self.register_index;
            self.register_index += 1;
            Register { id: v, ty }
        }
    }
}


pub use hide::{Register, CodeGenerator};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompilationErrorType {

}

impl std::fmt::Display for CompilationErrorType {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            _ => todo!()
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompilationError {
    pub ty: CompilationErrorType,
    pub index: usize
}

impl std::fmt::Display for CompilationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "compilation error at index {}: {}", self.index, self.ty)
    }
}

impl std::error::Error for CompilationError {}

impl CodeGenerator {
    pub fn generate_ir(mut self, expr: Expression) -> Result<Vec<Instruction>, CompilationError> {
        let t_register = self.make_register(Type::I32);
        let mut payload = self.gen_expr(expr, &t_register)?;
        if payload.ty() != Type::U8 {
            payload = self.convert(payload, Type::U8);
        }
        self.instructions.push(Instruction::Return { payload });
        Ok(self.instructions)
    }

    #[inline]
    fn gexpr(&mut self, expr: Expression, t: &Register) -> Result<Register, CompilationError> {
        stacker::maybe_grow(STACK_REDZONE, STACK_NEWSIZE, || self.gen_expr(expr, t))
    }

    fn gen_expr(&mut self, expr: Expression, t: &Register) -> Result<Register, CompilationError> {
        Ok(match expr {
            Expression::Integer { value, .. } => self.init(
                i32::try_from(value).map_or(Value::U32(value), Value::I32)
            ),
            Expression::Float { value, .. } => self.init(Value::F32(value)),
            Expression::Double { value, .. } => self.init(Value::F64(value)),
            Expression::T { .. } => t.clone(),
            Expression::Unary { prefix: UnaryOperand { ty: UnaryOperandType::Identity, .. }, value } => {
                let reg = self.gexpr(*value, t)?;
                let ity = reg.ty().integer_promote();
                if reg.ty() != ity { self.convert(reg, ity) } else { reg }
            },
            Expression::Unary { prefix: UnaryOperand { ty: UnaryOperandType::Not, .. }, value } => {
                let reg = self.gexpr(*value, t)?;
                let ity = reg.ty().integer_promote();
                let oreg = if reg.ty() != ity { self.convert(reg, ity) } else { reg };
                let rreg = self.make_register(Type::U1);
                self.instructions.push(Instruction::Unary { output: rreg.clone(), ty: UnaryOperandType::Not, payload: oreg });
                rreg
            },
            Expression::Unary { prefix: UnaryOperand { ty: UnaryOperandType::Negate, .. }, value } => {
                let reg = self.gexpr(*value, t)?;
                let sty = reg.ty().integer_promote().make_signed();
                let oreg = if reg.ty() != sty { self.convert(reg, sty) } else { reg };
                let rreg = self.make_register(sty);
                self.instructions.push(Instruction::Unary { output: rreg.clone(), ty: UnaryOperandType::Negate, payload: oreg });
                rreg
            },
            Expression::Unary { prefix: UnaryOperand { ty: UnaryOperandType::Invert, .. }, value } => {
                let mut reg = self.gexpr(*value, t)?;
                if reg.ty().is_float() {
                    reg = self.convert(reg, Type::I32)
                }
                let sty = reg.ty().integer_promote();
                let oreg = if reg.ty() != sty { self.convert(reg, sty) } else { reg };
                let rreg = self.make_register(sty);
                self.instructions.push(Instruction::Unary { output: rreg.clone(), ty: UnaryOperandType::Invert, payload: oreg });
                rreg
            },
            Expression::Binary { lhs, operand: BinaryOperand { ty: Bin::Ternary(expr), ..}, rhs } => {
                let cond = self.gexpr(*lhs, t)?;
                let pre = self.instructions.len();
                
                let mut lreg = self.gexpr(*expr, t)?;
                let mut yes = self.instructions.drain(pre..).collect::<Vec<_>>();

                let mut rreg = self.gexpr(*rhs, t)?;
                let mut no = self.instructions.drain(pre..).collect::<Vec<_>>();
                
                // major fuckery here because integer promotion
                let iaty = lreg.ty().integer_promote();
                let ibty = rreg.ty().integer_promote();
                let precise_ty = iaty.max(ibty);

                if lreg.ty() != precise_ty {
                    lreg = self.convert(lreg, precise_ty);
                    yes.extend(self.instructions.drain(pre..))
                }

                if rreg.ty() != precise_ty {
                    rreg = self.convert(rreg, precise_ty);
                    no.extend(self.instructions.drain(pre..))
                }

                let output = self.make_register(precise_ty);
                self.instructions.push(Instruction::Ternary { output: output.clone(), cond, yes, lreg, no, rreg });
                output
            },
            Expression::Binary { lhs, operand: BinaryOperand { ty: ty @ Bin::Divide | ty @ Bin::Modulus, .. }, rhs } => {
                let mut lhs = self.gexpr(*lhs, t)?;
                let mut rhs = self.gexpr(*rhs, t)?;
                if ty == Bin::Modulus {
                    if lhs.ty().is_float() { lhs = self.convert(lhs, Type::I32) }
                    if rhs.ty().is_float() { rhs = self.convert(rhs, Type::I32) }
                }

                if !rhs.ty().is_float() {
                    // If !rhs, then we branch to error
                    let reg = self.init(Value::Register(rhs.clone()));
                    let ity = reg.ty().integer_promote();
                    let oreg = if reg.ty() != ity { self.convert(reg, ity) } else { reg };
                    let rreg = self.make_register(Type::U1);
                    self.instructions.push(Instruction::Unary { output: rreg.clone(), ty: UnaryOperandType::Not, payload: oreg });
                    self.instructions.push(Instruction::Deny { cond: rreg });
                }

                let pty = lhs.ty().integer_promote().max(rhs.ty().integer_promote());
                let outreg = self.make_register(pty);
                if lhs.ty() != pty {
                    lhs = self.convert(lhs, pty);
                }
                if rhs.ty() != pty {
                    rhs = self.convert(rhs, pty);
                }
                self.instructions.push(Instruction::Binary { output: outreg.clone(), lhs, rhs, ty: ty.into() });
                outreg
            },
            Expression::Binary { lhs, operand: BinaryOperand { ty: ty @ Bin::LeftShift | ty @ Bin::RightShift | ty @ Bin::BinaryAnd | ty @ Bin::BinaryOr | ty @ Bin::BinaryXor, .. }, rhs } => {
                let mut lhs = self.gexpr(*lhs, t)?;
                let mut rhs = self.gexpr(*rhs, t)?;
                if lhs.ty().is_float() { lhs = self.convert(lhs, Type::I32) }
                if rhs.ty().is_float() { rhs = self.convert(rhs, Type::I32) }

                let pty = lhs.ty().integer_promote().max(rhs.ty().integer_promote());
                if lhs.ty() != pty {
                    lhs = self.convert(lhs, pty);
                }
                if rhs.ty() != pty {
                    rhs = self.convert(rhs, pty);
                }
                let outreg = self.make_register(pty);
                self.instructions.push(Instruction::Binary { output: outreg.clone(), lhs, rhs, ty: ty.into() });
                outreg
            },
            Expression::Binary { lhs, operand: BinaryOperand { ty: ty @ Bin::Less | ty @ Bin::Greater | ty @ Bin::Eq | ty @ Bin::NotEq | ty @ Bin::LessEq | ty @ Bin::GreaterEq, .. }, rhs } => {
                let mut lhs = self.gexpr(*lhs, t)?;
                let mut rhs = self.gexpr(*rhs, t)?;

                let pty = lhs.ty().integer_promote().max(rhs.ty().integer_promote());
                if lhs.ty() != pty {
                    lhs = self.convert(lhs, pty);
                }
                if rhs.ty() != pty {
                    rhs = self.convert(rhs, pty);
                }
                let outreg = self.make_register(Type::U1);
                self.instructions.push(Instruction::Binary { output: outreg.clone(), lhs, rhs, ty: ty.into() });
                outreg
            },
            Expression::Binary { lhs, operand: BinaryOperand { ty, .. }, rhs } => {
                let mut lhs = self.gexpr(*lhs, t)?;
                let mut rhs = self.gexpr(*rhs, t)?;

                let pty = lhs.ty().integer_promote().max(rhs.ty().integer_promote());
                if lhs.ty() != pty {
                    lhs = self.convert(lhs, pty);
                }
                if rhs.ty() != pty {
                    rhs = self.convert(rhs, pty);
                }
                let outreg = self.make_register(pty);
                self.instructions.push(Instruction::Binary { output: outreg.clone(), lhs, rhs, ty: ty.into() });
                outreg
            }
        })
    }

    fn init(&mut self, payload: Value) -> Register {
        let ty = payload.ty();
        let reg = self.make_register(ty);
        self.instructions.push(Instruction::Constant { output: reg.clone(), payload });
        reg
    }

    fn convert(&mut self, reg: Register, ty: Type) -> Register {
        let out = self.make_register(ty);
        self.instructions.push(Instruction::Convert { output: out.clone(), ty, payload: reg });
        out
    }
}