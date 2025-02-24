
use std::sync::atomic::{AtomicBool, Ordering};

use bytebeat_rs::{ir, parser, tokens, JitError};
use rayon::prelude::*;

macro_rules! sexpr_to_rust {
    (($($tt: tt)*)) => { sexpr_to_rust!($($tt)*) };
    ($tt: tt) => { $tt };
    ($ty: ident: $($val: tt)*) => { sexpr_to_rust! ($($val)*) as $ty };
    ($opr: tt $l: tt $r: tt) => { sexpr_to_rust! ($l) $opr sexpr_to_rust! ($r) };
    ($opr: tt $v: tt) => { $opr sexpr_to_rust! ($v) };
    (?: $cond: tt $yes: tt $no: tt) => { if (sexpr_to_rust! ($cond) as i32) != 0 { sexpr_to_rust! ($yes) as i32 } else { sexpr_to_rust! ($no) as i32 } };
}

#[test]
fn test_42() -> Result<(), JitError> {

    let beat = "(t&8192?t&4096?t&1024?2*t:4*t:t&512?4*t:4.2*t:(t&4096?t&1024?2*t:10*t:t&512?2*t:8*t)>>2)*(t&16384?3:2)|t*(t&16384?1/8:1/(.01*t))";
    let mut toks = tokens::parse_tokens(beat);
    let expr = parser::parse(&mut toks)?;
    eprintln!("{expr}");
    let insts = ir::CodeGenerator::new().generate_ir(expr)?;
    println!("IR: ");
    for inst in &insts {
        println!("{inst}")
    }
    println!("---");

    /*
    let ctx = bytebeat_rs::Context::create();

    let poison = AtomicBool::new(false);

    let func = bytebeat_rs::compile(&ctx, beat)?;
    (i32::MIN ..= i32::MAX).into_par_iter().for_each(|t| {
        if poison.load(Ordering::SeqCst) { return; }
        let actual = std::hint::black_box(unsafe { func.call(t as f64) });
        let expected = 
            sexpr_to_rust!(?: (?: (?: (?: (?: (> 131072 (% t 262144)) (| (& (& (>> (/ t 64) 3) (* 2 t)) (* 10 t)) (& (& (>> t 5) (* 6 t)) (| (>> t 4) (>> t 5)))) (& (< 131072 (% t 262144)) (> 163840 (% t 262144)))) (| (& (& (>> t 4) (* 8 t)) (| (>> t 5) (>> t 4))) (& (* 3 t) (* 10 t))) (& (< 163840 (% t 262144)) (> 196608 (% t 262144)))) (| (& (& (>> t 4) (* 8 t)) (| (>> t 5) (>> t 4))) (& (* 3 t) (* 6 t))) (& (< 196608 (% t 262144)) (> 229376 (% t 262144)))) (| (& (& (>> t 4) (* 8 t)) (| (>> t 5) (>> t 4))) (& (* 4 t) (* 6 t))) (& (< 229376 (% t 262144)) (> 245760 (% t 262144)))) (| (& (& (>> t 4) (* 8 t)) (| (>> t 5) (>> t 4))) (& (* 4 t) (* 2 t))) (| (& (& (>> t 4) (* 8 t)) (>> t 4)) (>> (& (* 4 t) (* 2 t)) 20)))
            as u8;
        if actual != expected {
            poison.store(true, Ordering::SeqCst);
            panic!("deviated at t={t} - expected {expected}, got {actual}");
        }
        if t % 0x100_0000 == 0 {
            eprintln!("at {t:02x}");
        }
    }); */
    Ok(())


}