# Rust HDC302x(-Q1) Low-Power Humidity and Temperature Digital Sensor Driver

This is a platform-agnostic Rust driver for the HDC3020, HDC3021, HDC3022, HDC3020-Q1,
HDC3021-Q1 and HDC3022-Q1 low-power humidity and temperature digital sensors using
the [`embedded-hal-async`] traits. This is current no support for a blocking API.
This driver was inspired by [Diego Barrios Romero's hdc20xx-rs driver](https://github.com/eldruin/hdc20xx-rs).

[`embedded-hal-async`]: https://github.com/rust-embedded/embedded-hal/tree/master/embedded-hal-async

This driver allows you to:

- Start and read samples in both one-shot and auto (self-timed) mode.
- Read last temperature and humidity values in auto mode.
- Read minimum and maximum temperature and humidity values in auto mode.
- Exit auto mode.
- Enable/disable the heater, including 100%, 50%, and 25% settings.
- Trigger a software reset.
- Read the manufacturer ID.
- Read the device serial number.
- Read and optionally clear the device status bits.
- async support.

This driver does not yet support the following device features:

- Alerts (read/write and non-volatile storage of setpoints).
- Offset calibration (non-volatile storage of temperature and relative humidity
  offsets).
- Configuration of post-reset state (default behavior after power-on and software
  reset).
- Blocking API support.

## Supported devices: HDC3020, HDC3021, HDC3022, HDC3020-Q1, HDC3021-Q1, HDC3022-Q1

The following description is copied from the manufacturer's datasheet:

The HDC302x-Q1 is an integrated capacitive based relative humidity (RH) and temperature
sensor. The device provides high accuracy measurements over a wide supply range
(1.62 V – 5.5 V), along with ultra-low power consumption in a compact 2.5-mm × 2.5-mm
package. Both the temperature and humidity sensors are 100% tested and trimmed
on a production setup that is NIST traceable and verified with equipment that is
calibrated to ISO/IEC 17025 standards.

Offset Error Correction reduces RH sensor offset due to aging, exposure to extreme
operating conditions, and contaminants to return device to within accuracy specifications.
For battery IoT applications, auto measurement mode and ALERT feature enable low
system power by maximizing MCU sleep time. There are four different I2C addresses
that support speeds up to 1 MHz. A heating element is available to dissipate condensation
and moisture.

The HDC3020-Q1 is an open cavity package without protective cover. Two device variants
have a cover option to protect the open cavity RH sensor: HDC3021-Q1 and HDC3022-Q1.
HDC3021-Q1 has removable protective tape to allow conformal coatings and PCB
wash. HDC3022-Q1 has a permanent IP67 filter membrane to protect against dust, water
and PCB wash. All three package variants have wettable flanks option.

Datasheets:
  [HDC302x](https://www.ti.com/lit/ds/symlink/hdc3020.pdf)
  [HDC302x-Q1](https://www.ti.com/lit/ds/symlink/hdc3020-q1.pdf)

## Usage examples

To use this driver, import this crate and an `embedded_hal_async` implementation,
then instantiate the device.

## Future Work

- Blocking API

## License

MIT OR Apache-2.0
