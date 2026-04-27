const SCREEN_COLUMNS: usize = 40;
const SCREEN_ROWS: usize = 25;
const SCREEN_RAM_BASE: usize = 0x0400;
const CYCLES_PER_RASTER_LINE: u16 = 63;
const RASTER_LINES_PER_FRAME: u16 = 312;
const IRQ_RASTER: u8 = 0x01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterState {
    pub line: u16,
    pub cycle: u16,
    pub frame: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VicII {
    registers: [u8; 0x40],
    raster_line: u16,
    cycle_in_line: u16,
    frame_count: u64,
    raster_irq_line: u16,
    irq_enable: u8,
    irq_status: u8,
    irq_pending: bool,
}

impl Default for VicII {
    fn default() -> Self {
        let mut vic = Self {
            registers: [0; 0x40],
            raster_line: 0,
            cycle_in_line: 0,
            frame_count: 0,
            raster_irq_line: 0,
            irq_enable: 0,
            irq_status: 0,
            irq_pending: false,
        };
        vic.sync_raster_registers();
        vic
    }
}

impl VicII {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr as usize & 0x3F {
            0x11 => (self.registers[0x11] & 0x7F) | ((self.raster_line >> 1) as u8 & 0x80),
            0x12 => self.raster_line as u8,
            0x19 => self.irq_status | if self.irq_pending { 0x80 } else { 0x00 },
            0x1A => self.irq_enable,
            index => self.registers[index],
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        let index = addr as usize & 0x3F;
        match index {
            0x11 => {
                self.registers[index] = value & 0x7F;
                self.raster_irq_line = (self.raster_irq_line & 0x00FF) | (((value as u16) & 0x80) << 1);
            }
            0x12 => {
                self.raster_irq_line = (self.raster_irq_line & 0x0100) | value as u16;
            }
            0x19 => {
                self.irq_status &= !(value & 0x0F);
                self.update_irq_pending();
            }
            0x1A => {
                self.irq_enable = value & 0x0F;
                self.update_irq_pending();
            }
            _ => self.registers[index] = value,
        }
    }

    pub fn tick(&mut self, cycles: u8) {
        self.cycle_in_line += cycles as u16;

        while self.cycle_in_line >= CYCLES_PER_RASTER_LINE {
            self.cycle_in_line -= CYCLES_PER_RASTER_LINE;
            self.raster_line += 1;

            if self.raster_line >= RASTER_LINES_PER_FRAME {
                self.raster_line = 0;
                self.frame_count += 1;
            }

            self.check_raster_irq();
        }

        self.sync_raster_registers();
    }

    pub fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    pub fn raster_state(&self) -> RasterState {
        RasterState {
            line: self.raster_line,
            cycle: self.cycle_in_line,
            frame: self.frame_count,
        }
    }

    pub fn render_text_screen(&self, ram: &[u8; 0x10000]) -> String {
        let mut output = String::with_capacity((SCREEN_COLUMNS + 1) * SCREEN_ROWS);

        for row in 0..SCREEN_ROWS {
            let row_start = SCREEN_RAM_BASE + row * SCREEN_COLUMNS;
            let row_slice = &ram[row_start..row_start + SCREEN_COLUMNS];
            for &cell in row_slice {
                output.push(screen_code_to_ascii(cell));
            }

            if row + 1 != SCREEN_ROWS {
                output.push('\n');
            }
        }

        output
    }

    fn sync_raster_registers(&mut self) {
        self.registers[0x12] = self.raster_line as u8;
    }

    fn check_raster_irq(&mut self) {
        if self.raster_line == self.raster_irq_line {
            self.irq_status |= IRQ_RASTER;
            self.update_irq_pending();
        }
    }

    fn update_irq_pending(&mut self) {
        self.irq_pending = self.irq_status & self.irq_enable != 0;
    }
}

fn screen_code_to_ascii(value: u8) -> char {
    match value {
        0x00 | 0x20 => ' ',
        0x01..=0x1A => (b'A' + value - 1) as char,
        0x1B..=0x1F => ' ',
        0x21..=0x3A => value as char,
        0x41..=0x5A => value as char,
        _ => '.',
    }
}

#[cfg(test)]
mod tests {
    use super::{RasterState, VicII};

    #[test]
    fn tick_advances_raster_position() {
        let mut vic = VicII::new();

        vic.tick(63);
        assert_eq!(vic.raster_state(), RasterState { line: 1, cycle: 0, frame: 0 });
        assert_eq!(vic.read(0xD012), 1);

        vic.tick(62);
        assert_eq!(vic.raster_state(), RasterState { line: 1, cycle: 62, frame: 0 });
    }

    #[test]
    fn screen_snapshot_uses_screen_ram_contents() {
        let vic = VicII::new();
        let mut ram = [0u8; 0x10000];
        ram[0x0400] = 0x08;
        ram[0x0401] = 0x05;
        ram[0x0402] = 0x0C;
        ram[0x0403] = 0x0C;
        ram[0x0404] = 0x0F;

        let screen = vic.render_text_screen(&ram);
        let first_line = screen.lines().next().unwrap();

        assert_eq!(&first_line[..5], "HELLO");
    }

    #[test]
    fn raster_irq_latches_and_clears_via_registers() {
        let mut vic = VicII::new();
        vic.write(0xD012, 0x01);
        vic.write(0xD01A, 0x01);

        vic.tick(63);

        assert!(vic.irq_pending());
        assert_eq!(vic.read(0xD019), 0x81);

        vic.write(0xD019, 0x01);
        assert!(!vic.irq_pending());
        assert_eq!(vic.read(0xD019), 0x00);
    }
}