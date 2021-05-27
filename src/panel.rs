use crate::display::Display;
use crate::SharedState;

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
        HourH,
        HourL,
        MinuteH,
        MinuteL,
        SecondH,
        SecondL,
    }

    pub struct TimePanel {
        in_edit: bool,
        selected: SelectedField,
    }

    impl TimePanel {
        pub fn new() -> Self {
            TimePanel {
                in_edit: false,
                selected: SelectedField::HourH,
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
                    SelectedField::HourH => DecF::next(&mut state.hour_h),
                    SelectedField::HourL => DecF::next(&mut state.hour_l),
                    SelectedField::MinuteH => DecF::next(&mut state.minute_h),
                    SelectedField::MinuteL => DecF::next(&mut state.minute_l),
                    SelectedField::SecondH => DecF::next(&mut state.second_h),
                    SelectedField::SecondL => DecF::next(&mut state.second_l),
                }
            } else {
                self.selected = match self.selected {
                    SelectedField::HourH => SelectedField::HourL,
                    SelectedField::HourL => SelectedField::MinuteH,
                    SelectedField::MinuteH => SelectedField::MinuteL,
                    SelectedField::MinuteL => SelectedField::SecondH,
                    SelectedField::SecondH => SelectedField::SecondL,
                    SelectedField::SecondL => SelectedField::HourH,
                }
            }
        }

        fn previous(&mut self, state: &mut SharedState) {
            if self.in_edit {
                match self.selected {
                    SelectedField::HourH => DecF::previous(&mut state.hour_h),
                    SelectedField::HourL => DecF::previous(&mut state.hour_l),
                    SelectedField::MinuteH => DecF::previous(&mut state.minute_h),
                    SelectedField::MinuteL => DecF::previous(&mut state.minute_l),
                    SelectedField::SecondH => DecF::previous(&mut state.second_h),
                    SelectedField::SecondL => DecF::previous(&mut state.second_l),
                }
            } else {
                self.selected = match self.selected {
                    SelectedField::HourH => SelectedField::SecondL,
                    SelectedField::HourL => SelectedField::HourH,
                    SelectedField::MinuteH => SelectedField::HourL,
                    SelectedField::MinuteL => SelectedField::MinuteH,
                    SelectedField::SecondH => SelectedField::MinuteL,
                    SelectedField::SecondL => SelectedField::SecondH,
                }
            }
        }

        fn display(&self, disp: &mut D, state: &mut SharedState) -> Result<(), D::Error> {
            disp.set_cursor_position(0, 0)?;
            disp.write(b"Time")?;

            disp.set_cursor_position(2, 3)?;
            disp.write(DecF::get_str(state.hour_h).as_bytes())?;
            disp.write(DecF::get_str(state.hour_l).as_bytes())?;
            disp.write(b":")?;
            disp.write(DecF::get_str(state.minute_h).as_bytes())?;
            disp.write(DecF::get_str(state.minute_l).as_bytes())?;
            disp.write(b":")?;
            disp.write(DecF::get_str(state.second_h).as_bytes())?;
            disp.write(DecF::get_str(state.second_l).as_bytes())?;

            // set the cursor to the selected
            Ok(())
        }

        fn get_cursor_state(&self, _state: &SharedState) -> CursorState {
            let row = 2;
            let col = 3 + match self.selected {
                SelectedField::HourH => 0,
                SelectedField::HourL => 1,
                SelectedField::MinuteH => 3,
                SelectedField::MinuteL => 4,
                SelectedField::SecondH => 6,
                SelectedField::SecondL => 7,
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
    fn next(state: &mut u8) {
        if *state >= 9 {
            *state = 0
        } else {
            *state += 1
        }
    }

    fn previous(state: &mut u8) {
        if *state == 0 {
            *state = 9
        } else {
            *state -= 1
        }
    }

    fn get_str(state: u8) -> &'static str {
        match state {
            0 => "0",
            1 => "1",
            2 => "2",
            3 => "3",
            4 => "4",
            5 => "5",
            6 => "6",
            7 => "7",
            8 => "8",
            9 => "9",
            _ => "X",
        }
    }
}
