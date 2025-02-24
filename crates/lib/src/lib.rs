use inkwell::{builder::BuilderError, context::Context as Ctx, OptimizationLevel};
use ir::CompilationError;
use jit::TimeFunc;
use tokens::ParsingError;


pub mod parser;
pub mod tokens;
pub mod ir;
pub mod jit;

#[derive(Debug)]
pub enum JitError {
    Parsing(ParsingError),
    Compilation(CompilationError),
    Builder(BuilderError)
}

impl From<ParsingError> for JitError { fn from(value: ParsingError) -> Self { Self::Parsing(value) }}
impl From<CompilationError> for JitError { fn from(value: CompilationError) -> Self { Self::Compilation(value) }}
impl From<BuilderError> for JitError { fn from(value: BuilderError) -> Self { Self::Builder(value) }}

impl std::fmt::Display for JitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parsing(b) => b.fmt(f),
            Self::Compilation(b) => b.fmt(f),
            Self::Builder(b) => b.fmt(f),
        }
    }
}

impl std::error::Error for JitError {}

pub use inkwell::context::Context;

pub fn compile<'ctx>(ctx: &'ctx Ctx, beat: &str) -> Result<TimeFunc<'ctx>, JitError> {
    let mut toks = tokens::parse_tokens(beat);
    let expr = parser::parse(&mut toks)?;
    let insts = ir::CodeGenerator::new().generate_ir(expr)?;
    let func = jit::jit(&ctx, insts, OptimizationLevel::Aggressive)?;
    Ok(func)
}