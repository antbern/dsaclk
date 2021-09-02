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

        // write the blocks while we have enough serialized data available
        while self.current_idx >= SD_BLOCK_SIZE {
            self.write_first_block(sd_card, settings)?;
        }
        Ok(())
    }

    /// Writes the first block in the buffer to the SD-card, shifts the buffer contents, and updates the `current_idx` pointer.
    /// If the first block is not full (ie `current_idx < SD_BLOCK_SIZE`), the remaining bytes are zeroed before writing and the `current_idx` pointer is reset to zero.
    // If `current_idx == 0`, the function does nothing.
    fn write_first_block(
        &mut self,
        sd_card: &mut SdCard,
        settings: &mut Settings,
    ) -> Result<(), sdcard::Error> {
        // do nothing if we have no buffered bytes
        if self.current_idx == 0 {
            return Ok(());
        }

        // less than a complete block left, fill the remaining with zeroes (just for keeping the blocks nice and tidy when reading later)
        if self.current_idx < SD_BLOCK_SIZE {
            self.buffer[self.current_idx..].fill(0);
        }

        // extract an array of one block and write it to the SD-card
        let data_to_write: &[u8; SD_BLOCK_SIZE] = &self.buffer[..SD_BLOCK_SIZE].try_into().unwrap();
        sd_card.write_block(settings.logger_block, data_to_write)?;

        // update the logger_block index
        settings.logger_block += 1;
        sd_card.store_settings(*settings).unwrap();

        if self.current_idx < SD_BLOCK_SIZE {
            // fill the beginning of the buffer with zeroes as well (now the entire buffer should be zeroed)
            self.buffer[0..self.current_idx].fill(0);
            self.current_idx = 0;
        } else {
            // adjust the index pointer
            self.current_idx -= SD_BLOCK_SIZE;

            // shift remaining data to the beginning of the buffer
            for i in 0..self.current_idx {
                self.buffer[i] = self.buffer[i + SD_BLOCK_SIZE];
            }
        }

        defmt::debug!("Wrote block at address {}", &settings.logger_block - 1);

        Ok(())
    }

    /// Forces the Logger to write the rest of the buffered serialized LogEntries to the SD-card.
    /// Should be called when one wants to stop logging.
    pub fn flush(
        &mut self,
        sd_card: &mut SdCard,
        settings: &mut Settings,
    ) -> Result<(), sdcard::Error> {
        // do nothing if we have no buffered bytes
        if self.current_idx == 0 {
            return Ok(());
        }

        // write blocks until there is no more data to write
        while self.current_idx > 0 {
            self.write_first_block(sd_card, settings)?;
        }

        Ok(())
    }
}
