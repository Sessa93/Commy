use std::error::Error;
use std::fmt;

use crate::bus::Bus;

const FLAG_CARRY: u8 = 0b0000_0001;
const FLAG_ZERO: u8 = 0b0000_0010;
const FLAG_INTERRUPT_DISABLE: u8 = 0b0000_0100;
const FLAG_DECIMAL: u8 = 0b0000_1000;
const FLAG_BREAK: u8 = 0b0001_0000;
const FLAG_UNUSED: u8 = 0b0010_0000;
const FLAG_OVERFLOW: u8 = 0b0100_0000;
const FLAG_NEGATIVE: u8 = 0b1000_0000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpuError {
    UnsupportedOpcode { opcode: u8, pc: u16 },
}

impl fmt::Display for CpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedOpcode { opcode, pc } => {
                write!(f, "unsupported opcode ${opcode:02X} at ${pc:04X}")
            }
        }
    }
}

impl Error for CpuError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuState {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub status: u8,
    pub stopped: bool,
}

impl fmt::Display for CpuState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PC=${:04X} A=${:02X} X=${:02X} Y=${:02X} SP=${:02X} P=${:02X} stopped={}",
            self.pc, self.a, self.x, self.y, self.sp, self.status, self.stopped
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cpu6510 {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub status: u8,
    pub stopped: bool,
}

impl Default for Cpu6510 {
    fn default() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0,
            status: FLAG_INTERRUPT_DISABLE | FLAG_UNUSED,
            stopped: false,
        }
    }
}

impl Cpu6510 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset<B: Bus>(&mut self, bus: &mut B) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.status = FLAG_INTERRUPT_DISABLE | FLAG_UNUSED;
        self.stopped = false;
        self.pc = self.read_u16(bus, 0xFFFC);
    }

    pub fn state(&self) -> CpuState {
        CpuState {
            a: self.a,
            x: self.x,
            y: self.y,
            sp: self.sp,
            pc: self.pc,
            status: self.status,
            stopped: self.stopped,
        }
    }

    pub fn step<B: Bus>(&mut self, bus: &mut B) -> Result<u8, CpuError> {
        if self.stopped {
            return Ok(0);
        }

        if bus.poll_nmi() {
            self.service_interrupt(bus, 0xFFFA, false);
            return Ok(7);
        }

        if self.status & FLAG_INTERRUPT_DISABLE == 0 && bus.poll_irq() {
            self.service_interrupt(bus, 0xFFFE, false);
            return Ok(7);
        }

        let opcode_pc = self.pc;
        let opcode = self.fetch_byte(bus);

        let cycles = match opcode {
            0x00 => {
                self.stopped = true;
                7
            }
            0x06 => {
                let addr = self.fetch_zero_page_addr(bus);
                self.update_memory(bus, addr, Self::asl);
                5
            }
            0x08 => {
                self.push(bus, self.status | FLAG_BREAK | FLAG_UNUSED);
                3
            }
            0x09 => {
                self.a |= self.fetch_byte(bus);
                self.set_zn(self.a);
                2
            }
            0x0A => {
                self.a = self.asl(self.a);
                2
            }
            0x10 => self.branch(bus, self.status & FLAG_NEGATIVE == 0),
            0x16 => {
                let addr = self.fetch_zero_page_x_addr(bus);
                self.update_memory(bus, addr, Self::asl);
                6
            }
            0x18 => {
                self.set_flag(FLAG_CARRY, false);
                2
            }
            0x20 => {
                let target = self.fetch_word(bus);
                let return_addr = self.pc.wrapping_sub(1);
                self.push_u16(bus, return_addr);
                self.pc = target;
                6
            }
            0x24 => {
                let addr = self.fetch_zero_page_addr(bus);
                self.bit(bus.read(addr));
                3
            }
            0x26 => {
                let addr = self.fetch_zero_page_addr(bus);
                self.update_memory(bus, addr, Self::rol);
                5
            }
            0x28 => {
                self.status = (self.pop(bus) | FLAG_UNUSED) & !FLAG_BREAK;
                4
            }
            0x29 => {
                self.a &= self.fetch_byte(bus);
                self.set_zn(self.a);
                2
            }
            0x2A => {
                self.a = self.rol(self.a);
                2
            }
            0x2C => {
                let addr = self.fetch_word(bus);
                self.bit(bus.read(addr));
                4
            }
            0x2E => {
                let addr = self.fetch_word(bus);
                self.update_memory(bus, addr, Self::rol);
                6
            }
            0x30 => self.branch(bus, self.status & FLAG_NEGATIVE != 0),
            0x36 => {
                let addr = self.fetch_zero_page_x_addr(bus);
                self.update_memory(bus, addr, Self::rol);
                6
            }
            0x38 => {
                self.set_flag(FLAG_CARRY, true);
                2
            }
            0x3E => {
                let addr = self.fetch_absolute_x_addr(bus);
                self.update_memory(bus, addr, Self::rol);
                7
            }
            0x40 => {
                self.status = (self.pop(bus) | FLAG_UNUSED) & !FLAG_BREAK;
                self.pc = self.pop_u16(bus);
                6
            }
            0x46 => {
                let addr = self.fetch_zero_page_addr(bus);
                self.update_memory(bus, addr, Self::lsr);
                5
            }
            0x4C => {
                self.pc = self.fetch_word(bus);
                3
            }
            0x48 => {
                self.push(bus, self.a);
                3
            }
            0x49 => {
                self.a ^= self.fetch_byte(bus);
                self.set_zn(self.a);
                2
            }
            0x4A => {
                self.a = self.lsr(self.a);
                2
            }
            0x4E => {
                let addr = self.fetch_word(bus);
                self.update_memory(bus, addr, Self::lsr);
                6
            }
            0x50 => self.branch(bus, self.status & FLAG_OVERFLOW == 0),
            0x56 => {
                let addr = self.fetch_zero_page_x_addr(bus);
                self.update_memory(bus, addr, Self::lsr);
                6
            }
            0x58 => {
                self.set_flag(FLAG_INTERRUPT_DISABLE, false);
                2
            }
            0x5E => {
                let addr = self.fetch_absolute_x_addr(bus);
                self.update_memory(bus, addr, Self::lsr);
                7
            }
            0x60 => {
                self.pc = self.pop_u16(bus).wrapping_add(1);
                6
            }
            0x66 => {
                let addr = self.fetch_zero_page_addr(bus);
                self.update_memory(bus, addr, Self::ror);
                5
            }
            0x68 => {
                self.a = self.pop(bus);
                self.set_zn(self.a);
                4
            }
            0x69 => {
                let value = self.fetch_byte(bus);
                self.adc(value);
                2
            }
            0x6A => {
                self.a = self.ror(self.a);
                2
            }
            0x6C => {
                let vector = self.fetch_word(bus);
                self.pc = self.read_u16_bug(bus, vector);
                5
            }
            0x6E => {
                let addr = self.fetch_word(bus);
                self.update_memory(bus, addr, Self::ror);
                6
            }
            0x70 => self.branch(bus, self.status & FLAG_OVERFLOW != 0),
            0x76 => {
                let addr = self.fetch_zero_page_x_addr(bus);
                self.update_memory(bus, addr, Self::ror);
                6
            }
            0x78 => {
                self.set_flag(FLAG_INTERRUPT_DISABLE, true);
                2
            }
            0x7E => {
                let addr = self.fetch_absolute_x_addr(bus);
                self.update_memory(bus, addr, Self::ror);
                7
            }
            0x81 => {
                let addr = self.fetch_indexed_indirect_addr(bus);
                bus.write(addr, self.a);
                6
            }
            0x85 => {
                let addr = self.fetch_zero_page_addr(bus);
                bus.write(addr, self.a);
                3
            }
            0x8C => {
                let addr = self.fetch_word(bus);
                bus.write(addr, self.y);
                4
            }
            0x8D => {
                let addr = self.fetch_word(bus);
                bus.write(addr, self.a);
                4
            }
            0x8E => {
                let addr = self.fetch_word(bus);
                bus.write(addr, self.x);
                4
            }
            0x84 => {
                let addr = self.fetch_zero_page_addr(bus);
                bus.write(addr, self.y);
                3
            }
            0x86 => {
                let addr = self.fetch_zero_page_addr(bus);
                bus.write(addr, self.x);
                3
            }
            0x90 => self.branch(bus, self.status & FLAG_CARRY == 0),
            0x91 => {
                let addr = self.fetch_indirect_indexed_addr(bus);
                bus.write(addr, self.a);
                6
            }
            0x94 => {
                let addr = self.fetch_zero_page_x_addr(bus);
                bus.write(addr, self.y);
                4
            }
            0x95 => {
                let addr = self.fetch_zero_page_x_addr(bus);
                bus.write(addr, self.a);
                4
            }
            0x96 => {
                let addr = self.fetch_zero_page_y_addr(bus);
                bus.write(addr, self.x);
                4
            }
            0x88 => {
                self.y = self.y.wrapping_sub(1);
                self.set_zn(self.y);
                2
            }
            0x8A => {
                self.a = self.x;
                self.set_zn(self.a);
                2
            }
            0x98 => {
                self.a = self.y;
                self.set_zn(self.a);
                2
            }
            0x99 => {
                let addr = self.fetch_absolute_y_addr(bus);
                bus.write(addr, self.a);
                5
            }
            0x9A => {
                self.sp = self.x;
                2
            }
            0x9D => {
                let addr = self.fetch_absolute_x_addr(bus);
                bus.write(addr, self.a);
                5
            }
            0xA1 => {
                let addr = self.fetch_indexed_indirect_addr(bus);
                self.a = bus.read(addr);
                self.set_zn(self.a);
                6
            }
            0xA0 => {
                self.y = self.fetch_byte(bus);
                self.set_zn(self.y);
                2
            }
            0xA2 => {
                self.x = self.fetch_byte(bus);
                self.set_zn(self.x);
                2
            }
            0xA4 => {
                let addr = self.fetch_zero_page_addr(bus);
                self.y = bus.read(addr);
                self.set_zn(self.y);
                3
            }
            0xA5 => {
                let addr = self.fetch_zero_page_addr(bus);
                self.a = bus.read(addr);
                self.set_zn(self.a);
                3
            }
            0xA6 => {
                let addr = self.fetch_zero_page_addr(bus);
                self.x = bus.read(addr);
                self.set_zn(self.x);
                3
            }
            0xA8 => {
                self.y = self.a;
                self.set_zn(self.y);
                2
            }
            0xA9 => {
                self.a = self.fetch_byte(bus);
                self.set_zn(self.a);
                2
            }
            0xAA => {
                self.x = self.a;
                self.set_zn(self.x);
                2
            }
            0xAC => {
                let addr = self.fetch_word(bus);
                self.y = bus.read(addr);
                self.set_zn(self.y);
                4
            }
            0xAD => {
                let addr = self.fetch_word(bus);
                self.a = bus.read(addr);
                self.set_zn(self.a);
                4
            }
            0xAE => {
                let addr = self.fetch_word(bus);
                self.x = bus.read(addr);
                self.set_zn(self.x);
                4
            }
            0xB0 => self.branch(bus, self.status & FLAG_CARRY != 0),
            0xB1 => {
                let addr = self.fetch_indirect_indexed_addr(bus);
                self.a = bus.read(addr);
                self.set_zn(self.a);
                5
            }
            0xB5 => {
                let addr = self.fetch_zero_page_x_addr(bus);
                self.a = bus.read(addr);
                self.set_zn(self.a);
                4
            }
            0xB6 => {
                let addr = self.fetch_zero_page_y_addr(bus);
                self.x = bus.read(addr);
                self.set_zn(self.x);
                4
            }
            0xB8 => {
                self.set_flag(FLAG_OVERFLOW, false);
                2
            }
            0xB9 => {
                let addr = self.fetch_absolute_y_addr(bus);
                self.a = bus.read(addr);
                self.set_zn(self.a);
                4
            }
            0xBA => {
                self.x = self.sp;
                self.set_zn(self.x);
                2
            }
            0xBC => {
                let addr = self.fetch_absolute_x_addr(bus);
                self.y = bus.read(addr);
                self.set_zn(self.y);
                4
            }
            0xBD => {
                let addr = self.fetch_absolute_x_addr(bus);
                self.a = bus.read(addr);
                self.set_zn(self.a);
                4
            }
            0xBE => {
                let addr = self.fetch_absolute_y_addr(bus);
                self.x = bus.read(addr);
                self.set_zn(self.x);
                4
            }
            0xC0 => {
                let value = self.fetch_byte(bus);
                self.compare(self.y, value);
                2
            }
            0xC5 => {
                let addr = self.fetch_zero_page_addr(bus);
                self.compare(self.a, bus.read(addr));
                3
            }
            0xC6 => {
                let addr = self.fetch_zero_page_addr(bus);
                let value = bus.read(addr).wrapping_sub(1);
                bus.write(addr, value);
                self.set_zn(value);
                5
            }
            0xC8 => {
                self.y = self.y.wrapping_add(1);
                self.set_zn(self.y);
                2
            }
            0xC9 => {
                let value = self.fetch_byte(bus);
                self.compare(self.a, value);
                2
            }
            0xCA => {
                self.x = self.x.wrapping_sub(1);
                self.set_zn(self.x);
                2
            }
            0xCD => {
                let addr = self.fetch_word(bus);
                self.compare(self.a, bus.read(addr));
                4
            }
            0xCE => {
                let addr = self.fetch_word(bus);
                let value = bus.read(addr).wrapping_sub(1);
                bus.write(addr, value);
                self.set_zn(value);
                6
            }
            0xD0 => self.branch(bus, self.status & FLAG_ZERO == 0),
            0xD8 => {
                self.set_flag(FLAG_DECIMAL, false);
                2
            }
            0xE0 => {
                let value = self.fetch_byte(bus);
                self.compare(self.x, value);
                2
            }
            0xE6 => {
                let addr = self.fetch_zero_page_addr(bus);
                let value = bus.read(addr).wrapping_add(1);
                bus.write(addr, value);
                self.set_zn(value);
                5
            }
            0xE8 => {
                self.x = self.x.wrapping_add(1);
                self.set_zn(self.x);
                2
            }
            0xE9 => {
                let value = self.fetch_byte(bus);
                self.sbc(value);
                2
            }
            0xEA => 2,
            0xEE => {
                let addr = self.fetch_word(bus);
                let value = bus.read(addr).wrapping_add(1);
                bus.write(addr, value);
                self.set_zn(value);
                6
            }
            0x0E => {
                let addr = self.fetch_word(bus);
                self.update_memory(bus, addr, Self::asl);
                6
            }
            0x1E => {
                let addr = self.fetch_absolute_x_addr(bus);
                self.update_memory(bus, addr, Self::asl);
                7
            }
            0xF0 => self.branch(bus, self.status & FLAG_ZERO != 0),
            0xF8 => {
                self.set_flag(FLAG_DECIMAL, true);
                2
            }
            _ => {
                return Err(CpuError::UnsupportedOpcode {
                    opcode,
                    pc: opcode_pc,
                })
            }
        };

        Ok(cycles)
    }

    fn adc(&mut self, value: u8) {
        let carry = if self.status & FLAG_CARRY != 0 { 1 } else { 0 };
        let sum = self.a as u16 + value as u16 + carry as u16;
        let result = sum as u8;

        self.set_flag(FLAG_CARRY, sum > 0xFF);
        self.set_flag(
            FLAG_OVERFLOW,
            (!(self.a ^ value) & (self.a ^ result) & 0x80) != 0,
        );

        self.a = result;
        self.set_zn(self.a);
    }

    fn sbc(&mut self, value: u8) {
        self.adc(!value);
    }

    fn compare(&mut self, left: u8, right: u8) {
        let result = left.wrapping_sub(right);
        self.set_flag(FLAG_CARRY, left >= right);
        self.set_zn(result);
    }

    fn bit(&mut self, value: u8) {
        self.set_flag(FLAG_ZERO, self.a & value == 0);
        self.set_flag(FLAG_OVERFLOW, value & FLAG_OVERFLOW != 0);
        self.set_flag(FLAG_NEGATIVE, value & FLAG_NEGATIVE != 0);
    }

    fn asl(&mut self, value: u8) -> u8 {
        self.set_flag(FLAG_CARRY, value & 0x80 != 0);
        let result = value << 1;
        self.set_zn(result);
        result
    }

    fn lsr(&mut self, value: u8) -> u8 {
        self.set_flag(FLAG_CARRY, value & 0x01 != 0);
        let result = value >> 1;
        self.set_zn(result);
        result
    }

    fn rol(&mut self, value: u8) -> u8 {
        let carry_in = u8::from(self.status & FLAG_CARRY != 0);
        self.set_flag(FLAG_CARRY, value & 0x80 != 0);
        let result = (value << 1) | carry_in;
        self.set_zn(result);
        result
    }

    fn ror(&mut self, value: u8) -> u8 {
        let carry_in = if self.status & FLAG_CARRY != 0 { 0x80 } else { 0x00 };
        self.set_flag(FLAG_CARRY, value & 0x01 != 0);
        let result = (value >> 1) | carry_in;
        self.set_zn(result);
        result
    }

    fn branch<B: Bus>(&mut self, bus: &mut B, condition: bool) -> u8 {
        let offset = self.fetch_byte(bus) as i8;
        if condition {
            self.pc = self.pc.wrapping_add_signed(offset as i16);
            3
        } else {
            2
        }
    }

    fn fetch_byte<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let value = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        value
    }

    fn fetch_word<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let low = self.fetch_byte(bus);
        let high = self.fetch_byte(bus);
        u16::from_le_bytes([low, high])
    }

    fn fetch_zero_page_addr<B: Bus>(&mut self, bus: &mut B) -> u16 {
        self.fetch_byte(bus) as u16
    }

    fn fetch_zero_page_x_addr<B: Bus>(&mut self, bus: &mut B) -> u16 {
        self.fetch_byte(bus).wrapping_add(self.x) as u16
    }

    fn fetch_zero_page_y_addr<B: Bus>(&mut self, bus: &mut B) -> u16 {
        self.fetch_byte(bus).wrapping_add(self.y) as u16
    }

    fn fetch_absolute_x_addr<B: Bus>(&mut self, bus: &mut B) -> u16 {
        self.fetch_word(bus).wrapping_add(self.x as u16)
    }

    fn fetch_absolute_y_addr<B: Bus>(&mut self, bus: &mut B) -> u16 {
        self.fetch_word(bus).wrapping_add(self.y as u16)
    }

    fn fetch_indexed_indirect_addr<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let base = self.fetch_byte(bus).wrapping_add(self.x);
        self.read_zero_page_u16(bus, base)
    }

    fn fetch_indirect_indexed_addr<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let base = self.fetch_byte(bus);
        self.read_zero_page_u16(bus, base)
            .wrapping_add(self.y as u16)
    }

    fn read_zero_page_u16<B: Bus>(&mut self, bus: &mut B, base: u8) -> u16 {
        let low = bus.read(base as u16);
        let high = bus.read(base.wrapping_add(1) as u16);
        u16::from_le_bytes([low, high])
    }

    fn update_memory<B: Bus>(&mut self, bus: &mut B, addr: u16, op: fn(&mut Self, u8) -> u8) {
        let value = bus.read(addr);
        let result = op(self, value);
        bus.write(addr, result);
    }

    fn service_interrupt<B: Bus>(&mut self, bus: &mut B, vector: u16, is_break: bool) {
        self.push_u16(bus, self.pc);
        let status = if is_break {
            self.status | FLAG_BREAK | FLAG_UNUSED
        } else {
            (self.status & !FLAG_BREAK) | FLAG_UNUSED
        };
        self.push(bus, status);
        self.set_flag(FLAG_INTERRUPT_DISABLE, true);
        self.pc = self.read_u16(bus, vector);
    }

    fn read_u16<B: Bus>(&mut self, bus: &mut B, addr: u16) -> u16 {
        let low = bus.read(addr);
        let high = bus.read(addr.wrapping_add(1));
        u16::from_le_bytes([low, high])
    }

    fn read_u16_bug<B: Bus>(&mut self, bus: &mut B, addr: u16) -> u16 {
        let low = bus.read(addr);
        let high_addr = (addr & 0xFF00) | (addr.wrapping_add(1) & 0x00FF);
        let high = bus.read(high_addr);
        u16::from_le_bytes([low, high])
    }

    fn push<B: Bus>(&mut self, bus: &mut B, value: u8) {
        let addr = 0x0100 | self.sp as u16;
        bus.write(addr, value);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn push_u16<B: Bus>(&mut self, bus: &mut B, value: u16) {
        self.push(bus, (value >> 8) as u8);
        self.push(bus, value as u8);
    }

    fn pop<B: Bus>(&mut self, bus: &mut B) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        let addr = 0x0100 | self.sp as u16;
        bus.read(addr)
    }

    fn pop_u16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let low = self.pop(bus);
        let high = self.pop(bus);
        u16::from_le_bytes([low, high])
    }

    fn set_zn(&mut self, value: u8) {
        self.set_flag(FLAG_ZERO, value == 0);
        self.set_flag(FLAG_NEGATIVE, value & 0x80 != 0);
    }

    fn set_flag(&mut self, flag: u8, enabled: bool) {
        if enabled {
            self.status |= flag;
        } else {
            self.status &= !flag;
        }

        self.status |= FLAG_UNUSED;
    }
}

#[cfg(test)]
mod tests {
    use super::{Cpu6510, FLAG_BREAK, FLAG_INTERRUPT_DISABLE, FLAG_NEGATIVE, FLAG_OVERFLOW, FLAG_ZERO};
    use crate::bus::Bus;

    struct TestBus {
        memory: [u8; 0x10000],
        nmi_pending: bool,
        irq_pending: bool,
    }

    impl TestBus {
        fn new() -> Self {
            Self {
                memory: [0; 0x10000],
                nmi_pending: false,
                irq_pending: false,
            }
        }

        fn load(&mut self, start: u16, bytes: &[u8]) {
            let start = start as usize;
            self.memory[start..start + bytes.len()].copy_from_slice(bytes);
        }

        fn set_reset_vector(&mut self, addr: u16) {
            let [low, high] = addr.to_le_bytes();
            self.memory[0xFFFC] = low;
            self.memory[0xFFFD] = high;
        }
    }

    impl Bus for TestBus {
        fn read(&mut self, addr: u16) -> u8 {
            self.memory[addr as usize]
        }

        fn write(&mut self, addr: u16, value: u8) {
            self.memory[addr as usize] = value;
        }

        fn poll_nmi(&mut self) -> bool {
            self.nmi_pending
        }

        fn poll_irq(&mut self) -> bool {
            self.irq_pending
        }
    }

    #[test]
    fn lda_and_sta_store_bytes_in_memory() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(0x8000, &[0xA9, 0x42, 0x8D, 0x00, 0x20, 0x00]);

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.a, 0x42);
        assert_eq!(bus.memory[0x2000], 0x42);
    }

    #[test]
    fn jsr_and_rts_round_trip_through_the_stack() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(0x8000, &[0x20, 0x05, 0x80, 0x00, 0xEA, 0xA2, 0x05, 0x60]);

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.x, 0x05);
        assert!(cpu.stopped);
        assert_eq!(cpu.pc, 0x8004);
    }

    #[test]
    fn bne_takes_the_branch_when_zero_flag_is_clear() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(0x8000, &[0xA2, 0x01, 0xD0, 0x02, 0xA2, 0xFF, 0x00]);

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.x, 0x01);
        assert!(cpu.stopped);
    }

    #[test]
    fn indexed_store_and_load_round_trip_through_memory() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(
            0x8000,
            &[
                0xA2, 0x01, 0xA9, 0x2A, 0x9D, 0x00, 0x04, 0xA9, 0x00, 0xBD, 0x00, 0x04, 0x00,
            ],
        );

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        for _ in 0..6 {
            cpu.step(&mut bus).unwrap();
        }

        assert_eq!(bus.memory[0x0401], 0x2A);
        assert_eq!(cpu.a, 0x2A);
        assert!(cpu.stopped);
    }

    #[test]
    fn cpx_supports_simple_counting_loop() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(
            0x8000,
            &[
                0xA2, 0x00, 0xE8, 0xE0, 0x03, 0xD0, 0xFB, 0x8E, 0x00, 0x04, 0x00,
            ],
        );

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        while !cpu.stopped {
            cpu.step(&mut bus).unwrap();
        }

        assert_eq!(cpu.x, 0x03);
        assert_eq!(bus.memory[0x0400], 0x03);
    }

    #[test]
    fn stack_and_status_instructions_restore_previous_values() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(
            0x8000,
            &[
                0xA9, 0x00, 0x08, 0xA9, 0x80, 0x48, 0xA9, 0x01, 0x68, 0x28, 0x00,
            ],
        );

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        while !cpu.stopped {
            cpu.step(&mut bus).unwrap();
        }

        assert_eq!(cpu.a, 0x80);
        assert_ne!(cpu.status & FLAG_ZERO, 0);
        assert_ne!(cpu.status & FLAG_INTERRUPT_DISABLE, 0);
    }

    #[test]
    fn indexed_indirect_load_and_store_follow_zero_page_vectors() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(0x8000, &[0xA2, 0x04, 0xA1, 0x20, 0x81, 0x30, 0x00]);
        bus.memory[0x0024] = 0x00;
        bus.memory[0x0025] = 0x40;
        bus.memory[0x0034] = 0x00;
        bus.memory[0x0035] = 0x50;
        bus.memory[0x4000] = 0x7B;

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        while !cpu.stopped {
            cpu.step(&mut bus).unwrap();
        }

        assert_eq!(cpu.a, 0x7B);
        assert_eq!(bus.memory[0x5000], 0x7B);
    }

    #[test]
    fn indirect_indexed_load_and_store_copy_bytes_through_pointers() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(0x8000, &[0xA0, 0x01, 0xB1, 0x10, 0x91, 0x20, 0x00]);
        bus.memory[0x0010] = 0x00;
        bus.memory[0x0011] = 0x40;
        bus.memory[0x0020] = 0x00;
        bus.memory[0x0021] = 0x50;
        bus.memory[0x4001] = 0x44;

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        while !cpu.stopped {
            cpu.step(&mut bus).unwrap();
        }

        assert_eq!(cpu.a, 0x44);
        assert_eq!(bus.memory[0x5001], 0x44);
    }

    #[test]
    fn bit_updates_zero_negative_and_overflow_flags() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(0x8000, &[0xA9, 0x40, 0x2C, 0x00, 0x20, 0x00]);
        bus.memory[0x2000] = 0xC0;

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();

        assert_eq!(cpu.status & FLAG_ZERO, 0);
        assert_ne!(cpu.status & FLAG_NEGATIVE, 0);
        assert_ne!(cpu.status & FLAG_OVERFLOW, 0);
    }

    #[test]
    fn jmp_indirect_uses_page_wrapped_vector_like_6502() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(0x8000, &[0x6C, 0xFF, 0x20]);
        bus.memory[0x20FF] = 0x34;
        bus.memory[0x2000] = 0x12;

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        let cycles = cpu.step(&mut bus).unwrap();

        assert_eq!(cycles, 5);
        assert_eq!(cpu.pc, 0x1234);
    }

    #[test]
    fn shift_and_rotate_accumulator_round_trip_bits_and_carry() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(0x8000, &[0xA9, 0x81, 0x0A, 0x6A, 0x4A, 0x2A, 0x00]);

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        cpu.step(&mut bus).unwrap();
        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.a, 0x02);
        assert_ne!(cpu.status & super::FLAG_CARRY, 0);

        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.a, 0x81);
        assert_eq!(cpu.status & super::FLAG_CARRY, 0);

        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.a, 0x40);
        assert_ne!(cpu.status & super::FLAG_CARRY, 0);

        cpu.step(&mut bus).unwrap();
        assert_eq!(cpu.a, 0x81);
        assert_eq!(cpu.status & super::FLAG_CARRY, 0);
        assert_ne!(cpu.status & FLAG_NEGATIVE, 0);
    }

    #[test]
    fn memory_shift_and_rotate_variants_update_memory_and_flags() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(
            0x8000,
            &[
                0x38, 0xA2, 0x01, 0x36, 0x0F, 0x5E, 0xFF, 0x1F, 0x2E, 0x01, 0x20, 0x6E, 0x01,
                0x20, 0x00,
            ],
        );
        bus.memory[0x0010] = 0x40;
        bus.memory[0x2000] = 0x01;
        bus.memory[0x2001] = 0x80;

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);

        while !cpu.stopped {
            cpu.step(&mut bus).unwrap();
        }

        assert_eq!(bus.memory[0x0010], 0x81);
        assert_eq!(bus.memory[0x2000], 0x00);
        assert_eq!(bus.memory[0x2001], 0x80);
        assert_ne!(cpu.status & super::FLAG_CARRY, 0);
        assert_ne!(cpu.status & FLAG_NEGATIVE, 0);
    }

    #[test]
    fn pending_irq_vectors_before_next_instruction() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.memory[0xFFFE] = 0x00;
        bus.memory[0xFFFF] = 0x90;

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);
        cpu.status &= !FLAG_INTERRUPT_DISABLE;
        bus.irq_pending = true;

        let cycles = cpu.step(&mut bus).unwrap();

        assert_eq!(cycles, 7);
        assert_eq!(cpu.pc, 0x9000);
        assert_ne!(cpu.status & FLAG_INTERRUPT_DISABLE, 0);
    }

    #[test]
    fn pending_nmi_vectors_even_with_interrupt_disable_set() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.memory[0xFFFA] = 0x00;
        bus.memory[0xFFFB] = 0x90;

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);
        bus.nmi_pending = true;

        let cycles = cpu.step(&mut bus).unwrap();

        assert_eq!(cycles, 7);
        assert_eq!(cpu.pc, 0x9000);
        assert_ne!(cpu.status & FLAG_INTERRUPT_DISABLE, 0);
    }

    #[test]
    fn rti_restores_status_and_program_counter() {
        let mut bus = TestBus::new();
        bus.set_reset_vector(0x8000);
        bus.load(0x8000, &[0x40]);

        let mut cpu = Cpu6510::new();
        cpu.reset(&mut bus);
        cpu.sp = 0xFA;
        bus.memory[0x01FB] = FLAG_ZERO;
        bus.memory[0x01FC] = 0x34;
        bus.memory[0x01FD] = 0x12;

        let cycles = cpu.step(&mut bus).unwrap();

        assert_eq!(cycles, 6);
        assert_eq!(cpu.pc, 0x1234);
        assert_ne!(cpu.status & FLAG_ZERO, 0);
        assert_eq!(cpu.status & FLAG_BREAK, 0);
    }
}