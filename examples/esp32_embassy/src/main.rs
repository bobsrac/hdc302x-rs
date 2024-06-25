use core::cell::RefCell;
use critical_section::Mutex;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex as CSRawMutex,
    channel::Channel,
};
use esp_hal::{
    Async,
    i2c::I2C,
    interrupt::{self, Priority},
    peripherals::{I2C0, Interrupt},
    prelude::*,
    timer::{
        PeriodicTimer,
        systimer::{Alarm, Periodic},
    },
};
use fugit::ExtU64; // extend u32 with duration methods
use hdc302x_async::{
    Hdc302x,
    ManufacturerId,
    Datum as Hdc302xDatum,
};

#[main]
async fn main(_spawner: Spawner) {
    esp_println::logger::init_logger(log::LevelFilter::Info);

    let peripherals = Peripherals::take();

    // Clocks
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = mk_static!(Clocks, ClockControl::max(system.clock_control).freeze());

    // Io: provides access to the GPIO and IO_MUX peripherals
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    // System Timer: provides an Alarm instance, which implements
    // embedded_hal_async::delay::DelayNs
    let system_timer = SystemTimer::new_async(peripherals.SYSTIMER);

    // Hdc302x
    let mut hdc302x = Hdc302x::new(
        I2C::new_async(
            peripherals.I2C0,
            io.pins.gpio6,
            io.pins.gpio7,
            400.kHz(),
            &clocks,
        ),
        system_timer.alarm0.into_periodic(),
        Hdc302xI2cAddr::Addr00
    );

    log::info!("Manufacturer ID");
    match hdc302x.read_manufacturer_id().await {
        Ok(ManufacturerId::TexasInstruments) => {
            log::info!("hdc302x: manufacturer id: {}", ManufacturerId::TexasInstruments);
        },
        Ok(manuf_id) => {
            log::warn!("hdc302x: unexpected manufacturer id: {manuf_id}");
            return;
        },
        Err(e) => {
            log::error!("hdc302x: read_manufacturer_id error: {e:?}");
            return;
        },
    }

    log::info!("Serial Number");
    match hdc302x.read_serial_number().await {
        Ok(serial_number) => {
            log::info!("hdc302x: serial_number: {serial_number}");
        },
        Err(e) => {
            log::error!("hdc302x: read_serial_number error: {e:?}");
            return;
        },
    }

    log::info!("Status");
    match hdc302x.read_status(true).await {
        Ok(status_bits) => {
            log::info!("hdc302x: status_bits: {status_bits}");
        },
        Err(e) => {
            log::error!("hdc302x: read_status error: {e:?}");
            return;
        },
    }

    match hdc302x.one_shot(hdc302x_async::LowPowerMode::lowest_noise()).await {
        Ok(raw_datum) => {
            log::info!("hdc302x: one_shot raw_datum: {raw_datum:?}");
            match (&raw_datum).into() {
                Hdc302xDatum::TempAndRelHumid(hdc302x_temp_and_rel_humid) => {
                    let temp_and_rel_humid = messages::TempAndRelHumid {
                        humidity_percent: hdc302x_temp_and_rel_humid.humidity_percent,
                        fahrenheit: hdc302x_temp_and_rel_humid.fahrenheit,
                    };
                    log::info!("hdc302x: temp {} F, RH {} %", temp_and_rel_humid.fahrenheit, temp_and_rel_humid.humidity_percent);
                },
                datum => {
                    log::warn!("hdc302x: unexpected datum: {datum:?}");
                },
            }
        },
        Err(e) => {
            log::error!("hdc302x: one_shot error: {e:?}");
            return;
        },
    }
}
