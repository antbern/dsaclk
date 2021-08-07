#![allow(dead_code)]
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
    use super::{DecF, MonthF, WeekdayF};
    use crate::display::Display;
    use crate::SharedState;

    use super::CursorState;

    enum SelectedField {
        Hour,
        Minute,
        Second,
        Weekday,
        Day,
        Month,
        Year,
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
                    SelectedField::Hour => DecF::next(&mut state.clock.hour, 0, 23),
                    SelectedField::Minute => DecF::next(&mut state.clock.minute, 0, 59),
                    SelectedField::Second => DecF::next(&mut state.clock.second, 0, 59),
                    SelectedField::Weekday => WeekdayF::next(&mut state.clock.weekday),
                    SelectedField::Day => DecF::next(&mut state.clock.day, 1, 31),
                    SelectedField::Month => MonthF::next(&mut state.clock.month),
                    SelectedField::Year => DecF::next(&mut state.clock.year, 0, 40),
                }
            } else {
                self.selected = match self.selected {
                    SelectedField::Hour => SelectedField::Minute,
                    SelectedField::Minute => SelectedField::Second,
                    SelectedField::Second => SelectedField::Weekday,
                    SelectedField::Weekday => SelectedField::Day,
                    SelectedField::Day => SelectedField::Month,
                    SelectedField::Month => SelectedField::Year,
                    SelectedField::Year => SelectedField::Hour,
                }
            }
        }

        fn previous(&mut self, state: &mut SharedState) {
            if self.in_edit {
                match self.selected {
                    SelectedField::Hour => DecF::previous(&mut state.clock.hour, 0, 23),
                    SelectedField::Minute => DecF::previous(&mut state.clock.minute, 0, 59),
                    SelectedField::Second => DecF::previous(&mut state.clock.second, 0, 59),
                    SelectedField::Weekday => WeekdayF::previous(&mut state.clock.weekday),
                    SelectedField::Day => DecF::previous(&mut state.clock.day, 1, 31),
                    SelectedField::Month => MonthF::previous(&mut state.clock.month),
                    SelectedField::Year => DecF::previous(&mut state.clock.year, 0, 40),
                }
            } else {
                self.selected = match self.selected {
                    SelectedField::Hour => SelectedField::Year,
                    SelectedField::Minute => SelectedField::Hour,
                    SelectedField::Second => SelectedField::Minute,
                    SelectedField::Weekday => SelectedField::Second,
                    SelectedField::Day => SelectedField::Weekday,
                    SelectedField::Month => SelectedField::Day,
                    SelectedField::Year => SelectedField::Month,
                }
            }
        }

        fn display(&self, disp: &mut D, state: &mut SharedState) -> Result<(), D::Error> {
            disp.set_cursor_position(0, 0)?;
            disp.write(b"Time")?;

            disp.set_cursor_position(2, 3)?;
            disp.write(DecF::get_str(state.clock.hour, 0, 23).as_bytes())?;
            disp.write(b":")?;
            disp.write(DecF::get_str(state.clock.minute, 0, 59).as_bytes())?;
            disp.write(b":")?;
            disp.write(DecF::get_str(state.clock.second, 0, 59).as_bytes())?;

            disp.set_cursor_position(3, 2)?;
            disp.write(WeekdayF::get_str(state.clock.weekday).as_bytes())?;

            disp.set_cursor_position(3, 6)?;
            disp.write(DecF::get_str(state.clock.day, 1, 31).as_bytes())?;

            disp.set_cursor_position(3, 9)?;
            disp.write(MonthF::get_str(state.clock.month).as_bytes())?;

            disp.set_cursor_position(3, 13)?;
            disp.write(b"20")?;
            disp.write(DecF::get_str(state.clock.year, 0, 40).as_bytes())?;

            Ok(())
        }

        fn get_cursor_state(&self, _state: &SharedState) -> CursorState {
            use SelectedField::*;

            let row = 2 + match self.selected {
                Hour | Minute | Second => 0,
                Weekday | Day | Month | Year => 1,
            };

            let col = 1 + match self.selected {
                Hour => 3,
                Minute => 6,
                Second => 9,
                Weekday => 3,
                Day => 6,
                Month => 10,
                Year => 15,
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
    fn next(state: &mut u8, min: u8, max: u8) {
        if *state >= max - 1 {
            *state = min
        } else {
            *state += 1
        }
    }

    fn previous(state: &mut u8, min: u8, max: u8) {
        if *state <= min {
            *state = max
        } else {
            *state -= 1
        }
    }

    fn get_str(state: u8, min: u8, max: u8) -> &'static str {
        if max < 10 {
            match state {
                n if min <= n && n <= max => STR_DECIMAL_10[n as usize],
                _ => "?",
            }
        } else {
            match state {
                n if min <= n && n <= max && max < 60 => STR_DECIMAL_60[n as usize],
                _ => "??",
            }
        }
    }
}

struct WeekdayF {}

impl WeekdayF {
    fn next(state: &mut u8) {
        if *state >= 7 {
            *state = 1
        } else {
            *state += 1
        }
    }

    fn previous(state: &mut u8) {
        if *state <= 1 {
            *state = 7
        } else {
            *state -= 1
        }
    }

    fn get_str(state: u8) -> &'static str {
        match state {
            1 => "MON",
            2 => "TUE",
            3 => "WED",
            4 => "THU",
            5 => "FRI",
            6 => "SAT",
            7 => "SUN",
            _ => "XXX",
        }
    }
}

struct MonthF {}

impl MonthF {
    fn next(state: &mut u8) {
        if *state >= 12 {
            *state = 1
        } else {
            *state += 1
        }
    }

    fn previous(state: &mut u8) {
        if *state <= 1 {
            *state = 12
        } else {
            *state -= 1
        }
    }

    fn get_str(state: u8) -> &'static str {
        match state {
            1 => "JAN",
            2 => "FEB",
            3 => "MAR",
            4 => "APR",
            5 => "MAY",
            6 => "JUN",
            7 => "JUL",
            8 => "AUG",
            9 => "SEP",
            10 => "OCT",
            11 => "NOV",
            12 => "DEC",
            _ => "XXX",
        }
    }
}
