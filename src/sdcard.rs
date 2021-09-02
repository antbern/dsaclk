use embedded_hal::blocking::delay::DelayMs;
use stm32f4xx_hal::sdio::{self, Sdio};

use postcard::{from_bytes, to_slice};
use serde::{Deserialize, Serialize};

const SDIO_BLOCK_SETTINGS: u32 = 1;
const SDIO_RETRY_INTERVAL_MS: u8 = 100;

pub const SD_BLOCK_SIZE: usize = 512;
const LOGGER_BLOCK_START_IDX: u32 = 10;

#[derive(Debug)]
pub enum Error {
    InitTimeout,
    SdioError(sdio::Error),
    PostcardError(postcard::Error),
}
pub struct SdCard {
    sdio: Sdio,
}

impl SdCard {
    pub fn init<D: DelayMs<u8>>(
        mut sdio: Sdio,
        delay: &mut D,
        timeout: u32,
    ) -> Result<SdCard, Error> {
        let tries = timeout / SDIO_RETRY_INTERVAL_MS as u32;

        for _ in 0..tries {
            match sdio.init_card(sdio::ClockFreq::F12Mhz) {
                Ok(_) => return Ok(SdCard { sdio }),
                Err(e) => match e {
                    sdio::Error::Timeout => (),
                    x => return Err(Error::SdioError(x)),
                },
            }
            delay.delay_ms(SDIO_RETRY_INTERVAL_MS);
        }

        Err(Error::InitTimeout)
    }

    pub fn load_settings(&mut self) -> Result<Settings, Error> {
        // read bytes
        let mut block = [0u8; 512];

        self.sdio
            .read_block(SDIO_BLOCK_SETTINGS, &mut block)
            .map_err(|e| Error::SdioError(e))?;

        // parse using postcard
        from_bytes::<Settings>(&block).map_err(|e| Error::PostcardError(e))
    }

    pub fn store_settings(&mut self, settings: Settings) -> Result<(), Error> {
        let mut block = [0u8; 512];

        to_slice(&settings, &mut block).map_err(|e| Error::PostcardError(e))?;

        self.sdio
            .write_block(SDIO_BLOCK_SETTINGS, &block)
            .map_err(|e| Error::SdioError(e))
    }

    pub fn write_block(&mut self, addr: u32, block: &[u8; SD_BLOCK_SIZE]) -> Result<(), Error> {
        self.sdio
            .write_block(addr, block)
            .map_err(|e| Error::SdioError(e))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Settings {
    pub logger_block: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            logger_block: LOGGER_BLOCK_START_IDX,
        }
    }
}
