use embedded_hal::blocking::{delay::DelayMs, i2c};

pub trait Display {
    type Error;

    fn clear(&mut self) -> Result<(), Self::Error>;
    fn set_cursor_position(&mut self, row: u8, column: u8) -> Result<(), Self::Error>;
    fn write(&mut self, characters: &[u8]) -> Result<(), Self::Error>;
}

pub struct BufferedDisplay<const ROWS: usize, const COLUMNS: usize> {
    backing_buffer: [[u8; COLUMNS]; ROWS],
    visible_buffer: [[u8; COLUMNS]; ROWS],
    current_row: u8,
    current_column: u8,
}
#[derive(Debug)]
pub enum BufferedDisplayError {
    OutOfBounds,
}

impl<const ROWS: usize, const COLUMNS: usize> BufferedDisplay<ROWS, COLUMNS> {
    pub const fn new() -> Self {
        BufferedDisplay {
            backing_buffer: [[0u8; COLUMNS]; ROWS],
            visible_buffer: [[0u8; COLUMNS]; ROWS],
            current_row: 0,
            current_column: 0,
        }
    }

    pub fn apply<D: Display>(&self, target: &mut D) -> Result<(), D::Error> {
        // iterate over all rows and write them out
        // TODO: make this smarter and work with the backing/visible buffer
        for row in 0..ROWS as u8 {
            target.set_cursor_position(row, 0)?;
            target.write(&self.backing_buffer[row as usize])?;
        }

        Ok(())
    }
}

impl<const ROWS: usize, const COLUMNS: usize> Display for BufferedDisplay<ROWS, COLUMNS> {
    type Error = BufferedDisplayError;

    fn clear(&mut self) -> Result<(), Self::Error> {
        for row in 0..ROWS {
            for col in 0..COLUMNS {
                self.backing_buffer[row][col] = 0;
                self.visible_buffer[row][col] = 0;
            }
        }

        self.current_row = 0;
        self.current_column = 0;
        Ok(())
    }

    fn set_cursor_position(&mut self, row: u8, column: u8) -> Result<(), Self::Error> {
        if row >= ROWS as u8 || column >= COLUMNS as u8 {
            return Err(Self::Error::OutOfBounds);
        }
        self.current_row = row;
        self.current_column = column;
        Ok(())
    }

    fn write(&mut self, characters: &[u8]) -> Result<(), Self::Error> {
        for &b in characters {
            self.backing_buffer[self.current_row as usize][self.current_column as usize] = b;

            self.current_column += 1;
            if self.current_column >= COLUMNS as u8 {
                self.current_column = 0;
                self.current_row += 1;

                self.current_row = (self.current_row + 1) % ROWS as u8;

                // if self.current_row >= ROWS as u8 {
                //     self.current_row = 0;
                // }
            }
        }
        Ok(())
    }
}

pub struct I2CDisplayDriver<I> {
    port: I,
    address: u8,
    rows: u8,
    columns: u8,
}

#[allow(dead_code)]
impl<I: i2c::Write> I2CDisplayDriver<I> {
    pub fn new(port: I, address: u8, rows: u8, columns: u8) -> Self {
        I2CDisplayDriver {
            port,
            address,
            rows,
            columns,
        }
    }

    fn cmd(&mut self, cmd: u8) -> Result<(), I::Error> {
        // self.port.write_read(self.address, &[0x00, cmd], &mut[0u8; 0])
        self.port.write(self.address, &[0x00, cmd])
    }

    pub fn set_type<D: DelayMs<u8>>(&mut self, t: u8, delay: &mut D) -> Result<(), I::Error> {
        self.cmd(0x18)?;
        self.cmd(t)?;
        delay.delay_ms(10);
        Ok(())
    }

    pub fn set_backlight_enabled(&mut self, on: bool) -> Result<(), I::Error> {
        match on {
            true => self.cmd(0x13),
            false => self.cmd(0x14),
        }
    }

    pub fn set_backlight_brightness(&mut self, brightness: u8) -> Result<(), I::Error> {
        self.cmd(0x1f)?;
        self.cmd(brightness)
    }

    pub fn set_contrast(&mut self, contrast: u8) -> Result<(), I::Error> {
        self.cmd(0x1e)?;
        self.cmd(contrast)
    }
}

impl<I: i2c::Write> Display for I2CDisplayDriver<I> {
    type Error = I::Error;

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.cmd(0x0c)
    }

    fn set_cursor_position(&mut self, row: u8, column: u8) -> Result<(), Self::Error> {
        // bounds check
        if row >= self.rows || column >= self.columns {
            // panic for now
            panic!("Cursor position out of bounds")
        }
        self.cmd(0x02)?;
        self.cmd(row * self.columns + column + 1)
    }

    fn write(&mut self, characters: &[u8]) -> Result<(), Self::Error> {
        for &c in characters {
            self.cmd(c)?;
        }
        Ok(())
    }
}

// impl<I, E> Mpu6050<I> where I: Write<Error = E> + WriteRead<Error = E> {}

/* Python driver source
class LCD():
    ADDR = 0x63
    REG_CMD = 0x00

    WIDTH = 20
    HEIGHT = 4


    def __init__(self, bus, brightness = 32):
        self.bus = bus

        ###### Init the LCD
        self.set_type(4)
        # Underline cursor
        self.lcd_cmd(0x04)

        self.clear()
        self.set_brightness(brightness)

    def lcd_cmd(self, cmd):
        """ Main function for sending commands to the display :)"""
        self.bus.write_byte_data(self.ADDR,self.REG_CMD,cmd)

    def lcd_write(self, text):
        # Converts simple text to ASCII characters and send to display
        #text = [ ord(c) for c in text]
        [self.lcd_cmd(ord(str(c))) for c in text]


    def set_cursor(self, row, column):
        if row < 1 or row > self.HEIGHT or column < 0 or column > self.WIDTH - 1:
            raise ValueError("Value for row and column passed to set_cursor are out of limits :", row, column)
        self.lcd_cmd(0x02)
        self.lcd_cmd((((row - 1) * self.WIDTH) + column + 1))


    def set_type(self, type):
        self.lcd_cmd(0x18)
        self.lcd_cmd(type)

        sleep(.01)

    def set_backlit(self, val):
        if val == True:
            self.lcd_cmd(0x13)
        elif val == False:
            self.lcd_cmd(0x14)

    def set_brightness(self, val):
        if val < 0 or val > 255:
            raise ValueError("Value passed to set_brightness is not > 0 and < 255!! It's: " + str(val))
        self.lcd_cmd(0x1f)
        self.lcd_cmd(val)

    def set_contrast(self, val):
        if val < 0 or val > 255:
            raise ValueError("Value passed to set_contrast is not > 0 and < 255!! It's: " + str(val))
        self.lcd_cmd(0x1e)
        self.lcd_cmd(val)

    def clear(self):
        self.lcd_cmd(0x0c)

    def write(self, string, row = -1, column = -1, centered = False):
        """ Writes text to the screen at a specific place or just at current cursor."""

        if centered:
            column = self.WIDTH / 2 - len(string) / 2

        if row != -1 and column == -1:
            self.set_cursor(row, 0)
        elif column != -1 and row != -1:
            self.set_cursor(row, column)

        self.lcd_write(string)



*/
