use crate::{display::Display, event::InterruptEvent};

#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct Dialog {
    pub text: &'static str,
    pub event: Option<InterruptEvent>,
}

impl Dialog {
    pub fn new(text: &'static str, event: Option<InterruptEvent>) -> Dialog {
        Dialog { text, event }
    }

    pub fn interaction(&mut self) {}

    pub fn display<D: Display>(&self, disp: &mut D) -> Result<(), D::Error> {
        let offset = match self.text.len() {
            x if x < 20 => (20 - x) / 2,
            _ => 0,
        } as u8;

        disp.set_cursor_position(1, offset)?;
        disp.write(self.text.as_bytes())
    }
}
