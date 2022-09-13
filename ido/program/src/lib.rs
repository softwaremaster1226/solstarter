#![deny(missing_docs)]
//#![feature(min_const_generics)]

//! SolStarter program

pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;
pub mod utils;

use sol_starter_staking::program::ProgramPubkey;
pub use utils::invoke;
pub use utils::math;

/// Current program version
pub const PROGRAM_VERSION: u8 = 1;

/// tiers count
pub const TIERS_COUNT: usize = 4;

/// in use
pub const STAGES_ACTIVE_COUNT: usize = 2;

#[cfg(not(feature = "no-entrypoint"))]
pub mod entrypoint;

// Export current sdk types for downstream users building with a different sdk version
pub use solana_program;

solana_program::declare_id!("FY4Vb99dAuPa4ujpFBYzaHJYx9zaYgNJxnoe4FkoPbcA");

/// Seed for the accounts holding KYC information
pub const KYC_SEED: &str = "kyc";

/// marker type for collection token amount
type CollectionToken = u64;

/// market type for distribution amount
type DistributionToken = u64;

#[cfg(all(feature = "test-bpf", test))]
mod tests;

/// typed id
pub fn program_id() -> ProgramPubkey {
    ProgramPubkey(crate::id())
}

/// dependant program id
pub fn spl_token_id() -> ProgramPubkey {
    ProgramPubkey(spl_token::id())
}
