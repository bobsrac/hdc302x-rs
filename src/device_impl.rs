use crate::hw_def::*;
use crate::types::*;

use cfg_if::cfg_if;

#[cfg(feature = "crc")]
use crc::{Crc, CRC_8_NRSC_5};

#[cfg(feature = "defmt")]
use defmt::{trace, warn};
#[cfg(feature = "log")]
use log::{trace, warn};
#[cfg(not(any(feature = "defmt", feature = "log")))]
macro_rules! trace {
    ($($arg:tt)*) => {};
}
#[cfg(not(any(feature = "defmt", feature = "log")))]
macro_rules! warn {
    ($($arg:tt)*) => {};
}

#[cfg(feature = "crc")]
const CRC: crc::Crc<u8> = Crc::<u8>::new(&CRC_8_NRSC_5);

impl<I2C, Delay> Hdc302x<I2C, Delay> {
    /// Create a new HDC302x driver instance
    pub fn new(i2c: I2C, delay: Delay, i2c_addr: I2cAddr) -> Self {
        Self { i2c, delay, i2c_addr }
    }
}

#[cfg(feature = "blocking")]
impl<I2C, Delay, E> Hdc302x<I2C, Delay>
where
    I2C: embedded_hal::i2c::I2c<Error = E>,
    Delay: embedded_hal::delay::DelayNs,
{
    fn cmd_delay_read(&mut self, cmd_bytes: &[u8; 2], delay_us: Option<u32>, read_vals: &mut [u16]) -> Result<(), Error<E>> {
        let num_vals = read_vals.len();
        // We are heapless, so have to have an upper bound
        assert!(num_vals <= 2);

        if read_vals.is_empty() {
            if let Err(i2c_err) = self.i2c.write(self.i2c_addr.as_u8(), cmd_bytes) {
                return Err(Error::I2c(i2c_err));
            }
        } else {
            let mut read_buf = [0u8; 6];
            let read_buf_slice = &mut read_buf[0..(3 * num_vals)];
            trace!("hdc302x::cmd_delay_read(): read_buf_slice.len()={}", read_buf_slice.len());
            if let Err(i2c_err) = self.i2c.write(self.i2c_addr.as_u8(), cmd_bytes) {
                return Err(Error::I2c(i2c_err));
            }
            if let Some(delay_us) = delay_us {
                self.delay.delay_us(delay_us);
            }
            if let Err(i2c_err) = self.i2c.read(self.i2c_addr.as_u8(), read_buf_slice) {
                return Err(Error::I2c(i2c_err));
            }
            // TODO: consider whether to retry around this failure
            for ii in 0..num_vals {
                let read_word = &read_buf[ii*3..=ii*3+1];
                cfg_if! {
                    if #[cfg(feature = "crc")] {
                        let read_crc = &read_buf[ii*3+2];
                        let crc_expect = CRC.checksum(read_word);
                        if *read_crc != crc_expect {
                            warn!("hdc302x::cmd_delay_read(): crc mismatch word {}/{}: read_buf={:?}, read_word={:?}, read_crc={}, crc_expect={}",
                                ii,
                                num_vals,
                                read_buf,
                                read_word,
                                read_crc,
                                crc_expect);
                            return Err(Error::CrcMismatch);
                        }
                    }
                }
                read_vals[ii] = (read_word[0] as u16) << 8 | read_word[1] as u16;
            }
        }
        Ok(())
    }

    /// Trigger a one-shot measurement and return the raw sample pair
    pub fn one_shot(&mut self, low_power_mode: LowPowerMode) -> Result<RawDatum, Error<E>> {
        let cmd_bytes = start_sampling_command(SampleRate::OneShot, low_power_mode.clone()).to_be_bytes();
        let delay_us = 100 + match low_power_mode {
            LowPowerMode::LPM0 => 12_500,
            LowPowerMode::LPM1 =>  7_500,
            LowPowerMode::LPM2 =>  5_000,
            LowPowerMode::LPM3 =>  3_700,
        };
        let mut read_buf = [0u16; 2];
        self.cmd_delay_read(&cmd_bytes, Some(delay_us), &mut read_buf)?;
        Ok(RawDatum::TempAndRelHumid(RawTempAndRelHumid {
            temperature: read_buf[0],
            humidity: read_buf[1],
        }))
    }

    /// Enter auto mode (continuous self-timed sampling)
    pub fn auto_start(&mut self, sample_rate: SampleRate, low_power_mode: LowPowerMode) -> Result<(), Error<E>> {
        let cmd_bytes = start_sampling_command(sample_rate, low_power_mode).to_be_bytes();
        self.cmd_delay_read(&cmd_bytes, None, &mut [0u16; 0])?;
        Ok(())
    }

    /// exit auto mode and return to sleep
    pub fn auto_stop(&mut self) -> Result<(), Error<E>> {
        self.cmd_delay_read(&Command::AutoExit.as_be_bytes(), None, &mut [0u16; 0])?;
        Ok(())
    }

    /// read most recent temperature and relative humidity from auto mode
    pub fn auto_read(&mut self, target: AutoReadTarget) -> Result<RawDatum, Error<E>> {
        let cmd_bytes = match target {
            AutoReadTarget::LastTempAndRelHumid => Command::AutoReadTempAndRelHumid,
            AutoReadTarget::MinTemp => Command::AutoReadMinTemp,
            AutoReadTarget::MaxTemp => Command::AutoReadMaxTemp,
            AutoReadTarget::MinRelHumid => Command::AutoReadMinRelHumid,
            AutoReadTarget::MaxRelHumid => Command::AutoReadMaxRelHumid,
        }.as_be_bytes();

        let mut read_buf = [0u16; 2];
        let read_buf_slice = match target {
            AutoReadTarget::LastTempAndRelHumid => &mut read_buf[..2],
            AutoReadTarget::MinTemp => &mut read_buf[..1],
            AutoReadTarget::MaxTemp => &mut read_buf[..1],
            AutoReadTarget::MinRelHumid => &mut read_buf[..1],
            AutoReadTarget::MaxRelHumid => &mut read_buf[..1],
        };

        self.cmd_delay_read(&cmd_bytes, None, read_buf_slice)?;

        Ok(match target {
            AutoReadTarget::LastTempAndRelHumid => RawDatum::TempAndRelHumid(RawTempAndRelHumid {
                temperature: read_buf[0],
                humidity: read_buf[1],
            }),
            AutoReadTarget::MinTemp => RawDatum::MinTemp(read_buf[0]),
            AutoReadTarget::MaxTemp => RawDatum::MaxTemp(read_buf[0]),
            AutoReadTarget::MinRelHumid => RawDatum::MinRelHumid(read_buf[0]),
            AutoReadTarget::MaxRelHumid => RawDatum::MaxRelHumid(read_buf[0]),
        })
    }

    /// Condensation heater
    pub fn heater(&mut self, heater_level: HeaterLevel) -> Result<(), Error<E>> {
        self.cmd_delay_read(&Command::HeaterDisable.as_be_bytes(), None, &mut [0u16; 0])?;

        if let Some(setting) = heater_level.setting() {
            let mut cmd_bytes = [0u8; 4];
            cmd_bytes[0..2].copy_from_slice(&Command::HeaterConfig.as_be_bytes());
            cmd_bytes[2..4].copy_from_slice(&setting.to_be_bytes());
            if let Err(i2c_err) = self.i2c.write(self.i2c_addr.as_u8(), &cmd_bytes) {
                return Err(Error::I2c(i2c_err));
            }
            self.cmd_delay_read(&Command::HeaterEnable.as_be_bytes(), None, &mut [0u16; 0])?;
        }
        Ok(())
    }

    /// Read and optionally clear status bits
    pub fn read_status(&mut self, clear: bool) -> Result<StatusBits, Error<E>> {
        let mut read_buf = [0u16; 1];
        self.cmd_delay_read(&Command::StatusRead.as_be_bytes(), None, &mut read_buf)?;
        if clear {
            self.cmd_delay_read(&Command::StatusClear.as_be_bytes(), None, &mut [0u16; 0])?;
        }

        Ok(StatusBits::from(read_buf[0]))
    }

    /// Read the NIST-tracable serial number
    pub fn read_serial_number(&mut self) -> Result<SerialNumber, Error<E>> {
        let mut temp_u16 = [0u16; 1];
        let mut bytes= [0u8; 6];
        self.cmd_delay_read(&Command::SerialID54.as_be_bytes(), None, &mut temp_u16)?;
        bytes[5] = (temp_u16[0] >> 8) as u8;
        bytes[4] = temp_u16[0] as u8;
        self.cmd_delay_read(&Command::SerialID32.as_be_bytes(), None, &mut temp_u16)?;
        bytes[3] = (temp_u16[0] >> 8) as u8;
        bytes[2] = temp_u16[0] as u8;
        self.cmd_delay_read(&Command::SerialID10.as_be_bytes(), None, &mut temp_u16)?;
        bytes[1] = (temp_u16[0] >> 8) as u8;
        bytes[0] = temp_u16[0] as u8;
        Ok(SerialNumber(bytes))
    }

    /// Read the NIST-tracable manufacturer ID
    pub fn read_manufacturer_id(&mut self) -> Result<ManufacturerId, Error<E>> {
        let mut read_buf = [0u16; 1];
        self.cmd_delay_read(&Command::ManufacturerID.as_be_bytes(), None, &mut read_buf)?;
        Ok(ManufacturerId::from(read_buf[0]))
    }

    /// software reset
    pub fn software_reset(&mut self) -> Result<(), Error<E>> {
        self.cmd_delay_read(&Command::SoftReset.as_be_bytes(), None, &mut [0u16; 0])?;
        Ok(())
    }

    // TODO: Support Alerting
    // Command::WriteSetLowAlert,
    // Command::WriteSetHighAlert,
    // Command::WriteClearLowAlert,
    // Command::WriteClearHighAlert,
    // Command::AlertToNV,

    // Command::ReadSetLowAlert,
    // Command::ReadSetHighAlert,
    // Command::ReadClearLowAlert,
    // Command::ReadClearHighAlert,

    // TODO: Support non-volatile offset
    // Command::NVOffset,

    // TODO: Support reset state
    // Command::ResetState,
}

// TODO: consider adding type state pattern around the state of the device.  When we start a
// one-shot, don't do things other than read the result until that happens.  When in auto mode,
// don't do one-shot samples.  When sleeping (not in one-shot or auto mode), don't read auto mode
// results.
#[cfg(feature = "async")]
impl<I2C, Delay, E> Hdc302x<I2C, Delay>
where
    I2C: embedded_hal_async::i2c::I2c<Error = E>,
    Delay: embedded_hal_async::delay::DelayNs,
{
    async fn cmd_delay_read_async(&mut self, cmd_bytes: &[u8; 2], delay_us: Option<u32>, read_vals: &mut [u16]) -> Result<(), Error<E>> {
        let num_vals = read_vals.len();
        // We are heapless, so have to have an upper bound
        assert!(num_vals <= 2);

        if read_vals.is_empty() {
            if let Err(i2c_err) = self.i2c.write(self.i2c_addr.as_u8(), cmd_bytes).await {
                return Err(Error::I2c(i2c_err));
            }
        } else {
            let mut read_buf = [0u8; 6];
            let read_buf_slice = &mut read_buf[0..(3 * num_vals)];
            trace!("hdc302x::cmd_delayread_async(): read_buf_slice.len()={}", read_buf_slice.len());
            if let Err(i2c_err) = self.i2c.write(self.i2c_addr.as_u8(), cmd_bytes).await {
                return Err(Error::I2c(i2c_err));
            }
            if let Some(delay_us) = delay_us {
                self.delay.delay_us(delay_us).await;
            }
            if let Err(i2c_err) = self.i2c.read(self.i2c_addr.as_u8(), read_buf_slice).await {
                return Err(Error::I2c(i2c_err));
            }
            // TODO: consider whether to retry around this failure
            for ii in 0..num_vals {
                let read_word = &read_buf[ii*3..=ii*3+1];
                cfg_if! {
                    if #[cfg(feature = "crc")] {
                        let read_crc = &read_buf[ii*3+2];
                        let crc_expect = CRC.checksum(read_word);
                        if *read_crc != crc_expect {
                            warn!("hdc302x::cmd_delay_read_async(): crc mismatch word {}/{}: read_buf={:?}, read_word={:?}, read_crc={}, crc_expect={}",
                                ii,
                                num_vals,
                                read_buf,
                                read_word,
                                read_crc,
                                crc_expect);
                            return Err(Error::CrcMismatch);
                        }
                    }
                }
                read_vals[ii] = (read_word[0] as u16) << 8 | read_word[1] as u16;
            }
        }
        Ok(())
    }

    /// Trigger a one-shot measurement and return the raw sample pair
    pub async fn one_shot_async(&mut self, low_power_mode: LowPowerMode) -> Result<RawDatum, Error<E>> {
        let cmd_bytes = start_sampling_command(SampleRate::OneShot, low_power_mode.clone()).to_be_bytes();
        let delay_us = 100 + match low_power_mode {
            LowPowerMode::LPM0 => 12_500,
            LowPowerMode::LPM1 =>  7_500,
            LowPowerMode::LPM2 =>  5_000,
            LowPowerMode::LPM3 =>  3_700,
        };
        let mut read_buf = [0u16; 2];
        self.cmd_delay_read_async(&cmd_bytes, Some(delay_us), &mut read_buf).await?;
        Ok(RawDatum::TempAndRelHumid(RawTempAndRelHumid {
            temperature: read_buf[0],
            humidity: read_buf[1],
        }))
    }

    /// Enter auto mode (continuous self-timed sampling)
    pub async fn auto_start_async(&mut self, sample_rate: SampleRate, low_power_mode: LowPowerMode) -> Result<(), Error<E>> {
        let cmd_bytes = start_sampling_command(sample_rate, low_power_mode).to_be_bytes();
        self.cmd_delay_read_async(&cmd_bytes, None, &mut [0u16; 0]).await?;
        Ok(())
    }

    /// exit auto mode and return to sleep
    pub async fn auto_stop_async(&mut self) -> Result<(), Error<E>> {
        self.cmd_delay_read_async(&Command::AutoExit.as_be_bytes(), None, &mut [0u16; 0]).await?;
        Ok(())
    }

    /// read most recent temperature and relative humidity from auto mode
    pub async fn auto_read_async(&mut self, target: AutoReadTarget) -> Result<RawDatum, Error<E>> {
        let cmd_bytes = match target {
            AutoReadTarget::LastTempAndRelHumid => Command::AutoReadTempAndRelHumid,
            AutoReadTarget::MinTemp => Command::AutoReadMinTemp,
            AutoReadTarget::MaxTemp => Command::AutoReadMaxTemp,
            AutoReadTarget::MinRelHumid => Command::AutoReadMinRelHumid,
            AutoReadTarget::MaxRelHumid => Command::AutoReadMaxRelHumid,
        }.as_be_bytes();

        let mut read_buf = [0u16; 2];
        let read_buf_slice = match target {
            AutoReadTarget::LastTempAndRelHumid => &mut read_buf[..2],
            AutoReadTarget::MinTemp => &mut read_buf[..1],
            AutoReadTarget::MaxTemp => &mut read_buf[..1],
            AutoReadTarget::MinRelHumid => &mut read_buf[..1],
            AutoReadTarget::MaxRelHumid => &mut read_buf[..1],
        };

        self.cmd_delay_read_async(&cmd_bytes, None, read_buf_slice).await?;

        Ok(match target {
            AutoReadTarget::LastTempAndRelHumid => RawDatum::TempAndRelHumid(RawTempAndRelHumid {
                temperature: read_buf[0],
                humidity: read_buf[1],
            }),
            AutoReadTarget::MinTemp => RawDatum::MinTemp(read_buf[0]),
            AutoReadTarget::MaxTemp => RawDatum::MaxTemp(read_buf[0]),
            AutoReadTarget::MinRelHumid => RawDatum::MinRelHumid(read_buf[0]),
            AutoReadTarget::MaxRelHumid => RawDatum::MaxRelHumid(read_buf[0]),
        })
    }

    /// Condensation heater
    pub async fn heater_async(&mut self, heater_level: HeaterLevel) -> Result<(), Error<E>> {
        self.cmd_delay_read_async(&Command::HeaterDisable.as_be_bytes(), None, &mut [0u16; 0]).await?;

        if let Some(setting) = heater_level.setting() {
            let mut cmd_bytes = [0u8; 4];
            cmd_bytes[0..2].copy_from_slice(&Command::HeaterConfig.as_be_bytes());
            cmd_bytes[2..4].copy_from_slice(&setting.to_be_bytes());
            if let Err(i2c_err) = self.i2c.write(self.i2c_addr.as_u8(), &cmd_bytes).await {
                return Err(Error::I2c(i2c_err));
            }
            self.cmd_delay_read_async(&Command::HeaterEnable.as_be_bytes(), None, &mut [0u16; 0]).await?;
        }
        Ok(())
    }

    /// Read and optionally clear status bits
    pub async fn read_status_async(&mut self, clear: bool) -> Result<StatusBits, Error<E>> {
        let mut read_buf = [0u16; 1];
        self.cmd_delay_read_async(&Command::StatusRead.as_be_bytes(), None, &mut read_buf).await?;
        if clear {
            self.cmd_delay_read_async(&Command::StatusClear.as_be_bytes(), None, &mut [0u16; 0]).await?;
        }

        Ok(StatusBits::from(read_buf[0]))
    }

    /// Read the NIST-tracable serial number
    pub async fn read_serial_number_async(&mut self) -> Result<SerialNumber, Error<E>> {
        let mut temp_u16 = [0u16; 1];
        let mut bytes= [0u8; 6];
        self.cmd_delay_read_async(&Command::SerialID54.as_be_bytes(), None, &mut temp_u16).await?;
        bytes[5] = (temp_u16[0] >> 8) as u8;
        bytes[4] = temp_u16[0] as u8;
        self.cmd_delay_read_async(&Command::SerialID32.as_be_bytes(), None, &mut temp_u16).await?;
        bytes[3] = (temp_u16[0] >> 8) as u8;
        bytes[2] = temp_u16[0] as u8;
        self.cmd_delay_read_async(&Command::SerialID10.as_be_bytes(), None, &mut temp_u16).await?;
        bytes[1] = (temp_u16[0] >> 8) as u8;
        bytes[0] = temp_u16[0] as u8;
        Ok(SerialNumber(bytes))
    }

    /// Read the NIST-tracable manufacturer ID
    pub async fn read_manufacturer_id_async(&mut self) -> Result<ManufacturerId, Error<E>> {
        let mut read_buf = [0u16; 1];
        self.cmd_delay_read_async(&Command::ManufacturerID.as_be_bytes(), None, &mut read_buf).await?;
        Ok(ManufacturerId::from(read_buf[0]))
    }

    /// software reset
    pub async fn software_reset_async(&mut self) -> Result<(), Error<E>> {
        self.cmd_delay_read_async(&Command::SoftReset.as_be_bytes(), None, &mut [0u16; 0]).await?;
        Ok(())
    }

    // TODO: Support Alerting
    // Command::WriteSetLowAlert,
    // Command::WriteSetHighAlert,
    // Command::WriteClearLowAlert,
    // Command::WriteClearHighAlert,
    // Command::AlertToNV,

    // Command::ReadSetLowAlert,
    // Command::ReadSetHighAlert,
    // Command::ReadClearLowAlert,
    // Command::ReadClearHighAlert,

    // TODO: Support non-volatile offset
    // Command::NVOffset,

    // TODO: Support reset state
    // Command::ResetState,
}
