use std::error::Error;
use std::fmt;

use crate::cia::{Cia6526, CiaSnapshot};
use crate::sid::{Sid6581, SidSnapshot};
use crate::vic::{RasterState, VicII};

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, value: u8);

    fn poll_nmi(&mut self) -> bool {
        false
    }

    fn poll_irq(&mut self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomRegion {
    Basic,
    Kernal,
    Character,
}

impl RomRegion {
    fn expected_len(self) -> usize {
        match self {
            Self::Basic | Self::Kernal => 0x2000,
            Self::Character => 0x1000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomLoadError {
    pub region: RomRegion,
    pub expected: usize,
    pub actual: usize,
}

impl fmt::Display for RomLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid {:?} ROM size: expected {} bytes, got {} bytes",
            self.region, self.expected, self.actual
        )
    }
}

impl Error for RomLoadError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryAccessError {
    pub start: u16,
    pub len: usize,
}

impl fmt::Display for MemoryAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "memory write out of range: start=${:04X}, len={}",
            self.start, self.len
        )
    }
}

impl Error for MemoryAccessError {}

pub struct C64Bus {
    ram: [u8; 0x10000],
    color_ram: [u8; 0x0400],
    basic_rom: Option<Vec<u8>>,
    kernal_rom: Option<Vec<u8>>,
    char_rom: Option<Vec<u8>>,
    vic: VicII,
    sid: Sid6581,
    cia1: Cia6526,
    cia2: Cia6526,
    cpu_port_ddr: u8,
    cpu_port: u8,
}

impl Default for C64Bus {
    fn default() -> Self {
        let mut ram = [0; 0x10000];
        ram[0x0000] = 0x2F;
        ram[0x0001] = 0x37;

        Self {
            ram,
            color_ram: [0; 0x0400],
            basic_rom: None,
            kernal_rom: None,
            char_rom: None,
            vic: VicII::new(),
            sid: Sid6581::new(),
            cia1: Cia6526::new(),
            cia2: Cia6526::new(),
            cpu_port_ddr: 0x2F,
            cpu_port: 0x37,
        }
    }
}

impl C64Bus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_rom(&mut self, region: RomRegion, bytes: &[u8]) -> Result<(), RomLoadError> {
        let expected = region.expected_len();
        if bytes.len() != expected {
            return Err(RomLoadError {
                region,
                expected,
                actual: bytes.len(),
            });
        }

        let rom = bytes.to_vec();
        match region {
            RomRegion::Basic => self.basic_rom = Some(rom),
            RomRegion::Kernal => self.kernal_rom = Some(rom),
            RomRegion::Character => self.char_rom = Some(rom),
        }

        Ok(())
    }

    pub fn load_bytes(&mut self, start: u16, bytes: &[u8]) -> Result<(), MemoryAccessError> {
        let start_index = start as usize;
        let end_index = start_index.saturating_add(bytes.len());
        if end_index > self.ram.len() {
            return Err(MemoryAccessError {
                start,
                len: bytes.len(),
            });
        }

        self.ram[start_index..end_index].copy_from_slice(bytes);
        Ok(())
    }

    pub fn set_reset_vector(&mut self, addr: u16) {
        let [low, high] = addr.to_le_bytes();
        self.ram[0xFFFC] = low;
        self.ram[0xFFFD] = high;
    }

    pub fn reset_vector(&self) -> u16 {
        u16::from_le_bytes([self.ram[0xFFFC], self.ram[0xFFFD]])
    }

    pub fn current_reset_vector(&self) -> u16 {
        u16::from_le_bytes([self.read_internal(0xFFFC), self.read_internal(0xFFFD)])
    }

    pub fn peek_ram(&self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }

    pub fn peek_mapped(&self, addr: u16) -> u8 {
        self.read_internal(addr)
    }

    pub fn set_cpu_port(&mut self, value: u8) {
        self.cpu_port = value;
    }

    pub fn tick(&mut self, cycles: u8) {
        self.vic.tick(cycles);
        self.sid.tick(cycles);
        self.cia1.tick(cycles);
        self.cia2.tick(cycles);
    }

    pub fn raster_state(&self) -> RasterState {
        self.vic.raster_state()
    }

    pub fn screen_text(&self) -> String {
        self.vic.render_text_screen(&self.ram)
    }

    pub fn sid_snapshot(&self) -> SidSnapshot {
        self.sid.snapshot()
    }

    pub fn cia1_snapshot(&self) -> CiaSnapshot {
        self.cia1.snapshot()
    }

    pub fn cia2_snapshot(&self) -> CiaSnapshot {
        self.cia2.snapshot()
    }

    fn basic_visible(&self) -> bool {
        let loram = self.cpu_port & 0b001 != 0;
        let hiram = self.cpu_port & 0b010 != 0;
        loram && hiram
    }

    fn kernal_visible(&self) -> bool {
        self.cpu_port & 0b010 != 0
    }

    fn io_visible(&self) -> bool {
        self.cpu_port & 0b100 != 0
    }

    fn read_internal(&self, addr: u16) -> u8 {
        match addr {
            0x0000 => self.cpu_port_ddr,
            0x0001 => self.cpu_port,
            0xA000..=0xBFFF if self.basic_visible() => self
                .basic_rom
                .as_ref()
                .map(|rom| rom[(addr - 0xA000) as usize])
                .unwrap_or(self.ram[addr as usize]),
            0xD000..=0xDFFF => {
                if self.io_visible() {
                    self.read_io(addr)
                } else {
                    self.char_rom
                        .as_ref()
                        .map(|rom| rom[(addr - 0xD000) as usize])
                        .unwrap_or(self.ram[addr as usize])
                }
            }
            0xE000..=0xFFFF if self.kernal_visible() => self
                .kernal_rom
                .as_ref()
                .map(|rom| rom[(addr - 0xE000) as usize])
                .unwrap_or(self.ram[addr as usize]),
            _ => self.ram[addr as usize],
        }
    }

    fn read_io(&self, addr: u16) -> u8 {
        match addr {
            0xD000..=0xD3FF => self.vic.read(addr),
            0xD400..=0xD7FF => self.sid.read(addr),
            0xD800..=0xDBFF => self.color_ram[(addr - 0xD800) as usize] & 0x0F,
            0xDC00..=0xDCFF => self.cia1.read(addr),
            0xDD00..=0xDDFF => self.cia2.read(addr),
            _ => self.ram[addr as usize],
        }
    }

    fn write_io(&mut self, addr: u16, value: u8) {
        match addr {
            0xD000..=0xD3FF => self.vic.write(addr, value),
            0xD400..=0xD7FF => self.sid.write(addr, value),
            0xD800..=0xDBFF => self.color_ram[(addr - 0xD800) as usize] = value & 0x0F,
            0xDC00..=0xDCFF => self.cia1.write(addr, value),
            0xDD00..=0xDDFF => self.cia2.write(addr, value),
            _ => self.ram[addr as usize] = value,
        }
    }
}

impl Bus for C64Bus {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0xDC00..=0xDCFF if self.io_visible() => self.cia1.read_mut(addr),
            0xDD00..=0xDDFF if self.io_visible() => self.cia2.read_mut(addr),
            _ => self.read_internal(addr),
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000 => self.cpu_port_ddr = value,
            0x0001 => self.cpu_port = value,
            0xD000..=0xDFFF if self.io_visible() => self.write_io(addr, value),
            _ => self.ram[addr as usize] = value,
        }
    }

    fn poll_irq(&mut self) -> bool {
        self.cia1.irq_pending()
    }

    fn poll_nmi(&mut self) -> bool {
        self.cia2.take_irq()
    }
}

#[cfg(test)]
mod tests {
    use super::{Bus, C64Bus, RomRegion};

    #[test]
    fn basic_rom_is_visible_only_when_banked_in() {
        let mut bus = C64Bus::new();
        bus.load_bytes(0xA000, &[0x55]).unwrap();
        bus.load_rom(RomRegion::Basic, &[0xAA; 0x2000]).unwrap();

        assert_eq!(bus.read(0xA000), 0xAA);

        bus.set_cpu_port(0x34);
        assert_eq!(bus.read(0xA000), 0x55);
    }

    #[test]
    fn io_writes_land_in_device_space_when_enabled() {
        let mut bus = C64Bus::new();
        bus.write(0xD800, 0xFF);

        assert_eq!(bus.peek_mapped(0xD800), 0x0F);
        assert_eq!(bus.peek_ram(0xD800), 0x00);
    }

    #[test]
    fn vic_raster_advances_with_bus_ticks() {
        let mut bus = C64Bus::new();

        bus.tick(63);

        assert_eq!(bus.raster_state().line, 1);
        assert_eq!(bus.read(0xD012), 1);
    }

    #[test]
    fn mapped_reset_vector_comes_from_kernal_rom_when_present() {
        let mut bus = C64Bus::new();
        let mut kernal = vec![0; 0x2000];
        kernal[0x1FFC] = 0x34;
        kernal[0x1FFD] = 0x12;

        bus.set_reset_vector(0x0801);
        bus.load_rom(RomRegion::Kernal, &kernal).unwrap();

        assert_eq!(bus.reset_vector(), 0x0801);
        assert_eq!(bus.current_reset_vector(), 0x1234);
    }

    #[test]
    fn cia_timer_registers_advance_with_bus_ticks() {
        let mut bus = C64Bus::new();
        bus.write(0xDC04, 0x03);
        bus.write(0xDC05, 0x00);
        bus.write(0xDC0E, 0x11);

        bus.tick(2);

        assert_eq!(bus.read(0xDC04), 0x01);
        assert_eq!(bus.cia1_snapshot().total_cycles, 2);
    }

    #[test]
    fn sid_phase_advances_with_bus_ticks() {
        let mut bus = C64Bus::new();
        bus.write(0xD400, 0x34);
        bus.write(0xD401, 0x12);

        bus.tick(2);

        assert_eq!(bus.sid_snapshot().total_cycles, 2);
        assert_eq!(bus.sid_snapshot().voice_phase[0], 0x1234 * 2);
    }
}