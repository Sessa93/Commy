#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JoystickState {
    pressed: u8,
}

impl JoystickState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_up(&mut self, pressed: bool) {
        self.set_bit(0, pressed);
    }

    pub fn set_down(&mut self, pressed: bool) {
        self.set_bit(1, pressed);
    }

    pub fn set_left(&mut self, pressed: bool) {
        self.set_bit(2, pressed);
    }

    pub fn set_right(&mut self, pressed: bool) {
        self.set_bit(3, pressed);
    }

    pub fn set_fire(&mut self, pressed: bool) {
        self.set_bit(4, pressed);
    }

    pub fn port_value(&self) -> u8 {
        0xFF & !(self.pressed & 0x1F)
    }

    fn set_bit(&mut self, bit: u8, pressed: bool) {
        if pressed {
            self.pressed |= 1 << bit;
        } else {
            self.pressed &= !(1 << bit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::JoystickState;

    #[test]
    fn pressed_controls_pull_low_bits() {
        let mut joystick = JoystickState::new();
        joystick.set_left(true);
        joystick.set_fire(true);

        assert_eq!(joystick.port_value(), 0xEB);
    }
}