#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CiaSnapshot {
    pub total_cycles: u64,
    pub timer_a: u16,
    pub timer_b: u16,
    pub timer_a_running: bool,
    pub timer_b_running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cia6526 {
    registers: [u8; 0x10],
    timer_a_latch: u16,
    timer_b_latch: u16,
    timer_a_counter: u16,
    timer_b_counter: u16,
    timer_a_running: bool,
    timer_b_running: bool,
    total_cycles: u64,
}

impl Default for Cia6526 {
    fn default() -> Self {
        Self {
            registers: [0; 0x10],
            timer_a_latch: 0,
            timer_b_latch: 0,
            timer_a_counter: 0,
            timer_b_counter: 0,
            timer_a_running: false,
            timer_b_running: false,
            total_cycles: 0,
        }
    }
}

impl Cia6526 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr as usize & 0x0F {
            0x04 => self.timer_a_counter as u8,
            0x05 => (self.timer_a_counter >> 8) as u8,
            0x06 => self.timer_b_counter as u8,
            0x07 => (self.timer_b_counter >> 8) as u8,
            index => self.registers[index],
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        let index = addr as usize & 0x0F;
        self.registers[index] = value;

        match index {
            0x04 => {
                self.timer_a_latch = (self.timer_a_latch & 0xFF00) | value as u16;
                if !self.timer_a_running {
                    self.timer_a_counter = self.timer_a_latch;
                }
            }
            0x05 => {
                self.timer_a_latch = (self.timer_a_latch & 0x00FF) | ((value as u16) << 8);
                if !self.timer_a_running {
                    self.timer_a_counter = self.timer_a_latch;
                }
            }
            0x06 => {
                self.timer_b_latch = (self.timer_b_latch & 0xFF00) | value as u16;
                if !self.timer_b_running {
                    self.timer_b_counter = self.timer_b_latch;
                }
            }
            0x07 => {
                self.timer_b_latch = (self.timer_b_latch & 0x00FF) | ((value as u16) << 8);
                if !self.timer_b_running {
                    self.timer_b_counter = self.timer_b_latch;
                }
            }
            0x0E => {
                self.timer_a_running = value & 0x01 != 0;
                if value & 0x10 != 0 || self.timer_a_counter == 0 {
                    self.timer_a_counter = self.timer_a_latch.max(1);
                }
            }
            0x0F => {
                self.timer_b_running = value & 0x01 != 0;
                if value & 0x10 != 0 || self.timer_b_counter == 0 {
                    self.timer_b_counter = self.timer_b_latch.max(1);
                }
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, cycles: u8) {
        self.total_cycles += cycles as u64;

        for _ in 0..cycles {
            self.tick_timer_a();
            self.tick_timer_b();
        }
    }

    pub fn snapshot(&self) -> CiaSnapshot {
        CiaSnapshot {
            total_cycles: self.total_cycles,
            timer_a: self.timer_a_counter,
            timer_b: self.timer_b_counter,
            timer_a_running: self.timer_a_running,
            timer_b_running: self.timer_b_running,
        }
    }

    fn tick_timer_a(&mut self) {
        if self.timer_a_running && self.timer_a_latch != 0 {
            if self.timer_a_counter > 0 {
                self.timer_a_counter -= 1;
            }

            if self.timer_a_counter == 0 {
                self.registers[0x0D] |= 0x01;
                if self.registers[0x0E] & 0x08 != 0 {
                    self.timer_a_running = false;
                } else {
                    self.timer_a_counter = self.timer_a_latch.max(1);
                }
            }
        }
    }

    fn tick_timer_b(&mut self) {
        if self.timer_b_running && self.timer_b_latch != 0 {
            if self.timer_b_counter > 0 {
                self.timer_b_counter -= 1;
            }

            if self.timer_b_counter == 0 {
                self.registers[0x0D] |= 0x02;
                if self.registers[0x0F] & 0x08 != 0 {
                    self.timer_b_running = false;
                } else {
                    self.timer_b_counter = self.timer_b_latch.max(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cia6526, CiaSnapshot};

    #[test]
    fn timer_a_counts_down_when_started() {
        let mut cia = Cia6526::new();
        cia.write(0xDC04, 0x03);
        cia.write(0xDC05, 0x00);
        cia.write(0xDC0E, 0x11);

        cia.tick(2);

        assert_eq!(cia.snapshot(), CiaSnapshot {
            total_cycles: 2,
            timer_a: 1,
            timer_b: 0,
            timer_a_running: true,
            timer_b_running: false,
        });
    }
}