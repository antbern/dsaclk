use core::convert::TryInto;

use serde::{Deserialize, Serialize};

use crate::{
    clock::ClockState,
    mpu,
    sdcard::{self, Settings, SD_BLOCK_SIZE},
    SdCard,
};

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

pub struct Logger {
    buffer: [u8; SD_BLOCK_SIZE * 2],
    current_idx: usize,
}

impl Logger {
    pub fn new() -> Self {
        Self {
            buffer: [0u8; SD_BLOCK_SIZE * 2],
            current_idx: 0,
        }
    }

    /// Appends an new LogEntry containing the LogContents to the SD-card log
    pub fn append(
        &mut self,
        clock: &ClockState,
        contents: LogContents,
        sd_card: &mut SdCard,
        settings: &mut Settings,
    ) -> Result<(), sdcard::Error> {
        // construct a new logger entry with timestamp (TODO)
        let entry = LogEntry {
            timestamp: clock.into(),
            contents,
        };

        // get a mutable slice of the remaining content in the buffer array
        let buff = &mut self.buffer[self.current_idx..];

        // serialize the LogEntry to the buffer, note that this assumes that the entire LogEntry fits into the remaining buffer space
        let serialized =
            postcard::to_slice(&entry, buff).map_err(|e| sdcard::Error::PostcardError(e))?;

        // increment next buffer index
        self.current_idx += serialized.len();

        // if we have enough serialized to write to the SD-card, do it
        if self.current_idx >= SD_BLOCK_SIZE {
            // take out the data to write (note this should NEVER fail due to the explicit size being exact)
            let data_to_write: &[u8; SD_BLOCK_SIZE] =
                &self.buffer[..SD_BLOCK_SIZE].try_into().unwrap();

            sd_card.write_block(settings.logger_block, data_to_write)?;

            // store the settings with the new index
            settings.logger_block += 1;
            sd_card.store_settings(*settings).unwrap();

            // adjust the index pointer
            self.current_idx -= SD_BLOCK_SIZE;

            // shift remaining data to the beginning of the buffer
            for i in 0..self.current_idx {
                self.buffer[i] = self.buffer[i + SD_BLOCK_SIZE];
            }

            defmt::debug!("Wrote block at address {}", &settings.logger_block - 1);
        }
        Ok(())
    }
}
