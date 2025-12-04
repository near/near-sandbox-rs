# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.2](https://github.com/near/near-sandbox-rs/compare/v0.3.1...v0.3.2) - 2025-12-04

### Other

- update nearcore version to 2.10.1 ([#38](https://github.com/near/near-sandbox-rs/pull/38))

## [0.3.1](https://github.com/near/near-sandbox-rs/compare/v0.3.0...v0.3.1) - 2025-12-02

### Added

- intall sandbox ([#36](https://github.com/near/near-sandbox-rs/pull/36))

## [0.3.0](https://github.com/near/near-sandbox-rs/compare/v0.2.3...v0.3.0) - 2025-12-02

### Other

- [**breaking**] updated near-account-id to v2 ([#35](https://github.com/near/near-sandbox-rs/pull/35))
- updated nearcore version to 2.10.0 ([#33](https://github.com/near/near-sandbox-rs/pull/33))

## [0.2.3](https://github.com/near/near-sandbox-rs/compare/v0.2.2...v0.2.3) - 2025-11-30

### Added

- Enabled support for arm64 Linux target ([#31](https://github.com/near/near-sandbox-rs/pull/31))

## [0.2.2](https://github.com/near/near-sandbox-rs/compare/v0.2.1...v0.2.2) - 2025-11-18

### Added

- introduced sandbox rpc calls for state-patching and network fast forwarding ([#18](https://github.com/near/near-sandbox-rs/pull/18))

### Other

- remove reqwest and disable default features ([#29](https://github.com/near/near-sandbox-rs/pull/29))
- Update nearcore version to 2.9.0 ([#20](https://github.com/near/near-sandbox-rs/pull/20))

## [0.2.1](https://github.com/near/near-sandbox-rs/compare/v0.2.0...v0.2.1) - 2025-10-27

### Fixed

- Upgraded dependencies (+ avoid `home` crate as it is not ment for usage outside of cargo) ([#23](https://github.com/near/near-sandbox-rs/pull/23))

### Other

- update nearcore version to 2.8.0 ([#17](https://github.com/near/near-sandbox-rs/pull/17))
- fixed compilation with resolver=3 edition=2024 ([#14](https://github.com/near/near-sandbox-rs/pull/14))
- Update repository url in Cargo.toml ([#12](https://github.com/near/near-sandbox-rs/pull/12))

## [0.2.0](https://github.com/near/near-sandbox-rs/compare/v0.1.0...v0.2.0) - 2025-07-18

### Fixed

- `generate` feature didn't compile ([#10](https://github.com/near/near-sandbox-rs/pull/10))

### Other

- removing unneeded legacy code ([#7](https://github.com/near/near-sandbox-rs/pull/7))
- Updated one left-over reference to the old near-sandbox-utils in README.md
- Renamed `near-sandbox-utils` references to `near-sandbox` in README.md

## [0.15.0](https://github.com/near/near-sandbox/compare/v0.14.0...v0.15.0) - 2025-05-12

### Other

- [**breaking**] updates near-sandbox to nearcore 2.6.2 ([#112](https://github.com/near/near-sandbox/pull/112))

## [0.14.0](https://github.com/near/near-sandbox/compare/v0.13.0...v0.14.0) - 2025-03-14

### Other

- [**breaking**] updates near-sandbox to nearcore 2.5.0 ([#109](https://github.com/near/near-sandbox/pull/109))

## [0.13.0](https://github.com/near/near-sandbox/compare/v0.12.0...v0.13.0) - 2024-12-17

### Other

- [**breaking**] updates near-sandbox to nearcore 2.4.0 (#106)

## [0.12.0](https://github.com/near/near-sandbox/compare/v0.11.0...v0.12.0) - 2024-11-15

### Other

- Updated near-sandbox version to 2.3.1 version ([#103](https://github.com/near/near-sandbox/pull/103))

## [0.11.0](https://github.com/near/near-sandbox/compare/v0.10.0...v0.11.0) - 2024-09-06

### Other
- Updates near-sandbox to 2.1.1 ([#93](https://github.com/near/near-sandbox/pull/93))

## [0.10.0](https://github.com/near/near-sandbox/compare/v0.9.0...v0.10.0) - 2024-08-15

### Other
- [**breaking**] updated neard to 2.0.0 ([#88](https://github.com/near/near-sandbox/pull/88))

## [0.9.0](https://github.com/near/near-sandbox/compare/v0.8.0...v0.9.0) - 2024-07-05

### Added
- Avoid different versions of near-sandbox binaries collision ([#72](https://github.com/near/near-sandbox/pull/72))

### Other
- Updated the default neard version to 1.40.0 ([#85](https://github.com/near/near-sandbox/pull/85))

## [0.8.0](https://github.com/near/near-sandbox/compare/v0.7.0...v0.8.0) - 2024-06-11

### Added
- Update default nearcore version to v1.38.0 ([#81](https://github.com/near/near-sandbox/pull/81))

## [0.7.0](https://github.com/near/near-sandbox/compare/v0.6.3...v0.7.0) - 2023-10-04

### Added
- use tokio instead of async-process as dependants use tokio runtime anyway ([#68](https://github.com/near/near-sandbox/pull/68))

### Fixed
- pin async-process crate ([#66](https://github.com/near/near-sandbox/pull/66))

### Other
- use SANDBOX_ARTIFACT_URL ([#74](https://github.com/near/near-sandbox/pull/74))

## [0.6.3](https://github.com/near/sandbox/compare/v0.6.2...v0.6.3) - 2023-09-30

### Added
- Expose DEFAULT_NEAR_SANDBOX_VERSION const
- run sandbox instance with --fast flag ([#56](https://github.com/near/sandbox/pull/56))
- Allow to specify verion of neard-sandbox ([#63](https://github.com/near/sandbox/pull/63))

### Other
- Fixed linting warnings
- point nearcore to latest mainnet release 1.35.0 ([#61](https://github.com/near/sandbox/pull/61))
- Update crate/Cargo.toml
- update dependencies
