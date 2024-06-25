/// I2C device address options, which are selected via the ADDR1 and ADDR pins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum I2cAddr {
    /// ADDR1 = 0, ADDR0 = 0
    Addr00 = 0x44,
    /// ADDR1 = 0, ADDR0 = 1
    Addr01 = 0x45,
    /// ADDR1 = 1, ADDR0 = 0
    Addr10 = 0x46,
    /// ADDR1 = 1, ADDR0 = 1
    Addr11 = 0x47,
}
impl I2cAddr {
    pub(crate) fn as_u8(&self) -> u8 {
        match self {
            Self::Addr00 => 0x44,
            Self::Addr01 => 0x45,
            Self::Addr10 => 0x46,
            Self::Addr11 => 0x47,
        }
    }
}

/// Sample rate options, covering both the one-shot and auto modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SampleRate {
    /// initiate and read a single measurement, returning device back to sleep afterward
    OneShot,
    /// device self-times 1 sample every 2 seconds
    Auto500mHz,
    /// device self-times 1 sample every 1 second
    Auto1Hz,
    /// device self-times 1 sample every 0.5 seconds
    Auto2Hz,
    /// device self-times 1 sample every 0.25 seconds
    Auto4Hz,
    /// device self-times 1 sample every 0.1 seconds
    Auto10Hz,
}

/// Low power mode options, which control the trade-off between power consumption, measurement noise, and sample latency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LowPowerMode {
    /// lowest noise
    LPM0,
    /// lower noise
    LPM1,
    /// lower power
    LPM2,
    /// lowest power
    LPM3,
}
impl LowPowerMode {
    /// Returns LPM0, the lowest noise (highest power) mode
    pub fn lowest_noise() -> Self {
        Self::LPM0
    }
    /// Returns LPM1, a lower noise (higher power) mode
    pub fn lower_noise() -> Self {
        Self::LPM1
    }
    /// Returns LPM2, a lower power (higher noise) mode
    pub fn lower_power() -> Self {
        Self::LPM2
    }
    /// Returns LPM3, the lowest power (highest noise) mode
    pub fn lowest_power() -> Self {
        Self::LPM3
    }
}

/// Options for what to read from the device when in auto mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutoReadTarget {
    /// most recently sampled temperature and relative humidity
    LastTempAndRelHumid,
    /// minimum temperature since auto mode was started
    MinTemp,
    /// maximum temperature since auto mode was started
    MaxTemp,
    /// minimum relative humidity since auto mode was started
    MinRelHumid,
    /// maximum relative humidity since auto mode was started
    MaxRelHumid,
}

/// Options for the on-device heater.  The datasheet claims this may be useful to drive off condensation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeaterLevel{
    /// heater off (post-reset default)
    Off,
    /// heater on at 25% power
    On25Percent,
    /// heater on at 50% power
    On50Percent,
    /// heater on at 100% power
    On100Percent,
}
impl HeaterLevel {
    pub(crate) fn setting(&self) -> Option<u16> {
        match self {
            HeaterLevel::Off => None,
            HeaterLevel::On25Percent => Some(0x9f),
            HeaterLevel::On50Percent => Some(0x3ff),
            HeaterLevel::On100Percent => Some(0x3FFF),
        }
    }
}

pub(crate) fn start_sampling_command(sample_rate: SampleRate, low_power_mode: LowPowerMode) -> u16 {
    match (sample_rate, low_power_mode) {
        (SampleRate::OneShot, LowPowerMode::LPM0) => 0x2400,
        (SampleRate::OneShot, LowPowerMode::LPM1) => 0x240b,
        (SampleRate::OneShot, LowPowerMode::LPM2) => 0x2416,
        (SampleRate::OneShot, LowPowerMode::LPM3) => 0x24ff,
        (SampleRate::Auto500mHz, LowPowerMode::LPM0) => 0x2032,
        (SampleRate::Auto500mHz, LowPowerMode::LPM1) => 0x2024,
        (SampleRate::Auto500mHz, LowPowerMode::LPM2) => 0x202f,
        (SampleRate::Auto500mHz, LowPowerMode::LPM3) => 0x20ff,
        (SampleRate::Auto1Hz, LowPowerMode::LPM0) => 0x2130,
        (SampleRate::Auto1Hz, LowPowerMode::LPM1) => 0x2126,
        (SampleRate::Auto1Hz, LowPowerMode::LPM2) => 0x212d,
        (SampleRate::Auto1Hz, LowPowerMode::LPM3) => 0x21ff,
        (SampleRate::Auto2Hz, LowPowerMode::LPM0) => 0x2236,
        (SampleRate::Auto2Hz, LowPowerMode::LPM1) => 0x2220,
        (SampleRate::Auto2Hz, LowPowerMode::LPM2) => 0x222b,
        (SampleRate::Auto2Hz, LowPowerMode::LPM3) => 0x22ff,
        (SampleRate::Auto4Hz, LowPowerMode::LPM0) => 0x2334,
        (SampleRate::Auto4Hz, LowPowerMode::LPM1) => 0x2322,
        (SampleRate::Auto4Hz, LowPowerMode::LPM2) => 0x2329,
        (SampleRate::Auto4Hz, LowPowerMode::LPM3) => 0x23ff,
        (SampleRate::Auto10Hz, LowPowerMode::LPM0) => 0x2737,
        (SampleRate::Auto10Hz, LowPowerMode::LPM1) => 0x2721,
        (SampleRate::Auto10Hz, LowPowerMode::LPM2) => 0x272a,
        (SampleRate::Auto10Hz, LowPowerMode::LPM3) => 0x27ff,
    }
}

// TODO: disable allow(unusued)
#[allow(unused)]
pub(crate) fn reset_state_value(sample_rate: SampleRate, low_power_mode: LowPowerMode) -> u16 {
    match (sample_rate, low_power_mode) {
        (SampleRate::OneShot, _) => 0x0081,
        (SampleRate::Auto500mHz, LowPowerMode::LPM0) => 0x03b0,
        (SampleRate::Auto500mHz, LowPowerMode::LPM1) => 0x13f3,
        (SampleRate::Auto500mHz, LowPowerMode::LPM2) => 0x2336,
        (SampleRate::Auto500mHz, LowPowerMode::LPM3) => 0x3375,
        (SampleRate::Auto1Hz, LowPowerMode::LPM0) => 0x05d2,
        (SampleRate::Auto1Hz, LowPowerMode::LPM1) => 0x1591,
        (SampleRate::Auto1Hz, LowPowerMode::LPM2) => 0x2554,
        (SampleRate::Auto1Hz, LowPowerMode::LPM3) => 0x3517,
        (SampleRate::Auto2Hz, LowPowerMode::LPM0) => 0x0774,
        (SampleRate::Auto2Hz, LowPowerMode::LPM1) => 0x1737,
        (SampleRate::Auto2Hz, LowPowerMode::LPM2) => 0x27f2,
        (SampleRate::Auto2Hz, LowPowerMode::LPM3) => 0x37b1,
        (SampleRate::Auto4Hz, LowPowerMode::LPM0) => 0x0916,
        (SampleRate::Auto4Hz, LowPowerMode::LPM1) => 0x1955,
        (SampleRate::Auto4Hz, LowPowerMode::LPM2) => 0x2990,
        (SampleRate::Auto4Hz, LowPowerMode::LPM3) => 0x39d3,
        (SampleRate::Auto10Hz, LowPowerMode::LPM0) => 0x0b09,
        (SampleRate::Auto10Hz, LowPowerMode::LPM1) => 0x1b4a,
        (SampleRate::Auto10Hz, LowPowerMode::LPM2) => 0x2b8f,
        (SampleRate::Auto10Hz, LowPowerMode::LPM3) => 0x3bcc,
    }
}

// TODO: disable allow(unusued)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    AutoExit,
    AutoReadTempAndRelHumid,
    AutoReadMinTemp,
    AutoReadMaxTemp,
    AutoReadMinRelHumid,
    AutoReadMaxRelHumid,
    
    #[allow(unused)]
    WriteSetLowAlert,
    #[allow(unused)]
    WriteSetHighAlert,
    #[allow(unused)]
    WriteClearLowAlert,
    #[allow(unused)]
    WriteClearHighAlert,
    #[allow(unused)]
    AlertToNV,

    #[allow(unused)]
    ReadSetLowAlert,
    #[allow(unused)]
    ReadSetHighAlert,
    #[allow(unused)]
    ReadClearLowAlert,
    #[allow(unused)]
    ReadClearHighAlert,

    HeaterEnable,
    HeaterDisable,
    HeaterConfig,

    StatusRead,
    StatusClear,

    #[allow(unused)]
    NVOffset,

    SoftReset,

    SerialID54,
    SerialID32,
    SerialID10,

    ManufacturerID,

    #[allow(unused)]
    ResetState,
}
impl Command {
    pub(crate) fn to_be_bytes(&self) -> [u8; 2] {
        match self {
            Self::AutoExit => 0x3093_u16,
            Self::AutoReadTempAndRelHumid => 0xe000_u16,
            Self::AutoReadMinTemp => 0xe002_u16,
            Self::AutoReadMaxTemp => 0xe003_u16,
            Self::AutoReadMinRelHumid => 0xe004_u16,
            Self::AutoReadMaxRelHumid => 0xe005_u16,

            Self::WriteSetLowAlert => 0x6100_u16,
            Self::WriteSetHighAlert => 0x611d_u16,
            Self::WriteClearLowAlert => 0x610b_u16,
            Self::WriteClearHighAlert => 0x6116_u16,
            Self::AlertToNV => 0x6155_u16,

            Self::ReadSetLowAlert => 0xe102_u16,
            Self::ReadSetHighAlert => 0xe11f_u16,
            Self::ReadClearLowAlert => 0xe109_u16,
            Self::ReadClearHighAlert => 0xe114_u16,

            Self::HeaterEnable => 0x306d_u16,
            Self::HeaterDisable => 0x3066_u16,
            Self::HeaterConfig => 0x306e_u16,

            Self::StatusRead => 0xf32d_u16,
            Self::StatusClear => 0x3041_u16,

            Self::NVOffset => 0xa004_u16,

            Self::SoftReset => 0x30a2_u16,

            Self::SerialID54 => 0x3683_u16,
            Self::SerialID32 => 0x3684_u16,
            Self::SerialID10 => 0x3685_u16,

            Self::ManufacturerID => 0x3781_u16,

            Self::ResetState => 0x61bb_u16,
        }.to_be_bytes()
    }
}

pub(crate) const STATUS_FIELD_LSBIT_AT_LEAST_ONE_ALERT: usize = 15;
pub(crate) const STATUS_FIELD_LSBIT_HEATER_ENABLED: usize = 13;
pub(crate) const STATUS_FIELD_LSBIT_RH_TRACKING_ALERT: usize = 11;
pub(crate) const STATUS_FIELD_LSBIT_T_TRACKING_ALERT: usize = 10;
pub(crate) const STATUS_FIELD_LSBIT_RH_HIGH_TRACKING_ALERT: usize = 9;
pub(crate) const STATUS_FIELD_LSBIT_RH_LOW_TRACKING_ALERT: usize = 8;
pub(crate) const STATUS_FIELD_LSBIT_T_HIGH_TRACKING_ALERT: usize = 7;
pub(crate) const STATUS_FIELD_LSBIT_T_LOW_TRACKING_ALERT: usize = 6;
pub(crate) const STATUS_FIELD_LSBIT_RESET_SINCE_CLEAR: usize = 4;
pub(crate) const STATUS_FIELD_LSBIT_CHECKSUM_FAILURE: usize = 0;

pub(crate) const STATUS_FIELD_WIDTH_AT_LEAST_ONE_ALERT: usize = 1;
pub(crate) const STATUS_FIELD_WIDTH_HEATER_ENABLED: usize = 1;
pub(crate) const STATUS_FIELD_WIDTH_RH_TRACKING_ALERT: usize = 1;
pub(crate) const STATUS_FIELD_WIDTH_T_TRACKING_ALERT: usize = 1;
pub(crate) const STATUS_FIELD_WIDTH_RH_HIGH_TRACKING_ALERT: usize = 1;
pub(crate) const STATUS_FIELD_WIDTH_RH_LOW_TRACKING_ALERT: usize = 1;
pub(crate) const STATUS_FIELD_WIDTH_T_HIGH_TRACKING_ALERT: usize = 1;
pub(crate) const STATUS_FIELD_WIDTH_T_LOW_TRACKING_ALERT: usize = 1;
pub(crate) const STATUS_FIELD_WIDTH_RESET_SINCE_CLEAR: usize = 1;
pub(crate) const STATUS_FIELD_WIDTH_CHECKSUM_FAILURE: usize = 1;

pub(crate) const MANUFACTURER_ID_TEXAS_INSTRUMENTS: u16 = 0x3000u16;

pub(crate) fn raw_temp_to_centigrade(raw: u16) -> f32 {
    -45.0 + 175.0 * (raw as f32) / 65536.0
}
pub(crate) fn raw_temp_to_fahrenheit(raw: u16) -> f32 {
    -49.0 + 315.0 * (raw as f32) / 65536.0
}
pub(crate) fn raw_rel_humid_to_percent(raw: u16) -> f32 {
    100.0 * (raw as f32) / 65536.0
}
