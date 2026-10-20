//! Every integration test of this crate as one binary: one link of the whole
//! application instead of one per file. Each file stays where it was and reads
//! as it did on its own; a file under `tests/` that is not a module here, or a
//! target of its own in `Cargo.toml`, does not run, and `suites.rs` in the
//! cli's tests fails when one is missing.

#[path = "attestation_fixture.rs"]
mod attestation_fixture;
#[path = "drive_port.rs"]
mod drive_port;
#[path = "inbox_readings.rs"]
mod inbox_readings;
#[path = "inbox_round_trip.rs"]
mod inbox_round_trip;
#[path = "legacy_stores.rs"]
mod legacy_stores;
#[path = "model_port.rs"]
mod model_port;
#[path = "stores.rs"]
mod stores;
