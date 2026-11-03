//! LAB ONLY prototype of PRD v3 section 9 (social recovery). Software keys stand in for
//! hardware keys; no real value. See docs/estudo/06c-recuperacao-social-pv-rec.md.
pub mod account;
pub mod cheque;
pub mod codec;
pub mod device;
pub mod error;
pub mod keys;
pub mod ledger;
pub mod params;
pub mod pending;
pub mod recovery_cancel;
pub mod recovery_cert;
pub mod reservation;
pub mod sealing;
pub mod shares;
pub mod snapshot;
