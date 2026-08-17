use crate::hw_def::*;

use core::fmt;

#[cfg(feature = "bincode")]
use bincode::{Decode, Encode};
#[cfg(feature = "defmt")]
use defmt::Format;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// HDC302x(-Q1) device driver
#[cfg_attr(feature = "bincode", derive(Encode, Decode))]
#[cfg_attr(feature = "defmt", derive(Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Hdc302x<I2C, Delay> {
    pub(crate) i2c: I2C,
    pub(crate) delay: Delay,
    pub(crate) i2c_addr: crate::hw_def::I2cAddr,
}

/// All possible errors in this crate
#[cfg_attr(feature = "bincode", derive(Encode, Decode))]
#[cfg_attr(feature = "defmt", derive(Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub enum Error<E> {
    /// I²C communication error
    I2c(E),
    /// Invalid input data provided, such as `SampleRate::OneShot` to `auto_start*`.
    InvalidInputData,
    /// Failure of a checksum from the device was detected
    #[cfg(feature = "crc")]
    CrcMismatch,
}

/// Raw temperature and/or humidity data from the device, represented as `u16`.
///
/// Extrema variants are snapshots of the current automatic-mode interval;
/// reading them does not clear their history. On the HDC302x devices tested by
/// the maintainer, including the instrumented HDC3022 RevC, `auto_stop*`
/// clears extrema while the reset-status bit remains clear. TI documentation
/// describes extrema as reset only by reset. This crate therefore treats every
/// automatic-mode run as a fresh extrema interval; this is an observed
/// operational contract, not a promise for untested future revisions.
#[cfg_attr(feature = "bincode", derive(Encode, Decode))]
#[cfg_attr(feature = "defmt", derive(Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum RawDatum {
    /// Temperature and relative humidity from one-shot or automatic mode.
    TempAndRelHumid(RawTempAndRelHumid),
    /// Minimum temperature in the current automatic-mode interval.
    MinTemp(u16),
    /// Maximum temperature in the current automatic-mode interval.
    MaxTemp(u16),
    /// Minimum relative humidity in the current automatic-mode interval.
    MinRelHumid(u16),
    /// Maximum relative humidity in the current automatic-mode interval.
    MaxRelHumid(u16),
}
impl RawDatum {
    /// Get temperature in Fahrenheit
    pub fn fahrenheit(&self) -> Option<f32> {
        match self {
            Self::TempAndRelHumid(RawTempAndRelHumid { temperature, .. }) => {
                Some(raw_temp_to_fahrenheit(*temperature))
            }
            Self::MinTemp(u16) => Some(raw_temp_to_fahrenheit(*u16)),
            Self::MaxTemp(u16) => Some(raw_temp_to_fahrenheit(*u16)),
            Self::MinRelHumid(_) => None,
            Self::MaxRelHumid(_) => None,
        }
    }
    /// Get temperature in Centigrade
    pub fn centigrade(&self) -> Option<f32> {
        match self {
            Self::TempAndRelHumid(RawTempAndRelHumid { temperature, .. }) => {
                Some(raw_temp_to_centigrade(*temperature))
            }
            Self::MinTemp(u16) => Some(raw_temp_to_centigrade(*u16)),
            Self::MaxTemp(u16) => Some(raw_temp_to_centigrade(*u16)),
            Self::MinRelHumid(_) => None,
            Self::MaxRelHumid(_) => None,
        }
    }
    /// Get relative humidity in percent
    pub fn humidity_percent(&self) -> Option<f32> {
        match self {
            Self::TempAndRelHumid(RawTempAndRelHumid { humidity, .. }) => {
                Some(raw_rel_humid_to_percent(*humidity))
            }
            Self::MinTemp(_) => None,
            Self::MaxTemp(_) => None,
            Self::MinRelHumid(u16) => Some(raw_rel_humid_to_percent(*u16)),
            Self::MaxRelHumid(u16) => Some(raw_rel_humid_to_percent(*u16)),
        }
    }
}

/// Raw (still in u16 format) temperature and relative humidity from the device
#[cfg_attr(feature = "bincode", derive(Encode, Decode))]
#[cfg_attr(feature = "defmt", derive(Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawTempAndRelHumid {
    /// Unprocessed temperature.
    pub temperature: u16,
    /// Unprocessed relative humidity.
    pub humidity: u16,
}
impl RawTempAndRelHumid {
    /// Get temperature in Fahrenheit
    pub fn fahrenheit(&self) -> f32 {
        raw_temp_to_fahrenheit(self.temperature)
    }
    /// Get temperature in Centigrade
    pub fn centigrade(&self) -> f32 {
        raw_temp_to_centigrade(self.temperature)
    }
    /// Get relative humidity in percent
    pub fn humidity_percent(&self) -> f32 {
        raw_rel_humid_to_percent(self.humidity)
    }
}

/// Temperature and/or humidity from the device after conversion.
#[cfg_attr(feature = "bincode", derive(Encode, Decode))]
#[cfg_attr(feature = "defmt", derive(Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum Datum {
    /// Temperature and relative humidity from one-shot or automatic mode.
    TempAndRelHumid(TempAndRelHumid),
    /// Minimum temperature in the current automatic-mode interval.
    MinTemp(Temp),
    /// Maximum temperature in the current automatic-mode interval.
    MaxTemp(Temp),
    /// Minimum relative humidity in the current automatic-mode interval.
    MinRelHumid(f32),
    /// Maximum relative humidity in the current automatic-mode interval.
    MaxRelHumid(f32),
}
impl From<&RawDatum> for Datum {
    fn from(raw: &RawDatum) -> Self {
        match raw {
            RawDatum::TempAndRelHumid(raw) => Datum::TempAndRelHumid(raw.into()),
            RawDatum::MinTemp(raw) => Datum::MinTemp((*raw).into()),
            RawDatum::MaxTemp(raw) => Datum::MaxTemp((*raw).into()),
            RawDatum::MinRelHumid(raw) => Datum::MinRelHumid(raw_rel_humid_to_percent(*raw)),
            RawDatum::MaxRelHumid(raw) => Datum::MaxRelHumid(raw_rel_humid_to_percent(*raw)),
        }
    }
}

/// Temperature and relative humidity from the device after conversion.
#[cfg_attr(feature = "bincode", derive(Encode, Decode))]
#[cfg_attr(feature = "defmt", derive(Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TempAndRelHumid {
    /// degrees centigrade
    pub centigrade: f32,
    /// degrees fahrenheit
    pub fahrenheit: f32,
    /// relative humidity in percent
    pub humidity_percent: f32,
}
impl From<&RawTempAndRelHumid> for TempAndRelHumid {
    fn from(raw: &RawTempAndRelHumid) -> Self {
        Self {
            centigrade: raw_temp_to_centigrade(raw.temperature),
            fahrenheit: raw_temp_to_fahrenheit(raw.temperature),
            humidity_percent: raw_rel_humid_to_percent(raw.humidity),
        }
    }
}
/// Temperature after conversion.
#[cfg_attr(feature = "bincode", derive(Encode, Decode))]
#[cfg_attr(feature = "defmt", derive(Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Temp {
    /// degrees centigrade
    pub centigrade: f32,
    /// degrees fahrenheit
    pub fahrenheit: f32,
}
impl From<u16> for Temp {
    fn from(raw: u16) -> Self {
        Self {
            centigrade: raw_temp_to_centigrade(raw),
            fahrenheit: raw_temp_to_fahrenheit(raw),
        }
    }
}

/// Status bits from the device
#[cfg_attr(feature = "bincode", derive(Encode, Decode))]
#[cfg_attr(feature = "defmt", derive(Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatusBits {
    raw: u16,
    /// at least one alert is active
    pub at_least_one_alert: bool,
    /// heater is enabled
    pub heater_enabled: bool,
    /// relative humidity tracking alert
    pub rh_tracking_alert: bool,
    /// temperature tracking alert
    pub t_tracking_alert: bool,
    /// relative humidity high tracking alert
    pub rh_high_tracking_alert: bool,
    /// relative humidity low tracking alert
    pub rh_low_tracking_alert: bool,
    /// temperature high tracking alert
    pub t_high_tracking_alert: bool,
    /// temperature low tracking alert
    pub t_low_tracking_alert: bool,
    /// reset (power-on or software) detected since last clear of status register
    pub reset_since_clear: bool,
    /// Device-reported command-payload checksum failure.
    ///
    /// On the tested HDC3022, an intentionally invalid heater-configuration
    /// CRC caused a data NACK and set this bit. This is distinct from
    /// [`Error::CrcMismatch`], which reports a bad CRC in data read by the
    /// driver.
    pub checksum_failure: bool,
}
impl From<u16> for StatusBits {
    fn from(raw: u16) -> Self {
        Self {
            raw,
            at_least_one_alert: (raw >> STATUS_FIELD_LSBIT_AT_LEAST_ONE_ALERT)
                & ((1 << STATUS_FIELD_WIDTH_AT_LEAST_ONE_ALERT) - 1)
                != 0,
            heater_enabled: (raw >> STATUS_FIELD_LSBIT_HEATER_ENABLED)
                & ((1 << STATUS_FIELD_WIDTH_HEATER_ENABLED) - 1)
                != 0,
            rh_tracking_alert: (raw >> STATUS_FIELD_LSBIT_RH_TRACKING_ALERT)
                & ((1 << STATUS_FIELD_WIDTH_RH_TRACKING_ALERT) - 1)
                != 0,
            t_tracking_alert: (raw >> STATUS_FIELD_LSBIT_T_TRACKING_ALERT)
                & ((1 << STATUS_FIELD_WIDTH_T_TRACKING_ALERT) - 1)
                != 0,
            rh_high_tracking_alert: (raw >> STATUS_FIELD_LSBIT_RH_HIGH_TRACKING_ALERT)
                & ((1 << STATUS_FIELD_WIDTH_RH_HIGH_TRACKING_ALERT) - 1)
                != 0,
            rh_low_tracking_alert: (raw >> STATUS_FIELD_LSBIT_RH_LOW_TRACKING_ALERT)
                & ((1 << STATUS_FIELD_WIDTH_RH_LOW_TRACKING_ALERT) - 1)
                != 0,
            t_high_tracking_alert: (raw >> STATUS_FIELD_LSBIT_T_HIGH_TRACKING_ALERT)
                & ((1 << STATUS_FIELD_WIDTH_T_HIGH_TRACKING_ALERT) - 1)
                != 0,
            t_low_tracking_alert: (raw >> STATUS_FIELD_LSBIT_T_LOW_TRACKING_ALERT)
                & ((1 << STATUS_FIELD_WIDTH_T_LOW_TRACKING_ALERT) - 1)
                != 0,
            reset_since_clear: (raw >> STATUS_FIELD_LSBIT_RESET_SINCE_CLEAR)
                & ((1 << STATUS_FIELD_WIDTH_RESET_SINCE_CLEAR) - 1)
                != 0,
            checksum_failure: (raw >> STATUS_FIELD_LSBIT_CHECKSUM_FAILURE)
                & ((1 << STATUS_FIELD_WIDTH_CHECKSUM_FAILURE) - 1)
                != 0,
        }
    }
}
impl StatusBits {
    /// Get the raw status bits
    pub fn raw(&self) -> u16 {
        self.raw
    }
}
impl fmt::Display for StatusBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StatusBits {{ 0x{:02x}; ", self.raw)?;
        if self.at_least_one_alert {
            write!(f, "at_least_one_alert ")?;
        }
        if self.heater_enabled {
            write!(f, "heater_enabled ")?;
        }
        if self.rh_tracking_alert {
            write!(f, "rh_tracking_alert ")?;
        }
        if self.t_tracking_alert {
            write!(f, "t_tracking_alert ")?;
        }
        if self.rh_high_tracking_alert {
            write!(f, "rh_high_tracking_alert ")?;
        }
        if self.rh_low_tracking_alert {
            write!(f, "rh_low_tracking_alert ")?;
        }
        if self.t_high_tracking_alert {
            write!(f, "t_high_tracking_alert ")?;
        }
        if self.t_low_tracking_alert {
            write!(f, "t_low_tracking_alert ")?;
        }
        if self.reset_since_clear {
            write!(f, "reset_since_clear ")?;
        }
        if self.checksum_failure {
            write!(f, "checksum_failure ")?;
        }
        write!(f, "}}")
    }
}

/// NIST-traceable serial number of the device.
///
/// The public array is ordered `[NIST ID0, ID1, ID2, ID3, ID4, ID5]`, from
/// least- to most-significant byte. Its [`fmt::Display`] implementation renders
/// the canonical NIST order, ID5 through ID0.
#[cfg_attr(feature = "bincode", derive(Encode, Decode))]
#[cfg_attr(feature = "defmt", derive(Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SerialNumber(pub [u8; 6]);
impl fmt::Display for SerialNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.iter().rev() {
            write!(f, "{:02X}", byte)?;
        }
        Ok(())
    }
}
impl core::ops::Deref for SerialNumber {
    type Target = [u8; 6];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl AsRef<[u8; 6]> for SerialNumber {
    fn as_ref(&self) -> &[u8; 6] {
        &self.0
    }
}

/// Manufacturer ID of the device
#[cfg_attr(feature = "bincode", derive(Encode, Decode))]
#[cfg_attr(feature = "defmt", derive(Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ManufacturerId {
    /// Texas Instruments
    #[default]
    TexasInstruments,
    /// Other
    Other(u16),
}
impl From<u16> for ManufacturerId {
    fn from(raw: u16) -> Self {
        match raw {
            MANUFACTURER_ID_TEXAS_INSTRUMENTS => ManufacturerId::TexasInstruments,
            _ => ManufacturerId::Other(raw),
        }
    }
}
impl From<ManufacturerId> for u16 {
    fn from(manuf_id: ManufacturerId) -> u16 {
        match manuf_id {
            ManufacturerId::TexasInstruments => MANUFACTURER_ID_TEXAS_INSTRUMENTS,
            ManufacturerId::Other(id) => id,
        }
    }
}
impl fmt::Display for ManufacturerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManufacturerId::TexasInstruments => {
                let mid_u16: u16 = self.clone().into();
                write!(f, "Texas Instruments (0x{mid_u16:04X})")
            }
            ManufacturerId::Other(mid_u16) => write!(f, "Unknown (0x{mid_u16:04X})"),
        }
    }
}
