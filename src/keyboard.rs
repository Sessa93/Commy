#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardMatrix {
    pressed: [[bool; 8]; 8],
}

impl Default for KeyboardMatrix {
    fn default() -> Self {
        Self {
            pressed: [[false; 8]; 8],
        }
    }
}

impl KeyboardMatrix {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_key(&mut self, row: u8, column: u8, pressed: bool) {
        if row < 8 && column < 8 {
            self.pressed[row as usize][column as usize] = pressed;
        }
    }

    pub fn read_columns(&self, row_select: u8) -> u8 {
        let mut result = 0xFF;

        for row in 0..8 {
            if row_select & (1 << row) == 0 {
                for column in 0..8 {
                    if self.pressed[row][column] {
                        result &= !(1 << column);
                    }
                }
            }
        }

        result
    }

    pub fn read_rows(&self, column_select: u8) -> u8 {
        let mut result = 0xFF;

        for column in 0..8 {
            if column_select & (1 << column) == 0 {
                for row in 0..8 {
                    if self.pressed[row][column] {
                        result &= !(1 << row);
                    }
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::KeyboardMatrix;

    #[test]
    fn selected_row_pulls_pressed_column_low() {
        let mut matrix = KeyboardMatrix::new();
        matrix.set_key(0, 2, true);

        assert_eq!(matrix.read_columns(0xFE), 0xFB);
    }

    #[test]
    fn selected_column_pulls_pressed_row_low() {
        let mut matrix = KeyboardMatrix::new();
        matrix.set_key(3, 1, true);

        assert_eq!(matrix.read_rows(0xFD), 0xF7);
    }
}