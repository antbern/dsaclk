use cortex_m::delay::Delay;
use embedded_hal::blocking::delay::DelayMs;
use stm32f4xx_hal::sdio::{self, Card, Sdio};

use postcard::{from_bytes, to_slice};
use serde::{Deserialize, Serialize};

const SDIO_BLOCK_SETTINGS: u32 = 1;
const SDIO_RETRY_INTERVAL: u8 = 100;

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
        let tries = timeout / SDIO_RETRY_INTERVAL as u32;

        for _ in 0..tries {
            match sdio.init_card(sdio::ClockFreq::F12Mhz) {
                Ok(_) => return Ok(SdCard { sdio }),
                Err(e) => match e {
                    sdio::Error::Timeout => (),
                    x => return Err(Error::SdioError(x)),
                },
            }
            delay.delay_ms(SDIO_RETRY_INTERVAL);
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
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Settings {
    pub id: u32,
}
