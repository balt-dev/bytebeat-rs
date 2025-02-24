use std::{collections::HashMap, marker::PhantomData, mem::forget, sync::Arc};

use inkwell::{basic_block::BasicBlock, builder::{Builder, BuilderError}, context::Context, types::{AnyType, AsTypeRef, BasicType, FloatType, IntType}, values::{AnyValue, AsValueRef, BasicValue, BasicValueEnum, FloatValue, FunctionValue, IntValue}, FloatPredicate, IntPredicate, OptimizationLevel};
use llvm_sys::execution_engine::{LLVMDisposeExecutionEngine, LLVMExecutionEngineRef};

use crate::{ir::{BinaryOperandType, Instruction, Register, Type, Value}, parser::{UnaryOperandType, STACK_NEWSIZE, STACK_REDZONE}};

pub(crate) struct Codegen<'ctx> {
    ctx: &'ctx Context,
    builder: Builder<'ctx>,
    function: FunctionValue<'ctx>,
    i1: IntType<'ctx>,
    i8: IntType<'ctx>,
    i16: IntType<'ctx>,
    i32: IntType<'ctx>,
    f32: FloatType<'ctx>,
    f64: FloatType<'ctx>,
    register_map: HashMap<u128, NumberValue<'ctx>>,
    explode_block: BasicBlock<'ctx>
}

pub fn jit<'ctx, 'arr>(ctx: &'ctx Context, arr: Vec<Instruction>, opt: OptimizationLevel) -> Result<TimeFunc<'ctx>, BuilderError> {
    let i1 = ctx.bool_type();
    let i8 = ctx.i8_type();
    let i16 = ctx.i16_type();
    let i32 = ctx.i32_type();
    let f32 = ctx.f32_type();
    let f64 = ctx.f64_type();
    let func = i8.fn_type(&[i32.into()], false);
    let module = ctx.create_module("bytebeat");
    let engine = module.create_jit_execution_engine(opt)
        .expect("failed to create JIT execution engine");
    let function = module.add_function("beat", func, None);
    let block = ctx.append_basic_block(function, "entry");
    let t = function.get_nth_param(0).expect("function should have 1 parameter").into_int_value();
    let builder = ctx.create_builder();

    let explode_block = ctx.append_basic_block(function, "EXPLODE");
    builder.position_at_end(explode_block);
    builder.build_return(Some(&i8.const_int(0, false)))?;

    builder.position_at_end(block);
    let mut register_map = HashMap::new();
    register_map.insert(0, NumberValue::Int { int: t, signed: true });
    let mut cg = Codegen {
        ctx,
        builder, function,
        i1, i8, i16, i32, f32, f64,
        register_map, explode_block
    };
    for inst in arr.into_iter() {
        inst.build(&mut cg)?;
    }
    
    // SAFETY: We don't add other functions after this.
    unsafe {
        let func = engine.get_function("beat").expect("jitted function should exist");
        let fn_ptr = func.into_raw();
        let engine_ref = engine.as_mut_ptr();
        forget(engine);
        Ok(TimeFunc { _engine: SendExecEngineInner(Arc::new(engine_ref), PhantomData), fn_ptr })
    }
}


#[derive(Clone)]
pub struct TimeFunc<'ctx> {
    _engine: SendExecEngineInner<'ctx>,
    fn_ptr: unsafe extern "C" fn(i32) -> u8
}

impl<'ctx> TimeFunc<'ctx> {
    pub unsafe fn call(&self, t: i32) -> u8 {
        // SAFETY: This won't dangle until the engine is dropped, which doesn't happen unless the whole object is dropped.
        unsafe { (self.fn_ptr)(t) }
    }
}

#[derive(Clone)]
struct SendExecEngineInner<'ctx>(Arc<LLVMExecutionEngineRef>, PhantomData<&'ctx Context>);

impl<'ctx> Drop for SendExecEngineInner<'ctx> {
    fn drop(&mut self) {
        // SAFETY: This is essentially just an Arc version of the internals of Inkwell's JitFunction.
        if Arc::strong_count(&self.0) == 1 {
            unsafe {
                LLVMDisposeExecutionEngine(*self.0);
            }
        }
    }
}

unsafe impl<'ctx> Send for SendExecEngineInner<'ctx> {}
unsafe impl<'ctx> Sync for SendExecEngineInner<'ctx> {}

impl<'ctx> Codegen<'ctx> {
    fn put(&mut self, reg: Register, value: impl AsNumberValue<'ctx>) {
        let n = reg.id;
        value.check(&reg);
        let v = self.register_map.insert(n, value.into());
        assert!(v.is_none(), "duplicated register {n}")
    }

    #[must_use]
    fn get(&mut self, reg: Register) -> NumberValue<'ctx> {
        *self.register_map.get(&reg.id).expect(&format!("fetched register {} should always exist", reg.id))
    }
}

impl Instruction {
    fn build(self, cg: &mut Codegen) -> Result<(), BuilderError> {
        match self {
            Instruction::Constant { output, payload } => {
                match payload {
                    Value::Register(reg) => {
                        let val = cg.get(reg);
                        cg.put(output, val)
                    }
                    Value::U1(v) => cg.put(output, (cg.i1.const_int(v as u64, false), false)),
                    Value::U8(v) => cg.put(output, (cg.i8.const_int(v as u64, false), false)),
                    Value::I8(v) => cg.put(output, (cg.i8.const_int(v as u64, false), true)),
                    Value::U16(v) => cg.put(output, (cg.i16.const_int(v as u64, false), false)),
                    Value::I16(v) => cg.put(output, (cg.i16.const_int(v as u64, false), true)),
                    Value::U32(v) => cg.put(output, (cg.i32.const_int(v as u64, false), false)),
                    Value::I32(v) => cg.put(output, (cg.i32.const_int(v as u64, false), true)),
                    Value::F32(v) => cg.put(output, cg.f32.const_float(v as f64)),
                    Value::F64(v) => cg.put(output, cg.f64.const_float(v))
                }
            },
            Instruction::Binary { output, ty, lhs, rhs } => {
                ty.emit_operation(cg, lhs, rhs, output)?
            },
            Instruction::Unary { output, ty, payload } => {
                ty.emit_operation(cg, payload, output)?
            },
            Instruction::Convert { output, ty, payload } => {
                ty.emit_conversion(cg, payload, output)?
            },
            Instruction::Ternary { output, cond, yes, lreg, no, rreg } => {
                let id = cond.id;
                let value = cg.get(cond);

                let ptr_ty = output.ty().as_llvm(cg);
                let ptr_sign = if let NumberType::Int { signed, .. } = ptr_ty { signed } else { true };
                let ptr = cg.builder.build_alloca(ptr_ty, &format!("RES::{}", id))?;
                let yes_block = cg.ctx.append_basic_block(cg.function, &format!("TRUE::{}", id));
                let no_block = cg.ctx.append_basic_block(cg.function, &format!("FALSE::{}", id));
                let cont_block = cg.ctx.append_basic_block(cg.function, &format!("CONT::{}", id));
                
                let bool = value.into_bool(cg, id)?;
                cg.builder.build_conditional_branch(bool, yes_block, no_block)?;

                cg.builder.position_at_end(yes_block);
                for inst in yes.into_iter() {
                    stacker::maybe_grow(STACK_REDZONE, STACK_NEWSIZE, || inst.build(cg))?;
                }
                let lhs_value = cg.get(lreg);
                cg.builder.build_store(ptr, lhs_value)?;
                cg.builder.build_unconditional_branch(cont_block)?;

                cg.builder.position_at_end(no_block);
                for inst in no.into_iter() {
                    stacker::maybe_grow(STACK_REDZONE, STACK_NEWSIZE, || inst.build(cg))?;
                }
                let rhs_value = cg.get(rreg);
                cg.builder.build_store(ptr, rhs_value)?;
                cg.builder.build_unconditional_branch(cont_block)?;

                cg.builder.position_at_end(cont_block);
                let val = cg.builder.build_load(ptr_ty, ptr, &format!("VAL::{}", id))?;
                let v: NumberValue = match val {
                    BasicValueEnum::IntValue(i) => (i, ptr_sign).into(),
                    BasicValueEnum::FloatValue(f) => f.into(),
                    _ => unreachable!("got a value of non-numeric type from dereferencing stack pointer ({output})")
                };
                cg.put(output, v)
            },
            Instruction::Return { payload } => {
                assert!(payload.ty() == Type::U8, "should always be returning a u8 ({payload})");
                let payload_val = cg.get(payload);
                cg.builder.build_return(Some(&payload_val))?;
            },
            Instruction::Deny { cond } => {
                let id = cond.id;
                let value = cg.get(cond);
                let cont_block = cg.ctx.append_basic_block(cg.function, &format!("CONT::{}", id));
                let bool = value.into_bool(cg, id)?;
                cg.builder.build_conditional_branch(bool, cg.explode_block, cont_block)?;
                cg.builder.position_at_end(cont_block);
            },
        };
        Ok(())
    }
}


#[derive(Debug, Copy, Clone)]
enum NumberValue<'ctx> {
    Int { int: IntValue<'ctx>, signed: bool },
    Float(FloatValue<'ctx>)
}

impl<'ctx> NumberValue<'ctx> {
    fn into_bool(self, cg: &mut Codegen<'ctx>, id: u128) -> Result<IntValue<'ctx>, BuilderError> {
        match self {
            NumberValue::Int { int, .. } => {
                cg.builder.build_int_compare(inkwell::IntPredicate::NE, int, int.get_type().const_zero(), &format!("ASBOOL::{}", id))
            },
            NumberValue::Float(float) => {
                cg.builder.build_float_compare(inkwell::FloatPredicate::ONE, float, float.get_type().const_zero(), &format!("ASBOOL::{}", id))
            }
        }
    }
}

trait AsNumberValue<'ctx>: Into<NumberValue<'ctx>> {
    fn check(&self, reg: &Register);
}

impl<'ctx> From<(IntValue<'ctx>, bool)> for NumberValue<'ctx> {
    fn from(value: (IntValue<'ctx>, bool)) -> Self {
        NumberValue::Int { int: value.0, signed: value.1 }
    }
}

impl<'ctx> From<FloatValue<'ctx>> for NumberValue<'ctx> {
    fn from(value: FloatValue<'ctx>) -> Self {
        NumberValue::Float(value)
    }
}

impl<'ctx> AsNumberValue<'ctx> for (IntValue<'ctx>, bool) {
    fn check(&self, reg: &Register) {
        match (reg.ty(), self.0.get_type().get_bit_width(), self.1) {
            (Type::U1, 1, false) |
            (Type::U8, 8, false) |
            (Type::U16, 16, false) |
            (Type::U32, 32, false) |
            (Type::I8, 8, true) |
            (Type::I16, 16, true) |
            (Type::I32, 32, true) => {},
            (ty, width, signed) => panic!("incorrect type assigned to register {reg} - expected a {ty}, but got an integer with width {width} and signedness {signed}")
        }
    }
}
impl<'ctx> AsNumberValue<'ctx> for FloatValue<'ctx> {
    fn check(&self, reg: &Register) {
        match reg.ty() {
            Type::F32 | Type::F64 => {},
            ty => panic!("incorrect type assigned to register {reg} - expected a {ty}, but got a float")
        }
    }
}
impl<'ctx> AsNumberValue<'ctx> for NumberValue<'ctx> {
    fn check(&self, reg: &Register) {
        match self {
            NumberValue::Int { int, signed } => (*int, *signed).check(reg),
            NumberValue::Float(float_value) => float_value.check(reg),
        }
    }
}

// SAFETY: We're forwarding impl to Int/FloatValue.
unsafe impl<'ctx> AsValueRef for NumberValue<'ctx> {
    fn as_value_ref(&self) -> llvm_sys::prelude::LLVMValueRef {
        match self {
            Self::Int { int , .. } => int.as_value_ref(),
            Self::Float(f) => f.as_value_ref()
        }
    }
}
unsafe impl<'ctx> AnyValue<'ctx> for NumberValue<'ctx> {}
unsafe impl<'ctx> BasicValue<'ctx> for NumberValue<'ctx> {}


#[derive(Debug, Copy, Clone)]
enum NumberType<'ctx> {
    Int { ty: IntType<'ctx>, signed: bool },
    Float(FloatType<'ctx>)
}

unsafe impl<'ctx> AsTypeRef for NumberType<'ctx> {
    fn as_type_ref(&self) -> llvm_sys::prelude::LLVMTypeRef {
        match self {
            Self::Int { ty: i, .. } => i.as_type_ref(),
            Self::Float(f) => f.as_type_ref()
        }
    }
}
unsafe impl<'ctx> AnyType<'ctx> for NumberType<'ctx> {}
unsafe impl<'ctx> BasicType<'ctx> for NumberType<'ctx> {}

impl BinaryOperandType {
    pub(crate) fn emit_operation(&self, cg: &mut Codegen<'_>, lhs: Register, rhs: Register, output: Register) -> Result<(), BuilderError> {
        use NumberValue::*;
        use BinaryOperandType::*;
        let oid = output.id;
        let reg_name = format!("BINOP::{oid}");
        let l = cg.get(lhs);
        let r = cg.get(rhs);
        let signed = output.ty().is_signed();
        
        match (l, *self, r) {
            (Int { .. }, _, Float(_))
            | (Float(_), _, Int { .. })
                => unreachable!("should never operate between different types ({output})"),
            (Int { signed: true, .. }, _, Int {signed: false, ..})
            | (Int { signed: false, .. }, _, Int {signed: true, ..})
                => unreachable!("should never operate between integers of different signedness ({output})"),
            (Float(_), LeftShift | RightShift | BinaryAnd | BinaryXor | BinaryOr, Float(_))
                => unreachable!("should not use bitwise operations on floats ({output})"),
            (Int { int: i1, .. }, Add, Int { int: i2, .. }) 
                => cg.put(output, (cg.builder.build_int_add(i1, i2, &reg_name)?, signed)),
            (Int { int: i1, .. }, Subtract, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_sub(i1, i2, &reg_name)?, signed)),
            (Int { int: i1, .. }, Multiply, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_mul(i1, i2, &reg_name)?, signed)),
            (Int { int: i1, signed: false }, Divide, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_unsigned_div(i1, i2, &reg_name)?, false)),
            (Int { int: i1, signed: true }, Divide, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_signed_div(i1, i2, &reg_name)?, true)),
            (Int { int: i1, signed: false }, Modulus, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_unsigned_rem(i1, i2, &reg_name)?, false)),
            (Int { int: i1, signed: true }, Modulus, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_signed_rem(i1, i2, &reg_name)?, true)),
            (Int { int: i1, .. }, Eq, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_compare(IntPredicate::EQ, i1, i2, &reg_name)?, false)),
            (Int { int: i1, .. }, NotEq, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_compare(IntPredicate::NE, i1, i2, &reg_name)?, false)),
            (Int { int: i1, signed: false }, Less, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_compare(IntPredicate::ULT, i1, i2, &reg_name)?, false)),
            (Int { int: i1, signed: true }, Less, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_compare(IntPredicate::SLT, i1, i2, &reg_name)?, false)),
            (Int { int: i1, signed: false }, LessEq, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_compare(IntPredicate::ULE, i1, i2, &reg_name)?, false)),
            (Int { int: i1, signed: true }, LessEq, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_compare(IntPredicate::SLE, i1, i2, &reg_name)?, false)),
            (Int { int: i1, signed: false }, Greater, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_compare(IntPredicate::UGT, i1, i2, &reg_name)?, false)),
            (Int { int: i1, signed: true }, Greater, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_compare(IntPredicate::SGT, i1, i2, &reg_name)?, false)),
            (Int { int: i1, signed: false }, GreaterEq, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_compare(IntPredicate::UGE, i1, i2, &reg_name)?, false)),
            (Int { int: i1, signed: true }, GreaterEq, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_int_compare(IntPredicate::SGE, i1, i2, &reg_name)?, false)),
            (Int { int: i1, signed }, LeftShift, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_left_shift(i1, i2, &reg_name)?, signed)),
            (Int { int: i1, signed: sign }, RightShift, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_right_shift(i1, i2, sign, &reg_name)?, signed)),
            (Int { int: i1, signed }, BinaryAnd, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_and(i1, i2, &reg_name)?, signed)),
            (Int { int: i1, signed }, BinaryXor, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_xor(i1, i2, &reg_name)?, signed)),
            (Int { int: i1, signed }, BinaryOr, Int { int: i2, .. })
                => cg.put(output, (cg.builder.build_or(i1, i2, &reg_name)?, signed)),
            (Float(f1), Add, Float(f2))
                => cg.put(output, cg.builder.build_float_add(f1, f2, &reg_name)?),
            (Float(f1), Subtract, Float(f2))
                => cg.put(output, cg.builder.build_float_sub(f1, f2, &reg_name)?),
            (Float(f1), Multiply, Float(f2))
                => cg.put(output, cg.builder.build_float_mul(f1, f2, &reg_name)?),
            (Float(f1), Divide, Float(f2))
                => cg.put(output, cg.builder.build_float_div(f1, f2, &reg_name)?),
            (Float(f1), Modulus, Float(f2))
                => cg.put(output, cg.builder.build_float_rem(f1, f2, &reg_name)?),
            (Float(f1), Eq, Float(f2))
                => cg.put(output, (cg.builder.build_float_compare(FloatPredicate::OEQ, f1, f2, &reg_name)?, false)),
            (Float(f1), NotEq, Float(f2))
                => cg.put(output, (cg.builder.build_float_compare(FloatPredicate::ONE, f1, f2, &reg_name)?, false)),
            (Float(f1), Less, Float(f2))
                => cg.put(output, (cg.builder.build_float_compare(FloatPredicate::OLT, f1, f2, &reg_name)?, false)),
            (Float(f1), LessEq, Float(f2))
                => cg.put(output, (cg.builder.build_float_compare(FloatPredicate::OLE, f1, f2, &reg_name)?, false)),
            (Float(f1), Greater, Float(f2))
                => cg.put(output, (cg.builder.build_float_compare(FloatPredicate::OGT, f1, f2, &reg_name)?, false)),
            (Float(f1), GreaterEq, Float(f2))
                => cg.put(output, (cg.builder.build_float_compare(FloatPredicate::OGE, f1, f2, &reg_name)?, false)),
        };
        Ok(())
    }
}


impl UnaryOperandType {
    pub(crate) fn emit_operation(&self, cg: &mut Codegen<'_>, payload: Register, output: Register) -> Result<(), BuilderError> {
        let val = cg.get(payload);
        match (val, *self) {
            (v, UnaryOperandType::Not) => {
                let bool = v.into_bool(cg, output.id)?;
                let one = cg.i1.const_all_ones();
                let invbool = cg.builder.build_xor(bool, one, &format!("BOOLXOR::{}", output.id))?;
                let i = cg.builder.build_int_cast(invbool, cg.i1, &format!("BOOLNOT::{}", output.id))?;
                cg.put(output, (i, false));
            },
            (NumberValue::Int { int, signed }, UnaryOperandType::Negate) => {
                let i = cg.builder.build_int_neg(int, &format!("NEGATEINT::{}", output.id))?;
                cg.put(output, (i, signed));
            },
            (NumberValue::Int { int, signed }, UnaryOperandType::Invert) => {
                let ty = int.get_type();
                let mask = ty.const_all_ones();
                let i = cg.builder.build_xor(int, mask, &format!("INVERT::{}", output.id))?;
                cg.put(output, (i, signed));
            },
            (NumberValue::Float(float), UnaryOperandType::Negate) => {
                let f = cg.builder.build_float_neg(float, &format!("NEGATEFLOAT::{}", output.id))?;
                cg.put(output, f);
            },
            (NumberValue::Float(_), UnaryOperandType::Invert) => 
                unreachable!("should not invert a float ({output})"),
            (_, UnaryOperandType::Identity) => {}
        };
        Ok(())
    }
}


impl Type {
    pub(crate) fn emit_conversion(&self, cg: &mut Codegen<'_>, payload: Register, output: Register) -> Result<(), BuilderError> {
        let val = cg.get(payload);
        self.conv(cg, val, output)
    }

    fn conv<'ctx>(&self, cg: &mut Codegen<'ctx>, val: NumberValue<'ctx>, output: Register) -> Result<(), BuilderError> {
        let reg_name = format!("CONV::{}", output.id);
        let v = match (val, output.ty()) {
            (n, Type::U1) =>
                NumberValue::from((n.into_bool(cg, output.id)?, false)),
            (NumberValue::Int { int: i , ..}, t @ Type::U8 | t @ Type::I8) =>
                NumberValue::from((cg.builder.build_int_cast_sign_flag(i, cg.i8, t.is_signed(), &reg_name)?, t.is_signed())),
            (NumberValue::Int { int: i , ..}, t @ Type::U16 | t @ Type::I16) =>
                NumberValue::from((cg.builder.build_int_cast_sign_flag(i, cg.i16, t.is_signed(), &reg_name)?, t.is_signed())),
            (NumberValue::Int { int: i , ..}, t @ Type::U32 | t @ Type::I32) =>
                NumberValue::from((cg.builder.build_int_cast_sign_flag(i, cg.i32, t.is_signed(), &reg_name)?, t.is_signed())),
            (NumberValue::Int { int: i , signed: false}, Type::F32) =>
                NumberValue::from(cg.builder.build_unsigned_int_to_float(i, cg.f32, &reg_name)?),
            (NumberValue::Int { int: i , signed: true}, Type::F32) =>
                NumberValue::from(cg.builder.build_signed_int_to_float(i, cg.f32, &reg_name)?),
            (NumberValue::Int { int: i , signed: false}, Type::F64) =>
                NumberValue::from(cg.builder.build_unsigned_int_to_float(i, cg.f64, &reg_name)?),
            (NumberValue::Int { int: i , signed: true}, Type::F64) =>
                NumberValue::from(cg.builder.build_signed_int_to_float(i, cg.f64, &reg_name)?),
            (NumberValue::Float(f), Type::F32) =>
                NumberValue::from(cg.builder.build_float_cast(f, cg.f32, &reg_name)?),
            (NumberValue::Float(f), Type::F64) =>
                NumberValue::from(cg.builder.build_float_cast(f, cg.f64, &reg_name)?),
            (NumberValue::Float(f), ty) => {
                let mod_name = format!("CONVINT::{}", output.id);
                let int = cg.builder.build_float_to_signed_int(f, cg.i32, &mod_name)?;
                
                return ty.conv(cg, NumberValue::Int { int, signed: true }, output);
            }
        };
        cg.put(output, v);
        Ok(())
    }
    

    fn as_llvm<'this, 'ctx>(&'this self, cg: &mut crate::jit::Codegen<'ctx>) -> NumberType<'ctx> {
        match self {
            Type::U1 => NumberType::Int { ty: cg.i1, signed: false },
            Type::U8 => NumberType::Int { ty: cg.i8, signed: false },
            Type::I8 => NumberType::Int { ty: cg.i8, signed: true },
            Type::U16 => NumberType::Int { ty: cg.i16, signed: false },
            Type::I16 => NumberType::Int { ty: cg.i16, signed: true },
            Type::U32 => NumberType::Int { ty: cg.i32, signed: false },
            Type::I32 => NumberType::Int { ty: cg.i32, signed: true },
            Type::F32 => NumberType::Float(cg.f32),
            Type::F64 => NumberType::Float(cg.f64),
        }
    }
}