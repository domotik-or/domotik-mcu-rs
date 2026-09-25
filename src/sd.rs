use cortex_m;
#[cfg(feature = "defmt")]
use defmt::*;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiBus;

use crate::utils::crc16;

// fn sd_crc(data: &[u8]) -> u8 {
//     (crc7(data) << 1) | 1
// }


#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub enum SdError {
    Spi,
    Timeout,
    BadResponse,
    InvalidCard,
    BadCrc
}

pub trait BlockReader {
    fn read_block(
        &mut self,
        address: u32,
        buffer: &mut [u8; 512],
    ) -> Result<(), SdError>;
}

/// Temporary boot-time owner of the SPI peripheral and SD card CS.
///
/// The SPI peripheral is deliberately kept separate from `ExclusiveDevice`
/// because ownership must be returned after SD access is finished.
pub struct SdSpi<SPI, CS> {
    spi: SPI,
    cs: CS,

    // Set after initialization.
    block_addressing: bool,
}

impl<SPI, CS> SdSpi<SPI, CS>
where
    SPI: SpiBus<u8>,
    CS: OutputPin,
{
    pub fn new(mut spi: SPI, mut cs: CS) -> Result<Self, SdError> {
        cs.set_high().map_err(|_| SdError::Spi)?;

        // SD SPI mode requires >= 74 clock cycles with CS high.
        //
        // 10 bytes = 80 clocks.
        let dummy = [0xffu8; 10];
        spi.write(&dummy).map_err(|_| SdError::Spi)?;

        Ok(Self {
            spi,
            cs,
            block_addressing: false,
        })
    }

    /// Give the SPI peripheral and SD CS back to the caller.
    ///
    /// After this returns, this SdSpi no longer owns either resource.
    pub fn release(self) -> (SPI, CS) {
        (self.spi, self.cs)
    }

    fn read_ff(&mut self, data: &mut [u8]) -> Result<(), SdError> {
        data.fill(0xff);

        self.spi.transfer_in_place(data).map_err(|_| SdError::Spi)?;

        Ok(())
    }

    fn command(&mut self, cmd: u8, arg: u32, crc: u8) -> Result<u8, SdError> {
        let packet = [0x40 | cmd, (arg >> 24) as u8, (arg >> 16) as u8, (arg >> 8) as u8, arg as u8, crc];

        self.select()?;

        // Send command.
        self.spi.write(&packet).map_err(|_| SdError::Spi)?;

        // SD cards return 0xff while they are not ready.
        let mut response = [0xffu8];

        for _ in 0..8 {
            self.spi.transfer_in_place(&mut response).map_err(|_| SdError::Spi)?;

            if response[0] != 0xff {
                return Ok(response[0]);
            }
        }

        Err(SdError::Timeout)
    }

    fn select(&mut self) -> Result<(), SdError> {
        // Give the card clocks while deselected before asserting CS.
        self.spi.write(&[0xff]).map_err(|_| SdError::Spi)?;

        self.cs.set_low().map_err(|_| SdError::Spi)?;

        // clocks after CS goes low
        self.spi.write(&[0xff]).map_err(|_| SdError::Spi)?;

        Ok(())
    }

    fn deselect(&mut self) -> Result<(), SdError> {
        // Give the card a final byte of clocks while it is still selected.
        self.spi.write(&[0xff]).map_err(|_| SdError::Spi)?;

        self.cs.set_high().map_err(|_| SdError::Spi)?;

        // Give the card at least one extra clock with CS high.
        self.spi.write(&[0xff]).map_err(|_| SdError::Spi)?;

        Ok(())
    }

    pub fn init(&mut self) -> Result<(), SdError> {
        // CMD0: GO_IDLE_STATE
        let response = self.command(0, 0, 0x95)?;

        if response != 0x01 {
            let _ = self.deselect();
            return Err(SdError::BadResponse);
        }

        self.deselect()?;

        // CMD8: SEND_IF_COND, Voltage = 2.7-3.6 V, Check pattern = 0xAA
        let response = self.command(8, 0x0000_01aa, 0x87)?;

        if response != 0x01 {
            let _ = self.deselect();
            return Err(SdError::InvalidCard);
        }

        let mut r7 = [0u8; 4];
        self.read_ff(&mut r7).map_err(|_| SdError::Spi)?;

        self.deselect()?;

        if r7[2] != 0x01 || r7[3] != 0xaa {
            return Err(SdError::InvalidCard);
        }

        // ACMD41 with HCS. Keep trying until the card leaves idle state.
        for _ in 0..100 {
            // CMD55
            let r1 = self.command(55, 0, 0x01)?;
            self.deselect()?;

            if r1 > 0x01 {
                return Err(SdError::BadResponse);
            }

            // ACMD41
            let r1 = self.command(41, 0x4000_0000, 0x01)?;

            if r1 == 0x00 {
                self.deselect()?;
                break;
            }

            self.deselect()?;

            cortex_m::asm::delay(160_000);
        }

        // CMD58: READ_OCR
        let r1 = self.command(58, 0, 0x01)?;

        if r1 != 0x00 {
            let _ = self.deselect();
            return Err(SdError::BadResponse);
        }

        // Clock the response out with MOSI high.
        let mut ocr = [0u8; 4];
        self.read_ff(&mut ocr)?;

        self.deselect()?;

        // CCS bit tells us whether the card uses block addressing.
        self.block_addressing = (ocr[0] & 0x40) != 0;

        // SDSC cards need a 512-byte block length.
        if !self.block_addressing {
            let r1 = self.command(16, 512, 0x01)?;

            if r1 != 0x00 {
                let _ = self.deselect();
                return Err(SdError::BadResponse);
            }

            self.deselect()?;
        }

        Ok(())
    }
}

impl<SPI, CS> BlockReader for SdSpi<SPI, CS>
where
    SPI: SpiBus<u8>,
    CS: OutputPin,
{
    fn read_block(
        &mut self,
        address: u32,
        buffer: &mut [u8; 512],
    ) -> Result<(), SdError> {
        let argument = if self.block_addressing {
            // SDHC / SDXC: block address
            address
        } else {
            // SDSC: byte address
            address
                .checked_mul(512)
                .ok_or(SdError::InvalidCard)?
        };

        // CMD17: READ_SINGLE_BLOCK
        let r1 = self.command(17, argument, 0x01)?;

        if r1 != 0x00 {
            let _ = self.deselect();
            return Err(SdError::BadResponse);
        }

        // Wait for data token 0xFE.
        let mut token = [0u8];

        for _ in 0..100_000 {
            self.read_ff(&mut token).map_err(|_| SdError::Spi)?;

            if token[0] == 0xfe {
                break;
            }

            cortex_m::asm::delay(160_000);
        }

        if token[0] != 0xfe {
            let _ = self.deselect();
            return Err(SdError::Timeout);
        }

        // Read the actual 512-byte sector.
        self.read_ff(buffer).map_err(|_| SdError::Spi)?;

        // Read CRC16.
        let mut crc = [0u8; 2];
        self.read_ff(&mut crc).map_err(|_| SdError::Spi)?;

        let received_crc = u16::from_be_bytes(crc);
        let calculated_crc = crc16(buffer);

        if received_crc != calculated_crc {
            self.deselect()?;
            return Err(SdError::BadCrc);
        }

        self.deselect()?;

        Ok(())
    }
}
