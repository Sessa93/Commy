use std::error::Error;
use std::fmt;

use crate::bus::{C64Bus, MemoryAccessError, RomLoadError, RomRegion};
use crate::cpu::{Cpu6510, CpuError};
use crate::vic::RasterState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadPrgError {
    MissingLoadAddress,
    ProgramTooLarge { load_address: u16, len: usize },
}

impl fmt::Display for LoadPrgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLoadAddress => write!(f, "PRG is missing its two-byte load address"),
            Self::ProgramTooLarge { load_address, len } => write!(
                f,
                "PRG does not fit in RAM: load=${load_address:04X}, payload_len={len}"
            ),
        }
    }
}

impl Error for LoadPrgError {}

pub struct Commodore64 {
    pub cpu: Cpu6510,
    pub bus: C64Bus,
    pub cycles: u64,
}

impl Default for Commodore64 {
    fn default() -> Self {
        let mut bus = C64Bus::new();
        bus.set_reset_vector(0x0000);

        Self {
            cpu: Cpu6510::new(),
            bus,
            cycles: 0,
        }
    }
}

impl Commodore64 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
        self.cycles = 0;
    }

    pub fn load_rom(&mut self, region: RomRegion, bytes: &[u8]) -> Result<(), RomLoadError> {
        self.bus.load_rom(region, bytes)
    }

    pub fn load_prg(&mut self, bytes: &[u8]) -> Result<u16, LoadPrgError> {
        if bytes.len() < 2 {
            return Err(LoadPrgError::MissingLoadAddress);
        }

        let load_address = u16::from_le_bytes([bytes[0], bytes[1]]);
        let payload = &bytes[2..];

        self.bus
            .load_bytes(load_address, payload)
            .map_err(|MemoryAccessError { .. }| LoadPrgError::ProgramTooLarge {
                load_address,
                len: payload.len(),
            })?;

        self.bus.set_reset_vector(load_address);
        self.cpu.pc = load_address;
        Ok(load_address)
    }

    pub fn step(&mut self) -> Result<u8, CpuError> {
        let cycles = self.cpu.step(&mut self.bus)?;
        self.bus.tick(cycles);
        self.cycles += cycles as u64;
        Ok(cycles)
    }

    pub fn run_steps(&mut self, steps: usize) -> Result<u64, CpuError> {
        for _ in 0..steps {
            if self.cpu.stopped {
                break;
            }
            self.step()?;
        }

        Ok(self.cycles)
    }

    pub fn peek_ram(&self, addr: u16) -> u8 {
        self.bus.peek_ram(addr)
    }

    pub fn raster_state(&self) -> RasterState {
        self.bus.raster_state()
    }

    pub fn screen_text(&self) -> String {
        self.bus.screen_text()
    }

    pub fn current_reset_vector(&self) -> u16 {
        self.bus.current_reset_vector()
    }
}

#[cfg(test)]
mod tests {
    use super::Commodore64;
    use crate::RomRegion;

    #[test]
    fn prg_load_sets_reset_vector_and_ram_contents() {
        let mut c64 = Commodore64::new();
        let load_address = c64.load_prg(&[0x01, 0x08, 0xA9, 0x42, 0x00]).unwrap();

        assert_eq!(load_address, 0x0801);
        assert_eq!(c64.bus.reset_vector(), 0x0801);
        assert_eq!(c64.peek_ram(0x0801), 0xA9);
        assert_eq!(c64.peek_ram(0x0802), 0x42);
        assert_eq!(c64.peek_ram(0x0803), 0x00);
    }

    #[test]
    fn built_program_runs_until_brk() {
        let mut c64 = Commodore64::new();
        c64.load_prg(&[0x01, 0x08, 0xA9, 0x07, 0x8D, 0x00, 0x04, 0x00])
            .unwrap();
        c64.reset();

        c64.run_steps(8).unwrap();

        assert!(c64.cpu.stopped);
        assert_eq!(c64.peek_ram(0x0400), 0x07);
    }

    #[test]
    fn machine_exposes_vic_screen_snapshot() {
        let mut c64 = Commodore64::new();
        c64.load_prg(&[
            0x01, 0x08, 0xA9, 0x08, 0x8D, 0x00, 0x04, 0xA9, 0x09, 0x8D, 0x01, 0x04, 0x00,
        ])
        .unwrap();
        c64.reset();

        c64.run_steps(8).unwrap();

        assert!(c64.screen_text().starts_with("HI"));
        assert!(c64.raster_state().line > 0 || c64.raster_state().cycle > 0);
    }

    #[test]
    fn reset_uses_mapped_kernal_vector_when_rom_is_loaded() {
        let mut c64 = Commodore64::new();
        let mut kernal = vec![0; 0x2000];
        kernal[0x1FFC] = 0x00;
        kernal[0x1FFD] = 0xE0;

        c64.load_prg(&[0x01, 0x08, 0x00]).unwrap();
        c64.load_rom(RomRegion::Kernal, &kernal).unwrap();
        c64.reset();

        assert_eq!(c64.current_reset_vector(), 0xE000);
        assert_eq!(c64.cpu.pc, 0xE000);
    }
}