//! SolStarter Staking program
#![deny(missing_docs)]

pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;
pub mod utils;

pub use utils::borsh;
pub use utils::invoke;
pub use utils::math;
pub use utils::prelude;
pub use utils::program;

/// Current program version
pub const PROGRAM_VERSION: u8 = 1;

#[cfg(not(feature = "no-entrypoint"))]
pub mod entrypoint;

// Export current sdk types for downstream users building with a different sdk version
pub use solana_program;
use utils::program::ProgramPubkey;

solana_program::declare_id!("AHvm4wFiJmDw8rf6MYe7nmYopsZ6nFW6dwojt8BVzAfE");

/// Seed for the lock account
pub const LOCK_SEED: &str = "LOCK";

/// typed id
pub fn program_id() -> ProgramPubkey {
    ProgramPubkey(crate::id())
}

#[cfg(all(feature = "test-bpf", test))]
mod tests;

/// number of tiers
pub const TIERS_COUNT: usize = 4;
