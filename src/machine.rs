use std::error::Error;
use std::fmt;

use crate::bus::{C64Bus, MemoryAccessError, RomLoadError, RomRegion};
use crate::cia::CiaSnapshot;
use crate::cpu::{Cpu6510, CpuError};
use crate::sid::SidSnapshot;
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

    pub fn sid_snapshot(&self) -> SidSnapshot {
        self.bus.sid_snapshot()
    }

    pub fn cia1_snapshot(&self) -> CiaSnapshot {
        self.bus.cia1_snapshot()
    }

    pub fn cia2_snapshot(&self) -> CiaSnapshot {
        self.bus.cia2_snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::Commodore64;
    use crate::{Bus, RomRegion};

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

    #[test]
    fn machine_ticks_cia_and_sid_alongside_vic() {
        let mut c64 = Commodore64::new();
        c64.bus.write(0xDC04, 0x02);
        c64.bus.write(0xDC05, 0x00);
        c64.bus.write(0xDC0E, 0x11);
        c64.bus.write(0xD400, 0x34);
        c64.bus.write(0xD401, 0x12);

        c64.bus.tick(2);

        assert_eq!(c64.cia1_snapshot().total_cycles, 2);
        assert_eq!(c64.sid_snapshot().voice_phase[0], 0x1234 * 2);
    }

    #[test]
    fn kernal_rom_reset_handler_can_boot_and_write_to_screen() {
        let mut c64 = Commodore64::new();
        let mut kernal = vec![0; 0x2000];

        kernal[0x0000..0x0010].copy_from_slice(&[
            0xA2, 0x00, 0xBD, 0x10, 0xE0, 0x9D, 0x00, 0x04, 0xE8, 0xE0, 0x06, 0xD0, 0xF5, 0x00,
            0xEA, 0xEA,
        ]);
        kernal[0x0010..0x0016].copy_from_slice(&[0x12, 0x05, 0x01, 0x04, 0x19, 0x2E]);
        kernal[0x1FFC] = 0x00;
        kernal[0x1FFD] = 0xE0;

        c64.load_rom(RomRegion::Kernal, &kernal).unwrap();
        c64.reset();
        c64.run_steps(64).unwrap();

        assert!(c64.cpu.stopped);
        assert!(c64.screen_text().starts_with("READY."));
        assert_eq!(c64.current_reset_vector(), 0xE000);
    }

    #[test]
    fn kernal_rom_reset_handler_can_copy_through_zero_page_pointers() {
        let mut c64 = Commodore64::new();
        let mut kernal = vec![0; 0x2000];

        kernal[0x0000..0x0015].copy_from_slice(&[
            0x20, 0x20, 0xE0, 0xA0, 0x00, 0xB1, 0xFB, 0x91, 0xFD, 0xC8, 0xC0, 0x06, 0xD0, 0xF7,
            0x00, 0xEA, 0xEA, 0xEA, 0xEA, 0xEA, 0xEA,
        ]);
        kernal[0x0020..0x0031].copy_from_slice(&[
            0xA9, 0x40, 0x85, 0xFB, 0xA9, 0xE0, 0x85, 0xFC, 0xA9, 0x00, 0x85, 0xFD, 0xA9, 0x04,
            0x85, 0xFE, 0x60,
        ]);
        kernal[0x0040..0x0046].copy_from_slice(&[0x12, 0x05, 0x01, 0x04, 0x19, 0x2E]);
        kernal[0x1FFC] = 0x00;
        kernal[0x1FFD] = 0xE0;

        c64.load_rom(RomRegion::Kernal, &kernal).unwrap();
        c64.reset();
        c64.run_steps(96).unwrap();

        assert!(c64.cpu.stopped);
        assert!(c64.screen_text().starts_with("READY."));
        assert_eq!(c64.peek_ram(0x0405), 0x2E);
    }

    #[test]
    fn cia_timer_irq_reaches_kernal_handler() {
        let mut c64 = Commodore64::new();
        let mut kernal = vec![0; 0x2000];

        kernal[0x0000..0x001D].copy_from_slice(&[
            0xA9, 0x03, 0x8D, 0x04, 0xDC, 0xA9, 0x00, 0x8D, 0x05, 0xDC, 0xA9, 0x81, 0x8D, 0x0D,
            0xDC, 0xA9, 0x19, 0x8D, 0x0E, 0xDC, 0x58, 0xAD, 0x00, 0x04, 0xC9, 0x09, 0xD0, 0xF9,
            0x00,
        ]);
        kernal[0x001D..0x0020].copy_from_slice(&[0xEA, 0xEA, 0xEA]);
        kernal[0x0020..0x0029].copy_from_slice(&[0xAD, 0x0D, 0xDC, 0xA9, 0x09, 0x8D, 0x00, 0x04, 0x40]);
        kernal[0x1FFC] = 0x00;
        kernal[0x1FFD] = 0xE0;
        kernal[0x1FFE] = 0x20;
        kernal[0x1FFF] = 0xE0;

        c64.load_rom(RomRegion::Kernal, &kernal).unwrap();
        c64.reset();
        c64.run_steps(64).unwrap();

        assert!(c64.cpu.stopped);
        assert_eq!(c64.peek_ram(0x0400), 0x09);
        assert!(!c64.cia1_snapshot().irq_pending);
    }

    #[test]
    fn cia2_timer_nmi_reaches_kernal_handler_even_with_interrupts_disabled() {
        let mut c64 = Commodore64::new();
        let mut kernal = vec![0; 0x2000];

        kernal[0x0000..0x001C].copy_from_slice(&[
            0xA9, 0x03, 0x8D, 0x04, 0xDD, 0xA9, 0x00, 0x8D, 0x05, 0xDD, 0xA9, 0x81, 0x8D, 0x0D,
            0xDD, 0xA9, 0x19, 0x8D, 0x0E, 0xDD, 0xAD, 0x01, 0x04, 0xC9, 0x0A, 0xD0, 0xF9, 0x00,
        ]);
        kernal[0x0020..0x0029].copy_from_slice(&[0xAD, 0x0D, 0xDD, 0xA9, 0x0A, 0x8D, 0x01, 0x04, 0x40]);
        kernal[0x1FFA] = 0x20;
        kernal[0x1FFB] = 0xE0;
        kernal[0x1FFC] = 0x00;
        kernal[0x1FFD] = 0xE0;

        c64.load_rom(RomRegion::Kernal, &kernal).unwrap();
        c64.reset();
        c64.run_steps(64).unwrap();

        assert!(c64.cpu.stopped);
        assert_eq!(c64.peek_ram(0x0401), 0x0A);
        assert!(!c64.cia2_snapshot().irq_pending);
    }

    #[test]
    fn vic_raster_irq_reaches_kernal_handler() {
        let mut c64 = Commodore64::new();
        let mut kernal = vec![0; 0x2000];

        kernal[0x0000..0x0017].copy_from_slice(&[
            0xA9, 0x01, 0x8D, 0x12, 0xD0, 0xA9, 0x01, 0x8D, 0x1A, 0xD0, 0x58, 0xAD, 0x03, 0x04,
            0xC9, 0x0C, 0xD0, 0xF9, 0x00, 0xEA, 0xEA, 0xEA, 0xEA,
        ]);
        kernal[0x0020..0x002C].copy_from_slice(&[
            0xAD, 0x19, 0xD0, 0x8D, 0x19, 0xD0, 0xA9, 0x0C, 0x8D, 0x03, 0x04, 0x40,
        ]);
        kernal[0x1FFC] = 0x00;
        kernal[0x1FFD] = 0xE0;
        kernal[0x1FFE] = 0x20;
        kernal[0x1FFF] = 0xE0;

        c64.load_rom(RomRegion::Kernal, &kernal).unwrap();
        c64.reset();
        c64.run_steps(96).unwrap();

        assert!(c64.cpu.stopped);
        assert_eq!(c64.peek_ram(0x0403), 0x0C);
        assert_eq!(c64.bus.read(0xD019) & 0x01, 0x00);
    }
}