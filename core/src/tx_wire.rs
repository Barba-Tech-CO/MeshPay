//! Hand-written prost mirror of the PRD section 10 messages used by the two-phase commit,
//! one message per file.
pub mod ack_receipt;
pub mod cheque_spend;
pub mod cheque_spend_body;
pub mod tx_propose;
