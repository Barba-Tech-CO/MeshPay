//! Parameters. Source: docs/prd_meshpay_v3.md 9.2 and 9.5; values marked [ILU] there are
//! illustrative and still need owner decision.
pub const THRESHOLD: u8 = 3; // 9.2, 3-of-5
pub const GUARDIANS: usize = 5; // 9.2
pub const HOUR: u64 = 3_600;
pub const DAY: u64 = 24 * HOUR;
pub const T_REC: u64 = 72 * HOUR; // 9.5 item 1 [ILU]
pub const CANCEL_BLOCK: u64 = 7 * DAY; // 9.5 item 2 [ILU]
pub const RATE_WINDOW: u64 = 30 * DAY; // 9.5 item 3 [ILU]
pub const MAX_REQUESTS_PER_WINDOW: usize = 3; // 9.5 item 3 [ILU]
