# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-08-16

### Breaking changes

- `auto_start(SampleRate::OneShot, _)` and `auto_start_async` now return
  `Error::InvalidInputData` without I²C traffic. Use `one_shot` or
  `one_shot_async` instead.
- Raw conversion uses the documented inclusive full-scale range. Values for
  nonzero raw readings change slightly; in particular, `u16::MAX` now converts
  to 130 °C, 266 °F, and 100% RH. Update golden/reference values accordingly.

### Added

- Add `Hdc302x::into_parts` to return constructor resources for
  application-controlled lifecycle handling after completed operations.

### Fixed

- Update the optional `defmt` integration to 1.1.1, removing an upstream
  future-incompatibility warning. Applications that enable this feature must
  resolve `defmt` 1.1.1 or later.
- Correct automatic-mode documentation: the first completed sample is valid;
  unavailable or consumed latest results are reported as I²C errors.
- Document the hardware-validated HDC3022 behavior that `auto_stop` clears
  extrema, which differs from TI's reset-only extrema wording.
- Clarify that extrema reads are non-clearing snapshots; document the observed
  HDC302x testing scope, TI discrepancy, and fresh-interval operational
  assumption for `auto_stop`.
- Mark platform-specific top-level examples as illustrative so doctests do not
  try to compile their hardware placeholders.
- Correct raw temperature/RH conversion endpoints, append the required heater
  configuration CRC, and reject `auto_start(SampleRate::OneShot, _)`.
- Repair the advertised `serde` and `bincode` features and eliminate
  single-logging-feature warnings.
- Document status-clear ordering, serial-number byte/display ordering, and the
  boundary between validated heater protocol behavior and unmeasured thermal
  output; correct public prose and README blocking-trait wording.

### Upgrade from 0.4.1

- Most applications require no source changes. `Hdc302x::into_parts` is
  optional and returns constructor resources after a completed operation; it
  does not guarantee transport recovery.
- Replace any use of `auto_start(SampleRate::OneShot, ...)` with `one_shot` or
  `one_shot_async`.
- Continue to handle unavailable automatic latest results as `Error::I2c`.
  Corrected heater frames and serialization features require no expected
  call-site change; update conversion golden/reference values as above.

## [0.1.0](https://github.com/bobsrac/hdc302x-rs/releases/tag/v0.1.0) - 2024-08-21

### Other

- embedded-hal-async driver for HDC302x(-Q1) temp/RH sensor

## [0.2.0](https://github.com/bobsrac/hdc302x-rs/releases/tag/v0.2.0) - 2024-08-21

### Other

- added processing fns to RawDatum and a usage example for docs.rs front page
