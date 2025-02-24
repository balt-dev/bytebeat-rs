
use std::{error::Error, io::{stdin, Read}, process::ExitCode, sync::atomic::{AtomicBool, Ordering}};

use bytebeat_rs::Context;
use clap::Parser;
use cpal::{traits::{DeviceTrait, HostTrait as _, StreamTrait}, SampleFormat};

/// Program to play bytebeats over the speaker
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Sample rate to play the bytebeat at
    #[arg(short, long, default_value_t = 8000)]
    sample_rate: u32,

    #[arg(required = false)]
    /// Beat to play - if not supplied, will take from stdin
    beat: Option<String>,
}

fn main() -> ExitCode {
    match main_() {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

macro_rules! zst_error {
    ($($name: ident => $msg: literal),*) => {$(
        #[derive(Debug, Copy, Clone)] struct $name;
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, $msg) }
        }
        impl std::error::Error for $name {}
    )*};
}

zst_error! {
    NoDevice => "no devices found",
    NoSupportedStream => "no devices found that support unsigned 8-bit audio data"
}

static RUNNING: AtomicBool = AtomicBool::new(true);

fn main_() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let beat = match args.beat {
        Some(v) => v,
        None => {
            let mut buf = Vec::new();
            stdin().read_to_end(&mut buf)?;
            String::from_utf8_lossy(&buf).into()
        }
    };

    // Connect to audio device
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(NoDevice)?;
    let supported_configs = device.supported_output_configs()?;
    let mut conf = None;
    for config in supported_configs {
        if config.sample_format() == SampleFormat::U8 { conf = Some(config); break; }
    }
    let conf_range = conf.ok_or(NoSupportedStream)?;
    let conf = conf_range.with_max_sample_rate();
    let sample_rate = conf.sample_rate().0;
    let dilation = args.sample_rate as f64 / sample_rate as f64;

    // This should exist for the rest of the program
    let ctx = Box::leak(Box::new(Context::create()));
    let func = bytebeat_rs::compile(ctx, &beat)?;

    let mut t = 0i32;

    ctrlc::set_handler(move || {
        RUNNING.store(false, Ordering::SeqCst);
    })?;

    let stream = device.build_output_stream(
        &conf.config(),
        move |data: &mut [u8], _| {
            for sample in data {
                *sample = unsafe { func.call((t as f64 * dilation).trunc()) };
                t = t.wrapping_add(1);
            }
        }, 
        |err| {
            eprintln!("audio error: {err}");
        },
        None
    )?;

    stream.play()?;

    while RUNNING.load(Ordering::SeqCst) {
        std::hint::spin_loop();
    }

    Ok(())
}