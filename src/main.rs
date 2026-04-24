use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use commy::{Commodore64, RomRegion};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut steps = 32usize;
    let mut prg_path: Option<PathBuf> = None;
    let mut basic_rom_path: Option<PathBuf> = None;
    let mut kernal_rom_path: Option<PathBuf> = None;
    let mut char_rom_path: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--steps" => {
                let value = args.next().ok_or("missing value for --steps")?;
                steps = value.parse()?;
            }
            "--basic-rom" => {
                basic_rom_path = Some(PathBuf::from(
                    args.next().ok_or("missing value for --basic-rom")?,
                ));
            }
            "--kernal-rom" => {
                kernal_rom_path = Some(PathBuf::from(
                    args.next().ok_or("missing value for --kernal-rom")?,
                ));
            }
            "--char-rom" => {
                char_rom_path = Some(PathBuf::from(
                    args.next().ok_or("missing value for --char-rom")?,
                ));
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            _ => {
                if prg_path.is_some() {
                    return Err("only one PRG path can be provided".into());
                }
                prg_path = Some(PathBuf::from(arg));
            }
        }
    }

    let mut c64 = Commodore64::new();
    load_system_rom(&mut c64, RomRegion::Basic, basic_rom_path.as_ref())?;
    load_system_rom(&mut c64, RomRegion::Kernal, kernal_rom_path.as_ref())?;
    load_system_rom(&mut c64, RomRegion::Character, char_rom_path.as_ref())?;

    let roms_loaded = basic_rom_path.is_some() || kernal_rom_path.is_some() || char_rom_path.is_some();
    let load_address = if let Some(path) = prg_path {
        let bytes = fs::read(&path)?;
        let load_address = c64.load_prg(&bytes)?;
        println!("loaded {} at ${load_address:04X}", path.display());
        Some(load_address)
    } else if roms_loaded {
        None
    } else {
        let load_address = c64.load_prg(&demo_prg())?;
        println!("loaded built-in demo at ${load_address:04X}");
        Some(load_address)
    };

    c64.reset();
    let cycles = c64.run_steps(steps)?;

    println!("{}", c64.cpu.state());
    println!("reset vector=${:04X}", c64.current_reset_vector());
    println!("cycles={cycles}");
    let raster = c64.raster_state();
    println!(
        "vic raster: line={} cycle={} frame={}",
        raster.line, raster.cycle, raster.frame
    );
    let cia1 = c64.cia1_snapshot();
    let cia2 = c64.cia2_snapshot();
    let sid = c64.sid_snapshot();
    println!(
        "cia1: cycles={} timer_a={} timer_b={} running_a={} running_b={} irq_pending={} mask=${:02X}",
        cia1.total_cycles,
        cia1.timer_a,
        cia1.timer_b,
        cia1.timer_a_running,
        cia1.timer_b_running,
        cia1.irq_pending,
        cia1.interrupt_mask
    );
    println!(
        "cia2: cycles={} timer_a={} timer_b={} running_a={} running_b={} nmi_pending={} mask=${:02X}",
        cia2.total_cycles,
        cia2.timer_a,
        cia2.timer_b,
        cia2.timer_a_running,
        cia2.timer_b_running,
        cia2.irq_pending,
        cia2.interrupt_mask
    );
    println!(
        "sid: cycles={} voice_phase=[{}, {}, {}]",
        sid.total_cycles, sid.voice_phase[0], sid.voice_phase[1], sid.voice_phase[2]
    );
    if let Some(load_address) = load_address {
        println!("loaded bytes:");
        dump_window(&c64, load_address, 8);
    }
    println!("screen RAM:");
    dump_window(&c64, 0x0400, 8);
    println!("screen snapshot:");
    for line in c64.screen_text().lines().take(4) {
        println!("{line}");
    }

    Ok(())
}

fn print_usage() {
    println!("Usage: cargo run -- [path/to/program.prg] [--steps N] [--basic-rom PATH] [--kernal-rom PATH] [--char-rom PATH]");
}

fn load_system_rom(
    c64: &mut Commodore64,
    region: RomRegion,
    path: Option<&PathBuf>,
) -> Result<(), Box<dyn Error>> {
    if let Some(path) = path {
        let bytes = fs::read(path)?;
        c64.load_rom(region, &bytes)?;
        println!("loaded {:?} ROM from {}", region, path.display());
    }

    Ok(())
}

fn demo_prg() -> Vec<u8> {
    vec![
        0x01, 0x08, 0xA9, 0x01, 0x8D, 0x00, 0x04, 0xA9, 0x02, 0x8D, 0x01, 0x04, 0x00,
    ]
}

fn dump_window(c64: &Commodore64, start: u16, len: usize) {
    for offset in 0..len {
        let addr = start.wrapping_add(offset as u16);
        println!("${addr:04X}: ${:02X}", c64.peek_ram(addr));
    }
}