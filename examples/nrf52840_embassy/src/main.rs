#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::twim::{self, Twim};
use embassy_nrf::{bind_interrupts, peripherals};
use embassy_time::Duration;
use embassy_time::Ticker;
use {defmt_rtt as _, panic_probe as _};

use hdc302x::{
    Hdc302x,
    AutoReadTarget as HdcAutoReadTarget,
    I2cAddr as HdcI2cAddr,
    LowPowerMode as HdcLowPowerMode,
    SampleRate as HdcSampleRate,
};

#[allow(unused_imports)] #[macro_use] extern crate defmt;

bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());

    let _gpo_hdc_nrst = Output::new(p.P0_06, Level::High, OutputDrive::Standard);
    let mut hdc302x = {
        let mut config = twim::Config::default();
        config.sda_pullup = true;
        config.scl_pullup = true;
        let twi = Twim::new(p.TWISPI0, Irqs, p.P0_08, p.P0_12, config, &mut []);
        Hdc302x::new(twi, embassy_time::Delay, HdcI2cAddr::Addr00)
    };

    info!("One-Shot Test");
    hdc302x.software_reset_async().await.unwrap();

    let mut ticker = Ticker::every(Duration::from_millis(100));
    for _ in 0..4 {
        ticker.next().await;

        let raw_datum = hdc302x.one_shot_async(HdcLowPowerMode::lowest_noise()).await
            .unwrap();

        let d = hdc302x::Datum::from(&raw_datum);
        info!("  {:#?}", d);
    }

    info!("Auto-Mode Test");
    hdc302x.software_reset_async().await
        .unwrap();

    let mut ticker = Ticker::every(Duration::from_millis(5_000));
    loop {
        // start hdc302x auto
        hdc302x.auto_start_async(
            HdcSampleRate::Auto500mHz,
            HdcLowPowerMode::lowest_power()
        ).await
            .unwrap();

        // wait for min/max to accumulate for a while
        ticker.next().await;

        let t_min = hdc302x.auto_read_async(HdcAutoReadTarget::MinTemp).await.unwrap().fahrenheit();
        let t_max = hdc302x.auto_read_async(HdcAutoReadTarget::MaxTemp).await.unwrap().fahrenheit();
        let rh_min = hdc302x.auto_read_async(HdcAutoReadTarget::MinRelHumid).await.unwrap().humidity_percent();
        let rh_max = hdc302x.auto_read_async(HdcAutoReadTarget::MaxRelHumid).await.unwrap().humidity_percent();

        // Stop hdc3022 auto mode to restart min/max accumulation
        hdc302x.auto_stop_async().await
            .unwrap();

        if let (Some(t_min), Some(t_max), Some(rh_min), Some(rh_max))= (t_min, t_max, rh_min, rh_max)
        && t_max >= t_min
        && rh_max >= rh_min {
            info!("  {} - {} °F, {} - {} %RH", t_min, t_max, rh_min, rh_max);
        }
    }
}
