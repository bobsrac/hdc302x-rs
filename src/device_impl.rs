use crate::hw_def::*;
use crate::types::*;

use cfg_if::cfg_if;

#[cfg(feature = "crc")]
use crc::{Crc, CRC_8_NRSC_5};

#[cfg(feature = "defmt")]
use defmt::trace;
#[cfg(all(feature = "defmt", feature = "crc"))]
use defmt::warn;
#[cfg(all(feature = "log", not(feature = "defmt")))]
use log::trace;
#[cfg(all(feature = "log", feature = "crc", not(feature = "defmt")))]
use log::warn;
#[cfg(not(any(feature = "defmt", feature = "log")))]
macro_rules! trace {
    ($($arg:tt)*) => {};
}
#[cfg(all(feature = "crc", not(any(feature = "defmt", feature = "log"))))]
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

    /// Consume the driver and return the resources used to create it.
    ///
    /// This returns the I²C transport, delay provider, and address supplied to
    /// [`Hdc302x::new`]. It does not recover the transport; after a completed
    /// error, apply any board- or transport-specific recovery policy before
    /// constructing another driver.
    pub fn into_parts(self) -> (I2C, Delay, I2cAddr) {
        (self.i2c, self.delay, self.i2c_addr)
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

    /// Enter automatic mode (continuous self-timed sampling).
    ///
    /// [`SampleRate::OneShot`] is not an automatic rate and returns
    /// [`Error::InvalidInputData`] without I²C traffic.
    pub fn auto_start(&mut self, sample_rate: SampleRate, low_power_mode: LowPowerMode) -> Result<(), Error<E>> {
        if sample_rate == SampleRate::OneShot {
            return Err(Error::InvalidInputData);
        }
        let cmd_bytes = start_sampling_command(sample_rate, low_power_mode).to_be_bytes();
        self.cmd_delay_read(&cmd_bytes, None, &mut [0u16; 0])?;
        Ok(())
    }

    /// Exit automatic mode and return to sleep.
    ///
    /// On the HDC302x devices tested by the maintainer, including the
    /// instrumented HDC3022 RevC, this clears extrema while the reset-status
    /// bit remains clear. TI documentation describes extrema as reset only by
    /// reset. This crate therefore treats every automatic-mode run as a fresh
    /// extrema interval; this is an observed operational contract, not a
    /// promise for untested future revisions.
    pub fn auto_stop(&mut self) -> Result<(), Error<E>> {
        self.cmd_delay_read(&Command::AutoExit.as_be_bytes(), None, &mut [0u16; 0])?;
        Ok(())
    }

    /// Read an automatic-mode result or extrema value.
    ///
    /// A latest temperature-and-humidity result is consumed by a successful
    /// read. If no fresh result is available before the first conversion or
    /// after it has been consumed, the device may NACK and this method returns
    /// [`Error::I2c`].
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

    /// Configure the condensation heater.
    ///
    /// The [`HeaterLevel`] names select TI-defined configuration settings.
    /// Hardware testing validates the command frame and status transitions, not
    /// physical heater output. Measurements while the heater is active are not
    /// ambient measurements; choose a cooldown interval for the application and
    /// board layout.
    pub fn heater(&mut self, heater_level: HeaterLevel) -> Result<(), Error<E>> {
        self.cmd_delay_read(&Command::HeaterDisable.as_be_bytes(), None, &mut [0u16; 0])?;

        if let Some(setting) = heater_level.setting() {
            let setting_bytes = setting.to_be_bytes();
            let mut cmd_bytes = [0u8; 5];
            cmd_bytes[0..2].copy_from_slice(&Command::HeaterConfig.as_be_bytes());
            cmd_bytes[2..4].copy_from_slice(&setting_bytes);
            cmd_bytes[4] = command_payload_crc(setting_bytes);
            if let Err(i2c_err) = self.i2c.write(self.i2c_addr.as_u8(), &cmd_bytes) {
                return Err(Error::I2c(i2c_err));
            }
            self.cmd_delay_read(&Command::HeaterEnable.as_be_bytes(), None, &mut [0u16; 0])?;
        }
        Ok(())
    }

    /// Read status bits and optionally clear clearable status.
    ///
    /// If `clear` is `true`, this returns the status read before clearing and
    /// then sends the status-clear command. On the instrumented HDC3022 RevC,
    /// the command clears reset and tracking status but not checksum-failure
    /// status.
    pub fn read_status(&mut self, clear: bool) -> Result<StatusBits, Error<E>> {
        let mut read_buf = [0u16; 1];
        self.cmd_delay_read(&Command::StatusRead.as_be_bytes(), None, &mut read_buf)?;
        if clear {
            self.cmd_delay_read(&Command::StatusClear.as_be_bytes(), None, &mut [0u16; 0])?;
        }

        Ok(StatusBits::from(read_buf[0]))
    }

    /// Read the NIST-traceable serial number.
    ///
    /// See [`SerialNumber`] for its byte and display ordering.
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

    /// Read the manufacturer ID.
    pub fn read_manufacturer_id(&mut self) -> Result<ManufacturerId, Error<E>> {
        let mut read_buf = [0u16; 1];
        self.cmd_delay_read(&Command::ManufacturerID.as_be_bytes(), None, &mut read_buf)?;
        Ok(ManufacturerId::from(read_buf[0]))
    }

    /// Perform a software reset.
    ///
    /// This sends the command only. Wait the TI-specified reset-ready interval
    /// before issuing a subsequent command.
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

#[cfg(test)]
fn sampling_command_cases() -> [(SampleRate, LowPowerMode, [u8; 2]); 24] {
    [
        (SampleRate::OneShot, LowPowerMode::LPM0, [0x24, 0x00]),
        (SampleRate::OneShot, LowPowerMode::LPM1, [0x24, 0x0b]),
        (SampleRate::OneShot, LowPowerMode::LPM2, [0x24, 0x16]),
        (SampleRate::OneShot, LowPowerMode::LPM3, [0x24, 0xff]),
        (SampleRate::Auto500mHz, LowPowerMode::LPM0, [0x20, 0x32]),
        (SampleRate::Auto500mHz, LowPowerMode::LPM1, [0x20, 0x24]),
        (SampleRate::Auto500mHz, LowPowerMode::LPM2, [0x20, 0x2f]),
        (SampleRate::Auto500mHz, LowPowerMode::LPM3, [0x20, 0xff]),
        (SampleRate::Auto1Hz, LowPowerMode::LPM0, [0x21, 0x30]),
        (SampleRate::Auto1Hz, LowPowerMode::LPM1, [0x21, 0x26]),
        (SampleRate::Auto1Hz, LowPowerMode::LPM2, [0x21, 0x2d]),
        (SampleRate::Auto1Hz, LowPowerMode::LPM3, [0x21, 0xff]),
        (SampleRate::Auto2Hz, LowPowerMode::LPM0, [0x22, 0x36]),
        (SampleRate::Auto2Hz, LowPowerMode::LPM1, [0x22, 0x20]),
        (SampleRate::Auto2Hz, LowPowerMode::LPM2, [0x22, 0x2b]),
        (SampleRate::Auto2Hz, LowPowerMode::LPM3, [0x22, 0xff]),
        (SampleRate::Auto4Hz, LowPowerMode::LPM0, [0x23, 0x34]),
        (SampleRate::Auto4Hz, LowPowerMode::LPM1, [0x23, 0x22]),
        (SampleRate::Auto4Hz, LowPowerMode::LPM2, [0x23, 0x29]),
        (SampleRate::Auto4Hz, LowPowerMode::LPM3, [0x23, 0xff]),
        (SampleRate::Auto10Hz, LowPowerMode::LPM0, [0x27, 0x37]),
        (SampleRate::Auto10Hz, LowPowerMode::LPM1, [0x27, 0x21]),
        (SampleRate::Auto10Hz, LowPowerMode::LPM2, [0x27, 0x2a]),
        (SampleRate::Auto10Hz, LowPowerMode::LPM3, [0x27, 0xff]),
    ]
}

#[cfg(test)]
mod ownership_tests {
    use super::*;

    #[test]
    fn into_parts_returns_each_constructor_resource() {
        for (i2c, delay, address) in [
            (1_u8, 10_u16, I2cAddr::Addr00),
            (2_u8, 20_u16, I2cAddr::Addr01),
            (3_u8, 30_u16, I2cAddr::Addr10),
            (4_u8, 40_u16, I2cAddr::Addr11),
        ] {
            let expected_address = address.clone();
            let hdc = Hdc302x::new(i2c, delay, address);

            let (returned_i2c, returned_delay, returned_address) = hdc.into_parts();

            assert_eq!(returned_i2c, i2c);
            assert_eq!(returned_delay, delay);
            assert_eq!(returned_address, expected_address);
        }
    }
}

#[cfg(all(test, feature = "blocking"))]
mod blocking_scripted_i2c_tests {
    use std::vec;
    use std::vec::Vec;

    use super::*;
    use embedded_hal::i2c::{ErrorKind, ErrorType, I2c, Operation};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ScriptError {
        UnexpectedRead,
        DataNack,
    }

    impl embedded_hal::i2c::Error for ScriptError {
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    #[derive(Default)]
    struct RecordingI2c {
        writes: Vec<(u8, Vec<u8>)>,
        nack_reads: bool,
        read_frames: Vec<Vec<u8>>,
        fail_on_write: Option<usize>,
    }

    impl ErrorType for RecordingI2c {
        type Error = ScriptError;
    }

    impl I2c for RecordingI2c {
        fn transaction(
            &mut self,
            address: u8,
            operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            for operation in operations {
                match operation {
                    Operation::Write(bytes) => {
                        self.writes.push((address, bytes.to_vec()));
                        if self.fail_on_write == Some(self.writes.len()) {
                            return Err(ScriptError::DataNack);
                        }
                    }
                    Operation::Read(buffer) => {
                        if self.nack_reads {
                            return Err(ScriptError::DataNack);
                        }
                        let Some(frame) = self.read_frames.first() else {
                            return Err(ScriptError::UnexpectedRead);
                        };
                        if frame.len() != buffer.len() {
                            return Err(ScriptError::UnexpectedRead);
                        }
                        buffer.copy_from_slice(frame);
                        self.read_frames.remove(0);
                    }
                }
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct ImmediateDelay;

    impl embedded_hal::delay::DelayNs for ImmediateDelay {
        fn delay_ns(&mut self, _ns: u32) {}
    }

    fn crc_word(word: u16) -> Vec<u8> {
        let bytes = word.to_be_bytes();
        vec![bytes[0], bytes[1], command_payload_crc(bytes)]
    }

    #[test]
    fn every_sampling_command_uses_its_documented_bytes() {
        for (sample_rate, low_power_mode, command) in sampling_command_cases() {
            let read_frames = if sample_rate == SampleRate::OneShot {
                let mut read_frame = crc_word(0x1234);
                read_frame.extend(crc_word(0x5678));
                vec![read_frame]
            } else {
                vec![]
            };
            let mut hdc = Hdc302x::new(
                RecordingI2c {
                    read_frames,
                    ..RecordingI2c::default()
                },
                ImmediateDelay,
                I2cAddr::Addr00,
            );

            if sample_rate == SampleRate::OneShot {
                assert_eq!(
                    hdc.one_shot(low_power_mode).unwrap(),
                    RawDatum::TempAndRelHumid(RawTempAndRelHumid {
                        temperature: 0x1234,
                        humidity: 0x5678,
                    })
                );
            } else {
                hdc.auto_start(sample_rate, low_power_mode).unwrap();
            }

            assert_eq!(hdc.i2c.writes, vec![(0x44, command.to_vec())]);
            assert!(hdc.i2c.read_frames.is_empty());
        }
    }

    #[test]
    fn every_automatic_read_target_uses_its_documented_command_and_width() {
        for (target, command, expected) in [
            (
                AutoReadTarget::LastTempAndRelHumid,
                [0xe0, 0x00],
                RawDatum::TempAndRelHumid(RawTempAndRelHumid {
                    temperature: 0x1234,
                    humidity: 0x5678,
                }),
            ),
            (AutoReadTarget::MinTemp, [0xe0, 0x02], RawDatum::MinTemp(0x1234)),
            (AutoReadTarget::MaxTemp, [0xe0, 0x03], RawDatum::MaxTemp(0x1234)),
            (
                AutoReadTarget::MinRelHumid,
                [0xe0, 0x04],
                RawDatum::MinRelHumid(0x1234),
            ),
            (
                AutoReadTarget::MaxRelHumid,
                [0xe0, 0x05],
                RawDatum::MaxRelHumid(0x1234),
            ),
        ] {
            let mut frame = crc_word(0x1234);
            if target == AutoReadTarget::LastTempAndRelHumid {
                frame.extend(crc_word(0x5678));
            }
            let mut hdc = Hdc302x::new(
                RecordingI2c {
                    read_frames: vec![frame],
                    ..RecordingI2c::default()
                },
                ImmediateDelay,
                I2cAddr::Addr00,
            );

            assert_eq!(hdc.auto_read(target).unwrap(), expected);
            assert_eq!(hdc.i2c.writes, vec![(0x44, command.to_vec())]);
            assert!(hdc.i2c.read_frames.is_empty());
        }
    }

    #[test]
    fn status_reset_identity_and_auto_stop_use_the_documented_sequence() {
        let mut hdc = Hdc302x::new(
            RecordingI2c {
                read_frames: vec![
                    crc_word(0x8010),
                    crc_word(0x859d),
                    crc_word(0xa573),
                    crc_word(0x0c3e),
                    crc_word(0x3000),
                ],
                ..RecordingI2c::default()
            },
            ImmediateDelay,
            I2cAddr::Addr00,
        );

        assert_eq!(hdc.read_status(true).unwrap().raw(), 0x8010);
        hdc.software_reset().unwrap();
        let serial = hdc.read_serial_number().unwrap();
        assert_eq!(serial.as_ref(), &[0x3e, 0x0c, 0x73, 0xa5, 0x9d, 0x85]);
        assert_eq!(std::format!("{serial}"), "859DA5730C3E");
        assert_eq!(
            hdc.read_manufacturer_id().unwrap(),
            ManufacturerId::TexasInstruments
        );
        hdc.auto_stop().unwrap();

        assert_eq!(
            hdc.i2c.writes,
            vec![
                (0x44, vec![0xf3, 0x2d]),
                (0x44, vec![0x30, 0x41]),
                (0x44, vec![0x30, 0xa2]),
                (0x44, vec![0x36, 0x83]),
                (0x44, vec![0x36, 0x84]),
                (0x44, vec![0x36, 0x85]),
                (0x44, vec![0x37, 0x81]),
                (0x44, vec![0x30, 0x93]),
            ]
        );
        assert!(hdc.i2c.read_frames.is_empty());
    }

    #[test]
    fn every_heater_level_writes_the_documented_command_sequence() {
        for (heater_level, expected_writes) in [
            (HeaterLevel::Off, vec![(0x44, vec![0x30, 0x66])]),
            (
                HeaterLevel::On25Percent,
                vec![
                    (0x44, vec![0x30, 0x66]),
                    (0x44, vec![0x30, 0x6e, 0x00, 0x9f, 0x96]),
                    (0x44, vec![0x30, 0x6d]),
                ],
            ),
            (
                HeaterLevel::On50Percent,
                vec![
                    (0x44, vec![0x30, 0x66]),
                    (0x44, vec![0x30, 0x6e, 0x03, 0xff, 0x00]),
                    (0x44, vec![0x30, 0x6d]),
                ],
            ),
            (
                HeaterLevel::On100Percent,
                vec![
                    (0x44, vec![0x30, 0x66]),
                    (0x44, vec![0x30, 0x6e, 0x3f, 0xff, 0x06]),
                    (0x44, vec![0x30, 0x6d]),
                ],
            ),
        ] {
            let mut hdc = Hdc302x::new(RecordingI2c::default(), ImmediateDelay, I2cAddr::Addr00);

            hdc.heater(heater_level).unwrap();

            assert_eq!(hdc.i2c.writes, expected_writes);
        }
    }

    #[test]
    fn auto_start_rejects_the_one_shot_rate_without_i2c_traffic() {
        let mut hdc = Hdc302x::new(RecordingI2c::default(), ImmediateDelay, I2cAddr::Addr00);

        let result = hdc.auto_start(SampleRate::OneShot, LowPowerMode::LPM0);

        assert!(matches!(result, Err(Error::InvalidInputData)));
        assert!(hdc.i2c.writes.is_empty());
    }

    #[test]
    fn valid_auto_start_writes_its_automatic_command() {
        let mut hdc = Hdc302x::new(RecordingI2c::default(), ImmediateDelay, I2cAddr::Addr00);

        hdc.auto_start(SampleRate::Auto1Hz, LowPowerMode::LPM3)
            .unwrap();

        assert_eq!(hdc.i2c.writes, vec![(0x44, vec![0x21, 0xff])]);
    }

    #[test]
    fn unavailable_latest_data_i2c_error_leaves_driver_consumable() {
        let mut hdc = Hdc302x::new(
            RecordingI2c {
                nack_reads: true,
                ..RecordingI2c::default()
            },
            ImmediateDelay,
            I2cAddr::Addr00,
        );

        let result = hdc.auto_read(AutoReadTarget::LastTempAndRelHumid);

        assert!(matches!(result, Err(Error::I2c(ScriptError::DataNack))));
        let (i2c, _delay, address) = hdc.into_parts();
        assert_eq!(i2c.writes, vec![(0x44, vec![0xe0, 0x00])]);
        assert_eq!(address, I2cAddr::Addr00);
    }

    #[cfg(feature = "crc")]
    #[test]
    fn crc_error_leaves_driver_consumable() {
        let mut hdc = Hdc302x::new(
            RecordingI2c {
                read_frames: vec![vec![0x12, 0x34, 0x00]],
                ..RecordingI2c::default()
            },
            ImmediateDelay,
            I2cAddr::Addr00,
        );

        assert!(matches!(
            hdc.auto_read(AutoReadTarget::MinTemp),
            Err(Error::CrcMismatch)
        ));
        let (i2c, _delay, address) = hdc.into_parts();
        assert_eq!(i2c.writes, vec![(0x44, vec![0xe0, 0x02])]);
        assert_eq!(address, I2cAddr::Addr00);
    }

    #[test]
    fn direct_heater_configuration_write_nack_is_returned_without_enabling() {
        let mut hdc = Hdc302x::new(
            RecordingI2c {
                fail_on_write: Some(2),
                ..RecordingI2c::default()
            },
            ImmediateDelay,
            I2cAddr::Addr00,
        );

        assert!(matches!(
            hdc.heater(HeaterLevel::On25Percent),
            Err(Error::I2c(ScriptError::DataNack))
        ));
        assert_eq!(
            hdc.i2c.writes,
            vec![
                (0x44, vec![0x30, 0x66]),
                (0x44, vec![0x30, 0x6e, 0x00, 0x9f, 0x96]),
            ]
        );
    }
}

#[cfg(all(test, feature = "async"))]
mod async_scripted_i2c_tests {
    use std::vec;
    use std::vec::Vec;

    use futures::executor::block_on;

    use super::*;
    use embedded_hal_async::i2c::{ErrorKind, ErrorType, I2c, Operation};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ScriptError {
        UnexpectedRead,
        DataNack,
    }

    impl embedded_hal_async::i2c::Error for ScriptError {
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    #[derive(Default)]
    struct RecordingI2c {
        writes: Vec<(u8, Vec<u8>)>,
        nack_reads: bool,
        read_frames: Vec<Vec<u8>>,
        fail_on_write: Option<usize>,
    }

    impl ErrorType for RecordingI2c {
        type Error = ScriptError;
    }

    impl I2c for RecordingI2c {
        async fn transaction(
            &mut self,
            address: u8,
            operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            for operation in operations {
                match operation {
                    Operation::Write(bytes) => {
                        self.writes.push((address, bytes.to_vec()));
                        if self.fail_on_write == Some(self.writes.len()) {
                            return Err(ScriptError::DataNack);
                        }
                    }
                    Operation::Read(buffer) => {
                        if self.nack_reads {
                            return Err(ScriptError::DataNack);
                        }
                        let Some(frame) = self.read_frames.first() else {
                            return Err(ScriptError::UnexpectedRead);
                        };
                        if frame.len() != buffer.len() {
                            return Err(ScriptError::UnexpectedRead);
                        }
                        buffer.copy_from_slice(frame);
                        self.read_frames.remove(0);
                    }
                }
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct ImmediateDelay;

    impl embedded_hal_async::delay::DelayNs for ImmediateDelay {
        async fn delay_ns(&mut self, _ns: u32) {}
    }

    fn crc_word(word: u16) -> Vec<u8> {
        let bytes = word.to_be_bytes();
        vec![bytes[0], bytes[1], command_payload_crc(bytes)]
    }

    #[test]
    fn every_sampling_command_uses_its_documented_bytes() {
        for (sample_rate, low_power_mode, command) in sampling_command_cases() {
            let read_frames = if sample_rate == SampleRate::OneShot {
                let mut read_frame = crc_word(0x1234);
                read_frame.extend(crc_word(0x5678));
                vec![read_frame]
            } else {
                vec![]
            };
            let mut hdc = Hdc302x::new(
                RecordingI2c {
                    read_frames,
                    ..RecordingI2c::default()
                },
                ImmediateDelay,
                I2cAddr::Addr00,
            );

            if sample_rate == SampleRate::OneShot {
                assert_eq!(
                    block_on(hdc.one_shot_async(low_power_mode)).unwrap(),
                    RawDatum::TempAndRelHumid(RawTempAndRelHumid {
                        temperature: 0x1234,
                        humidity: 0x5678,
                    })
                );
            } else {
                block_on(hdc.auto_start_async(sample_rate, low_power_mode)).unwrap();
            }

            assert_eq!(hdc.i2c.writes, vec![(0x44, command.to_vec())]);
            assert!(hdc.i2c.read_frames.is_empty());
        }
    }

    #[test]
    fn every_automatic_read_target_uses_its_documented_command_and_width() {
        for (target, command, expected) in [
            (
                AutoReadTarget::LastTempAndRelHumid,
                [0xe0, 0x00],
                RawDatum::TempAndRelHumid(RawTempAndRelHumid {
                    temperature: 0x1234,
                    humidity: 0x5678,
                }),
            ),
            (AutoReadTarget::MinTemp, [0xe0, 0x02], RawDatum::MinTemp(0x1234)),
            (AutoReadTarget::MaxTemp, [0xe0, 0x03], RawDatum::MaxTemp(0x1234)),
            (
                AutoReadTarget::MinRelHumid,
                [0xe0, 0x04],
                RawDatum::MinRelHumid(0x1234),
            ),
            (
                AutoReadTarget::MaxRelHumid,
                [0xe0, 0x05],
                RawDatum::MaxRelHumid(0x1234),
            ),
        ] {
            let mut frame = crc_word(0x1234);
            if target == AutoReadTarget::LastTempAndRelHumid {
                frame.extend(crc_word(0x5678));
            }
            let mut hdc = Hdc302x::new(
                RecordingI2c {
                    read_frames: vec![frame],
                    ..RecordingI2c::default()
                },
                ImmediateDelay,
                I2cAddr::Addr00,
            );

            assert_eq!(block_on(hdc.auto_read_async(target)).unwrap(), expected);
            assert_eq!(hdc.i2c.writes, vec![(0x44, command.to_vec())]);
            assert!(hdc.i2c.read_frames.is_empty());
        }
    }

    #[test]
    fn status_reset_identity_and_auto_stop_use_the_documented_sequence() {
        let mut hdc = Hdc302x::new(
            RecordingI2c {
                read_frames: vec![
                    crc_word(0x8010),
                    crc_word(0x859d),
                    crc_word(0xa573),
                    crc_word(0x0c3e),
                    crc_word(0x3000),
                ],
                ..RecordingI2c::default()
            },
            ImmediateDelay,
            I2cAddr::Addr00,
        );

        assert_eq!(block_on(hdc.read_status_async(true)).unwrap().raw(), 0x8010);
        block_on(hdc.software_reset_async()).unwrap();
        let serial = block_on(hdc.read_serial_number_async()).unwrap();
        assert_eq!(serial.as_ref(), &[0x3e, 0x0c, 0x73, 0xa5, 0x9d, 0x85]);
        assert_eq!(std::format!("{serial}"), "859DA5730C3E");
        assert_eq!(
            block_on(hdc.read_manufacturer_id_async()).unwrap(),
            ManufacturerId::TexasInstruments
        );
        block_on(hdc.auto_stop_async()).unwrap();

        assert_eq!(
            hdc.i2c.writes,
            vec![
                (0x44, vec![0xf3, 0x2d]),
                (0x44, vec![0x30, 0x41]),
                (0x44, vec![0x30, 0xa2]),
                (0x44, vec![0x36, 0x83]),
                (0x44, vec![0x36, 0x84]),
                (0x44, vec![0x36, 0x85]),
                (0x44, vec![0x37, 0x81]),
                (0x44, vec![0x30, 0x93]),
            ]
        );
        assert!(hdc.i2c.read_frames.is_empty());
    }

    #[test]
    fn every_heater_level_writes_the_documented_command_sequence() {
        for (heater_level, expected_writes) in [
            (HeaterLevel::Off, vec![(0x44, vec![0x30, 0x66])]),
            (
                HeaterLevel::On25Percent,
                vec![
                    (0x44, vec![0x30, 0x66]),
                    (0x44, vec![0x30, 0x6e, 0x00, 0x9f, 0x96]),
                    (0x44, vec![0x30, 0x6d]),
                ],
            ),
            (
                HeaterLevel::On50Percent,
                vec![
                    (0x44, vec![0x30, 0x66]),
                    (0x44, vec![0x30, 0x6e, 0x03, 0xff, 0x00]),
                    (0x44, vec![0x30, 0x6d]),
                ],
            ),
            (
                HeaterLevel::On100Percent,
                vec![
                    (0x44, vec![0x30, 0x66]),
                    (0x44, vec![0x30, 0x6e, 0x3f, 0xff, 0x06]),
                    (0x44, vec![0x30, 0x6d]),
                ],
            ),
        ] {
            let mut hdc = Hdc302x::new(RecordingI2c::default(), ImmediateDelay, I2cAddr::Addr00);

            block_on(hdc.heater_async(heater_level)).unwrap();

            assert_eq!(hdc.i2c.writes, expected_writes);
        }
    }

    #[test]
    fn auto_start_rejects_the_one_shot_rate_without_i2c_traffic() {
        let mut hdc = Hdc302x::new(RecordingI2c::default(), ImmediateDelay, I2cAddr::Addr00);

        let result = block_on(hdc.auto_start_async(SampleRate::OneShot, LowPowerMode::LPM0));

        assert!(matches!(result, Err(Error::InvalidInputData)));
        assert!(hdc.i2c.writes.is_empty());
    }

    #[test]
    fn valid_auto_start_writes_its_automatic_command() {
        let mut hdc = Hdc302x::new(RecordingI2c::default(), ImmediateDelay, I2cAddr::Addr00);

        block_on(hdc.auto_start_async(SampleRate::Auto1Hz, LowPowerMode::LPM3)).unwrap();

        assert_eq!(hdc.i2c.writes, vec![(0x44, vec![0x21, 0xff])]);
    }

    #[test]
    fn unavailable_latest_data_i2c_error_leaves_driver_consumable() {
        let mut hdc = Hdc302x::new(
            RecordingI2c {
                nack_reads: true,
                ..RecordingI2c::default()
            },
            ImmediateDelay,
            I2cAddr::Addr00,
        );

        let result = block_on(hdc.auto_read_async(AutoReadTarget::LastTempAndRelHumid));

        assert!(matches!(result, Err(Error::I2c(ScriptError::DataNack))));
        let (i2c, _delay, address) = hdc.into_parts();
        assert_eq!(i2c.writes, vec![(0x44, vec![0xe0, 0x00])]);
        assert_eq!(address, I2cAddr::Addr00);
    }

    #[cfg(feature = "crc")]
    #[test]
    fn crc_error_leaves_driver_consumable() {
        let mut hdc = Hdc302x::new(
            RecordingI2c {
                read_frames: vec![vec![0x12, 0x34, 0x00]],
                ..RecordingI2c::default()
            },
            ImmediateDelay,
            I2cAddr::Addr00,
        );

        assert!(matches!(
            block_on(hdc.auto_read_async(AutoReadTarget::MinTemp)),
            Err(Error::CrcMismatch)
        ));
        let (i2c, _delay, address) = hdc.into_parts();
        assert_eq!(i2c.writes, vec![(0x44, vec![0xe0, 0x02])]);
        assert_eq!(address, I2cAddr::Addr00);
    }

    #[test]
    fn direct_heater_configuration_write_nack_is_returned_without_enabling() {
        let mut hdc = Hdc302x::new(
            RecordingI2c {
                fail_on_write: Some(2),
                ..RecordingI2c::default()
            },
            ImmediateDelay,
            I2cAddr::Addr00,
        );

        assert!(matches!(
            block_on(hdc.heater_async(HeaterLevel::On25Percent)),
            Err(Error::I2c(ScriptError::DataNack))
        ));
        assert_eq!(
            hdc.i2c.writes,
            vec![
                (0x44, vec![0x30, 0x66]),
                (0x44, vec![0x30, 0x6e, 0x00, 0x9f, 0x96]),
            ]
        );
    }
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

    /// Enter automatic mode (continuous self-timed sampling).
    ///
    /// [`SampleRate::OneShot`] is not an automatic rate and returns
    /// [`Error::InvalidInputData`] without I²C traffic.
    pub async fn auto_start_async(&mut self, sample_rate: SampleRate, low_power_mode: LowPowerMode) -> Result<(), Error<E>> {
        if sample_rate == SampleRate::OneShot {
            return Err(Error::InvalidInputData);
        }
        let cmd_bytes = start_sampling_command(sample_rate, low_power_mode).to_be_bytes();
        self.cmd_delay_read_async(&cmd_bytes, None, &mut [0u16; 0]).await?;
        Ok(())
    }

    /// Exit automatic mode and return to sleep.
    ///
    /// On the HDC302x devices tested by the maintainer, including the
    /// instrumented HDC3022 RevC, this clears extrema while the reset-status
    /// bit remains clear. TI documentation describes extrema as reset only by
    /// reset. This crate therefore treats every automatic-mode run as a fresh
    /// extrema interval; this is an observed operational contract, not a
    /// promise for untested future revisions.
    pub async fn auto_stop_async(&mut self) -> Result<(), Error<E>> {
        self.cmd_delay_read_async(&Command::AutoExit.as_be_bytes(), None, &mut [0u16; 0]).await?;
        Ok(())
    }

    /// Read an automatic-mode result or extrema value.
    ///
    /// A latest temperature-and-humidity result is consumed by a successful
    /// read. If no fresh result is available before the first conversion or
    /// after it has been consumed, the device may NACK and this method returns
    /// [`Error::I2c`].
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

    /// Configure the condensation heater.
    ///
    /// The [`HeaterLevel`] names select TI-defined configuration settings.
    /// Hardware testing validates the command frame and status transitions, not
    /// physical heater output. Measurements while the heater is active are not
    /// ambient measurements; choose a cooldown interval for the application and
    /// board layout.
    pub async fn heater_async(&mut self, heater_level: HeaterLevel) -> Result<(), Error<E>> {
        self.cmd_delay_read_async(&Command::HeaterDisable.as_be_bytes(), None, &mut [0u16; 0]).await?;

        if let Some(setting) = heater_level.setting() {
            let setting_bytes = setting.to_be_bytes();
            let mut cmd_bytes = [0u8; 5];
            cmd_bytes[0..2].copy_from_slice(&Command::HeaterConfig.as_be_bytes());
            cmd_bytes[2..4].copy_from_slice(&setting_bytes);
            cmd_bytes[4] = command_payload_crc(setting_bytes);
            if let Err(i2c_err) = self.i2c.write(self.i2c_addr.as_u8(), &cmd_bytes).await {
                return Err(Error::I2c(i2c_err));
            }
            self.cmd_delay_read_async(&Command::HeaterEnable.as_be_bytes(), None, &mut [0u16; 0]).await?;
        }
        Ok(())
    }

    /// Read status bits and optionally clear clearable status.
    ///
    /// If `clear` is `true`, this returns the status read before clearing and
    /// then sends the status-clear command. On the instrumented HDC3022 RevC,
    /// the command clears reset and tracking status but not checksum-failure
    /// status.
    pub async fn read_status_async(&mut self, clear: bool) -> Result<StatusBits, Error<E>> {
        let mut read_buf = [0u16; 1];
        self.cmd_delay_read_async(&Command::StatusRead.as_be_bytes(), None, &mut read_buf).await?;
        if clear {
            self.cmd_delay_read_async(&Command::StatusClear.as_be_bytes(), None, &mut [0u16; 0]).await?;
        }

        Ok(StatusBits::from(read_buf[0]))
    }

    /// Read the NIST-traceable serial number.
    ///
    /// See [`SerialNumber`] for its byte and display ordering.
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

    /// Read the manufacturer ID.
    pub async fn read_manufacturer_id_async(&mut self) -> Result<ManufacturerId, Error<E>> {
        let mut read_buf = [0u16; 1];
        self.cmd_delay_read_async(&Command::ManufacturerID.as_be_bytes(), None, &mut read_buf).await?;
        Ok(ManufacturerId::from(read_buf[0]))
    }

    /// Perform a software reset.
    ///
    /// This sends the command only. Wait the TI-specified reset-ready interval
    /// before issuing a subsequent command.
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
