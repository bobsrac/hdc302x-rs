use crate::hw_def::*;
use crate::types::*;

use crc::{Crc, CRC_8_NRSC_5};
use embedded_hal_async::{delay::DelayNs, i2c::I2c};

const CRC: crc::Crc<u8> = Crc::<u8>::new(&CRC_8_NRSC_5);

// TODO: consider adding type state pattern around the state of the device.  When we start a
// one-shot, don't do things other than read the result until that happens.  When in auto mode,
// don't do one-shot samples.  When sleeping (not in one-shot or auto mode), don't read auto mode
// results.
impl<I2C, Delay, E> Hdc302x<I2C, Delay>
where
    I2C: I2c<Error = E>,
    Delay: DelayNs,
{
    /// Create a new HDC302x driver instance
    pub fn new(i2c: I2C, delay: Delay, i2c_addr: I2cAddr) -> Self {
        Self { i2c, delay, i2c_addr }
    }

    async fn cmd_and_read(&mut self, cmd_bytes: &[u8; 2], read_vals: &mut [u16]) -> Result<(), Error<E>> {
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
            log::trace!("hdc302x::cmd_and_read(): read_buf_slice.len()={}", read_buf_slice.len());
            if let Err(_) = self.i2c.write_read(self.i2c_addr.as_u8(), cmd_bytes, read_buf_slice).await {
                // TODO: consider a timeout and/or retry limit
                while let Err(_) = self.i2c.read(self.i2c_addr.as_u8(), read_buf_slice).await {
                    self.delay.delay_ms(1).await;
                };
            };
            // TODO: consider whether to retry around this failure
            for ii in 0..num_vals {
                let read_word = &read_buf[ii*3+0..=ii*3+1];
                let read_crc = &read_buf[ii*3+2];
                let crc_expect = CRC.checksum(read_word);
                if *read_crc != crc_expect {
                    log::warn!("hdc302x::cmd_and_read(): crc mismatch word {ii}/{num_vals}: read_buf={read_buf:?}, read_word={read_word:?}, read_crc={read_crc}, crc_expect={crc_expect}");
                    return Err(Error::CrcMismatch);
                }
                read_vals[ii] = (read_word[0] as u16) << 8 | read_word[1] as u16;
            }
        }
        Ok(())
    }

    /// Trigger a one-shot measurement and return the raw sample pair
    pub async fn one_shot(&mut self, low_power_mode: LowPowerMode) -> Result<RawDatum, Error<E>> {
        let cmd_bytes = start_sampling_command(SampleRate::OneShot, low_power_mode).to_be_bytes();
        let mut read_buf = [0u16; 2];
        self.cmd_and_read(&cmd_bytes, &mut read_buf).await?;
        Ok(RawDatum::TempAndRelHumid(RawTempAndRelHumid {
            temperature: read_buf[0],
            humidity: read_buf[1],
        }))
    }

    /// Enter auto mode (continuous self-timed sampling)
    pub async fn auto_start(&mut self, sample_rate: SampleRate, low_power_mode: LowPowerMode) -> Result<(), Error<E>> {
        let cmd_bytes = start_sampling_command(sample_rate, low_power_mode).to_be_bytes();
        self.cmd_and_read(&cmd_bytes, &mut [0u16; 0]).await?;
        Ok(())
    }

    /// exit auto mode and return to sleep
    pub async fn auto_stop(&mut self) -> Result<(), Error<E>> {
        self.cmd_and_read(&Command::AutoExit.to_be_bytes(), &mut [0u16; 0]).await?;
        Ok(())
    }

    /// read most recent temperature and relative humidity from auto mode
    pub async fn auto_read(&mut self, target: AutoReadTarget) -> Result<RawDatum, Error<E>> {
        let cmd_bytes = match target {
            AutoReadTarget::LastTempAndRelHumid => Command::AutoReadTempAndRelHumid,
            AutoReadTarget::MinTemp => Command::AutoReadMinTemp,
            AutoReadTarget::MaxTemp => Command::AutoReadMaxTemp,
            AutoReadTarget::MinRelHumid => Command::AutoReadMinRelHumid,
            AutoReadTarget::MaxRelHumid => Command::AutoReadMaxRelHumid,
        }.to_be_bytes();

        let mut read_buf = [0u16; 2];
        let read_buf_slice = match target {
            AutoReadTarget::LastTempAndRelHumid => &mut read_buf[..2],
            AutoReadTarget::MinTemp => &mut read_buf[..1],
            AutoReadTarget::MaxTemp => &mut read_buf[..1],
            AutoReadTarget::MinRelHumid => &mut read_buf[..1],
            AutoReadTarget::MaxRelHumid => &mut read_buf[..1],
        };

        self.cmd_and_read(&cmd_bytes, read_buf_slice).await?;

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
    pub async fn heater(&mut self, heater_level: HeaterLevel) -> Result<(), Error<E>> {
        self.cmd_and_read(&Command::HeaterDisable.to_be_bytes(), &mut [0u16; 0]).await?;

        if let Some(setting) = heater_level.setting() {
            let mut cmd_bytes = [0u8; 4];
            cmd_bytes[0..2].copy_from_slice(&Command::HeaterConfig.to_be_bytes());
            cmd_bytes[2..4].copy_from_slice(&setting.to_be_bytes());
            if let Err(i2c_err) = self.i2c.write(self.i2c_addr.as_u8(), &cmd_bytes).await {
                return Err(Error::I2c(i2c_err));
            }
            self.cmd_and_read(&Command::HeaterEnable.to_be_bytes(), &mut [0u16; 0]).await?;
        }
        Ok(())
    }

    /// Read and optionally clear status bits
    pub async fn read_status(&mut self, clear: bool) -> Result<StatusBits, Error<E>> {
        let mut read_buf = [0u16; 1];
        self.cmd_and_read(&Command::StatusRead.to_be_bytes(), &mut read_buf).await?;
        if clear {
            self.cmd_and_read(&Command::StatusClear.to_be_bytes(), &mut [0u16; 0]).await?;
        }

        Ok(StatusBits::from(read_buf[0]))
    }

    /// Read the NIST-tracable serial number
    pub async fn read_serial_number(&mut self) -> Result<SerialNumber, Error<E>> {
        let mut temp_u16 = [0u16; 1];
        let mut bytes= [0u8; 6];
        self.cmd_and_read(&Command::SerialID54.to_be_bytes(), &mut temp_u16).await?;
        bytes[5] = (temp_u16[0] >> 8) as u8;
        bytes[4] = temp_u16[0] as u8;
        self.cmd_and_read(&Command::SerialID32.to_be_bytes(), &mut temp_u16).await?;
        bytes[3] = (temp_u16[0] >> 8) as u8;
        bytes[2] = temp_u16[0] as u8;
        self.cmd_and_read(&Command::SerialID10.to_be_bytes(), &mut temp_u16).await?;
        bytes[1] = (temp_u16[0] >> 8) as u8;
        bytes[0] = temp_u16[0] as u8;
        Ok(SerialNumber(bytes))
    }

    /// Read the NIST-tracable manufacturer ID
    pub async fn read_manufacturer_id(&mut self) -> Result<ManufacturerId, Error<E>> {
        let mut read_buf = [0u16; 1];
        self.cmd_and_read(&Command::ManufacturerID.to_be_bytes(), &mut read_buf).await?;
        Ok(ManufacturerId::from(read_buf[0]))
    }

    /// software reset
    pub async fn software_reset(&mut self) -> Result<(), Error<E>> {
        self.cmd_and_read(&Command::SoftReset.to_be_bytes(), &mut [0u16; 0]).await?;
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
