#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidSnapshot {
    pub total_cycles: u64,
    pub voice_phase: [u32; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sid6581 {
    registers: [u8; 0x20],
    voice_phase: [u32; 3],
    total_cycles: u64,
}

impl Default for Sid6581 {
    fn default() -> Self {
        Self {
            registers: [0; 0x20],
            voice_phase: [0; 3],
            total_cycles: 0,
        }
    }
}

impl Sid6581 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read(&self, addr: u16) -> u8 {
        self.registers[addr as usize & 0x1F]
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        self.registers[addr as usize & 0x1F] = value;
    }

    pub fn tick(&mut self, cycles: u8) {
        self.total_cycles += cycles as u64;

        for voice in 0..3 {
            let register_base = voice * 7;
            let frequency = u16::from_le_bytes([
                self.registers[register_base],
                self.registers[register_base + 1],
            ]);
            self.voice_phase[voice] = self.voice_phase[voice]
                .wrapping_add(frequency as u32 * cycles as u32);
        }
    }

    pub fn snapshot(&self) -> SidSnapshot {
        SidSnapshot {
            total_cycles: self.total_cycles,
            voice_phase: self.voice_phase,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Sid6581, SidSnapshot};

    #[test]
    fn voice_phase_advances_with_frequency_and_cycles() {
        let mut sid = Sid6581::new();
        sid.write(0xD400, 0x34);
        sid.write(0xD401, 0x12);

        sid.tick(2);

        assert_eq!(sid.snapshot(), SidSnapshot {
            total_cycles: 2,
            voice_phase: [0x1234 * 2, 0, 0],
        });
    }
}