# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

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

## [0.1.0](https://github.com/bobsrac/hdc302x-rs/releases/tag/v0.1.0) - 2024-08-21

### Other

- embedded-hal-async driver for HDC302x(-Q1) temp/RH sensor

## [0.2.0](https://github.com/bobsrac/hdc302x-rs/releases/tag/v0.2.0) - 2024-08-21

### Other

- added processing fns to RawDatum and a usage example for docs.rs front page
