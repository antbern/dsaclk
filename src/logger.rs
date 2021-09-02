use core::convert::TryInto;

use postcard::to_slice;
use serde::{Deserialize, Serialize};

use crate::{
    clock::ClockState,
    mpu,
    sdcard::{Settings, SD_BLOCK_SIZE},
    SdCard,
};

/// the global static logger instance
// pub static LOGGER: Mutex<RefCell<Logger<{ 512 * 10 }>>> =
// Mutex::new(RefCell::new(Logger::new(100)));

const BUFFER_SD_PAGES: usize = 3;

static mut LOGGER_BUFFER: [u8; SD_BLOCK_SIZE * BUFFER_SD_PAGES] =
    [0; SD_BLOCK_SIZE * BUFFER_SD_PAGES];

// pub static LOGGer: GlobalCell<Logger<{ 512 * 10 }>> = GlobalCell::new(Some(Logger::new(100));
#[derive(Serialize, Deserialize, Debug)]
pub struct LogEntry {
    timestamp: LogTimestamp,
    contents: LogContents,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum LogContents {
    Measurement(mpu::Measurement),
    Alarm(),
}

/// An internal type used to represent the time instant a log entry is logged.
#[derive(Serialize, Deserialize, Debug)]
struct LogTimestamp {
    hour: u8,
    minute: u8,
    second: u8,
    day: u8,
    month: u8,
    year: u8,
}

impl From<&ClockState> for LogTimestamp {
    fn from(s: &ClockState) -> Self {
        Self {
            hour: s.hour,
            minute: s.minute,
            second: s.second,
            day: s.day,
            month: s.month,
            year: s.year,
        }
    }
}

pub struct Logger<'a, const N: usize> {
    timeout_ticks: u32,
    timeout_counter: u32,
    buffer: Option<&'a mut [u8]>,
    // current_slice: &'static [u8],
}

impl<'a, const N: usize> Logger<'a, N> {
    pub fn new(timeout: u32) -> Logger<'a, N> {
        Logger {
            timeout_ticks: timeout,
            timeout_counter: 0,
            buffer: Some(unsafe { LOGGER_BUFFER.as_mut() }),
            // current_slice: &[],
        }
    }

    /// let the logger update (if it needs to)
    pub fn tick(&mut self, sd_card: &mut SdCard, settings: &mut Settings) {
        self.timeout_counter += 1;

        if self.timeout_counter >= self.timeout_ticks {
            self.timeout_counter = 0;
            self.flush(sd_card, settings);
        }
    }

    pub fn append(
        &mut self,
        clock: &ClockState,
        contents: LogContents,
        sd_card: &mut SdCard,
        settings: &mut Settings,
    ) {
        // construct a new logger entry with timestamp (TODO)
        let entry = LogEntry {
            timestamp: clock.into(),
            contents,
        };

        // if serial buffer is full, flush the buffer to the SD card and try again
        'outer: loop {
            let buffer = self.buffer.take().unwrap();

            // serialize the new entry into the buffer, return the used portion
            let used_len = match to_slice(&entry, buffer) {
                Ok(x) => x.len(),
                Err(e) if e == postcard::Error::SerializeBufferFull => {
                    //  flush the buffer / write to SD card and reset (put the buffer back)
                    self.buffer = Some(buffer);
                    self.flush(sd_card, settings);
                    break 'outer;
                }
                Err(_) => panic!(), // TODO
            };

            // success

            // since to_slice only returns the used parts (even though the documentation states otherwise)
            // we have to modify the internal buffer to point correctly
            let (_, unused) = buffer.split_at_mut(used_len);

            self.buffer = Some(unused);

            // defmt::debug!("Used: {}", &used);
            defmt::debug!(
                "Self.buffer.len() = {}",
                self.buffer.as_ref().and_then(|f| Some(f.len()))
            );
            break;
        }
    }

    fn flush(&mut self, sd_card: &mut SdCard, settings: &mut Settings) {
        // fill the rest of the buffer with zeros
        let buffer = self.buffer.take().unwrap();

        // find out how many sd-card pages we need to write
        let bytes_used = unsafe { LOGGER_BUFFER.len() } - buffer.len();

        let sd_pages_used = if bytes_used % SD_BLOCK_SIZE > 0 {
            bytes_used / SD_BLOCK_SIZE + 1
        } else {
            bytes_used / SD_BLOCK_SIZE
        };

        assert!(sd_pages_used <= BUFFER_SD_PAGES);

        // reset the buffer slice to point at the entire allocated buffer
        let buffer = unsafe { LOGGER_BUFFER.as_mut() };

        // fill the rest with zeros
        for i in bytes_used..(sd_pages_used * SD_BLOCK_SIZE) {
            buffer[i] = 0;
        }

        // write the entire logger buffer to the sd card
        let start_block: u32 = settings.logger_block; // TODO: load at startup and store after writing
        for i in 0..sd_pages_used {
            sd_card
                .write_block(
                    start_block + i as u32,
                    &buffer[(i * SD_BLOCK_SIZE)..((i + 1) * SD_BLOCK_SIZE)]
                        .try_into()
                        .expect("slice incorrect length"),
                )
                .expect("Error writing log block");
        }

        settings.logger_block += sd_pages_used as u32;

        sd_card.store_settings(*settings).unwrap();

        defmt::debug!(
            "Wrote {} blocks starting at block {}",
            sd_pages_used,
            start_block
        );

        // store the buffer for reusing
        self.buffer = Some(buffer);
    }
}
