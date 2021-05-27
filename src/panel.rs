use crate::display::Display;
use crate::SharedState;

const STR_DECIMAL_10: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

const STR_DECIMAL_60: [&str; 60] = [
    "00", "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13", "14", "15",
    "16", "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "30", "31",
    "32", "33", "34", "35", "36", "37", "38", "39", "40", "41", "42", "43", "44", "45", "46", "47",
    "48", "49", "50", "51", "52", "53", "54", "55", "56", "57", "58", "59",
];

pub enum Panels {
    Time,
    Alarm,
}
#[derive(PartialEq)]
pub enum CursorState {
    Off,
    Underline(u8, u8),
    Blinking(u8, u8),
}
pub trait Panel<D: Display> {
    fn next(&mut self, state: &mut SharedState);
    fn previous(&mut self, state: &mut SharedState);
    fn enter(&mut self, state: &mut SharedState);
    fn leave(&mut self, state: &mut SharedState);
    fn display(&self, disp: &mut D, state: &mut SharedState) -> Result<(), D::Error>;
    fn get_cursor_state(&self, state: &SharedState) -> CursorState;
}

pub mod time {
    use super::DecF;
    use crate::display::Display;
    use crate::SharedState;

    use super::CursorState;

    enum SelectedField {
        Hour,
        Minute,
        Second,
    }

    pub struct TimePanel {
        in_edit: bool,
        selected: SelectedField,
    }

    impl TimePanel {
        pub fn new() -> Self {
            TimePanel {
                in_edit: false,
                selected: SelectedField::Hour,
            }
        }
    }

    impl<D: Display> crate::panel::Panel<D> for TimePanel {
        fn enter(&mut self, _state: &mut SharedState) {
            self.in_edit = !self.in_edit;
        }

        fn leave(&mut self, _state: &mut SharedState) {
            self.in_edit = false;
        }

        fn next(&mut self, state: &mut SharedState) {
            if self.in_edit {
                match self.selected {
                    SelectedField::Hour => DecF::next(&mut state.hour, 23),
                    SelectedField::Minute => DecF::next(&mut state.minute, 59),
                    SelectedField::Second => DecF::next(&mut state.second, 59),
                }
            } else {
                self.selected = match self.selected {
                    SelectedField::Hour => SelectedField::Minute,
                    SelectedField::Minute => SelectedField::Second,
                    SelectedField::Second => SelectedField::Hour,
                }
            }
        }

        fn previous(&mut self, state: &mut SharedState) {
            if self.in_edit {
                match self.selected {
                    SelectedField::Hour => DecF::previous(&mut state.hour, 23),
                    SelectedField::Minute => DecF::previous(&mut state.minute, 59),
                    SelectedField::Second => DecF::previous(&mut state.second, 59),
                }
            } else {
                self.selected = match self.selected {
                    SelectedField::Hour => SelectedField::Second,
                    SelectedField::Minute => SelectedField::Hour,
                    SelectedField::Second => SelectedField::Minute,
                }
            }
        }

        fn display(&self, disp: &mut D, state: &mut SharedState) -> Result<(), D::Error> {
            disp.set_cursor_position(0, 0)?;
            disp.write(b"Time")?;

            disp.set_cursor_position(2, 3)?;
            disp.write(DecF::get_str(state.hour, 23).as_bytes())?;
            disp.write(b":")?;
            disp.write(DecF::get_str(state.minute, 59).as_bytes())?;
            disp.write(b":")?;
            disp.write(DecF::get_str(state.second, 59).as_bytes())?;

            Ok(())
        }

        fn get_cursor_state(&self, _state: &SharedState) -> CursorState {
            let row = 2;
            let col = 3 + match self.selected {
                SelectedField::Hour => 1,
                SelectedField::Minute => 4,
                SelectedField::Second => 7,
            };

            match self.in_edit {
                true => CursorState::Blinking(row, col),
                false => CursorState::Underline(row, col),
            }
        }
    }
}

// empty struct only containing static methods for dealing with on/off values
struct OnOffF {}

impl OnOffF {
    fn next(state: &mut bool) {
        *state = !*state
    }

    fn previous(state: &mut bool) {
        *state = !*state
    }

    fn get_str(state: bool) -> &'static str {
        match state {
            true => "ON",
            false => "OFF",
        }
    }
}

struct DecF {}

impl DecF {
    fn next(state: &mut u8, max: u8) {
        if *state >= max - 1 {
            *state = 0
        } else {
            *state += 1
        }
    }

    fn previous(state: &mut u8, max: u8) {
        if *state == 0 {
            *state = max
        } else {
            *state -= 1
        }
    }

    fn get_str(state: u8, max: u8) -> &'static str {
        if max < 10 {
            match state {
                n if n <= max => STR_DECIMAL_10[n as usize],
                _ => "?",
            }
        } else {
            match state {
                n if n <= max && max < 60 => STR_DECIMAL_60[n as usize],
                _ => "??",
            }
        }
    }
}

struct WeekdayF {}

impl WeekdayF {
    fn next(state: &mut u8) {
        if *state >= 6 {
            *state = 0
        } else {
            *state += 1
        }
    }

    fn previous(state: &mut u8) {
        if *state == 0 {
            *state = 6
        } else {
            *state -= 1
        }
    }

    fn get_str(state: u8) -> &'static str {
        match state {
            0 => "MON",
            1 => "TUE",
            2 => "WED",
            3 => "THU",
            4 => "FRI",
            5 => "SAT",
            6 => "SUN",
            _ => "XXX",
        }
    }
}
