//! Program owned state

use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use solana_program::clock::UnixTimestamp;
use solana_program::pubkey::Pubkey;
use solana_program::{entrypoint::ProgramResult, program_error::ProgramError};

/// state version
#[repr(C)]
#[derive(Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub enum StateVersion {
    /// new
    Uninitialized,
    /// version 1
    V1,
}

impl Default for StateVersion {
    fn default() -> Self {
        StateVersion::Uninitialized
    }
}

/// pool state
#[repr(C)]
#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema, Default)]
pub struct StakePool {
    /// version
    pub version: StateVersion,
    /// Account accumulating staked SOS tokens
    pub token_account_sos: Pubkey,
    /// Mint issuing pool tokens to the users (xSOS)
    pub pool_mint_xsos: Pubkey,
    /// Authority controlling locking freeze/unfreeze
    pub ido_authority: Pubkey,
    /// Number of tier users
    pub tier_users: [u32; crate::TIERS_COUNT],
    /// Balance qualifying to each of the tiers (in ascending order)
    pub tier_balance: [u64; crate::TIERS_COUNT],

    /// Number of seconds SOS tokens are stuck in [TransitDirection::Incoming] transit
    pub transit_incoming: UnixTimestamp,

    /// Number of seconds SOS tokens are stuck in [TransitDirection::Outgoing] transit
    pub transit_outgoing: UnixTimestamp,

    /// if now is less than this - prevents [Instruction::Unlock]
    pub pool_active_until: UnixTimestamp,
}

/// flow of stake
#[repr(C)]
#[derive(Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub enum TransitDirection {
    /// something went wrong
    Uninitialized,
    /// from user to pool
    Incoming,
    /// from pool to user
    Outgoing,
}

impl Default for TransitDirection {
    fn default() -> Self {
        TransitDirection::Uninitialized
    }
}

/// derived
#[repr(C)]
#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema, Default)]
pub struct PoolTransit {
    /// version
    pub version: StateVersion,
    /// [StakePool] this transit area belongs to
    pub pool: Pubkey,
    /// `Incoming` for tokens coming into the pool, `Outgoing` for the tokens coming out
    pub direction: TransitDirection,
    /// User wallet controlling this transit record
    pub user_wallet: Pubkey,
    /// Account holding SOS tokens in the transit record
    pub token_account_sos: Pubkey,
    /// Transit starting timestamp
    pub transit_from: UnixTimestamp,
    /// Timestamp when tokens can be pulled out of transit in slots    
    pub transit_until: UnixTimestamp,

    /// Amount already claimed from this transit record
    pub amount_claimed: u64,
}

/// derived from pool and user_wallet (unique per such pair), can withdraw only via program
#[repr(C)]
#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema, Default)]
pub struct PoolLock {
    /// version
    pub version: StateVersion,
    /// [StakePool] this lock belongs to
    pub pool: Pubkey,
    /// User wallet controlling this lock record
    pub user_wallet: Pubkey,
    /// Token account storing locked xSOS tokens
    pub token_account_xsos: Pubkey,
}

impl StakePool {
    /// LEN
    pub const LEN: usize = 169;
    /// Check if already initialized
    pub fn uninitialized(&self) -> ProgramResult {
        if self.version == StateVersion::Uninitialized {
            Ok(())
        } else {
            Err(ProgramError::AccountAlreadyInitialized)
        }
    }
    /// Error if not initialized
    pub fn initialized(&self) -> ProgramResult {
        if self.version != StateVersion::Uninitialized {
            Ok(())
        } else {
            Err(ProgramError::UninitializedAccount)
        }
    }
}

impl PoolLock {
    /// LEN
    pub const LEN: usize = 97;
    /// Check if already initialized
    pub fn uninitialized(&self) -> ProgramResult {
        if self.version == StateVersion::Uninitialized {
            Ok(())
        } else {
            Err(ProgramError::AccountAlreadyInitialized)
        }
    }
    /// Error if not initialized
    pub fn initialized(&self) -> ProgramResult {
        if self.version != StateVersion::Uninitialized {
            Ok(())
        } else {
            Err(ProgramError::UninitializedAccount)
        }
    }
}

impl PoolTransit {
    /// LEN
    pub const LEN: usize = 122;
    /// Check if already initialized
    pub fn uninitialized(&self) -> ProgramResult {
        if self.version == StateVersion::Uninitialized {
            Ok(())
        } else {
            Err(ProgramError::AccountAlreadyInitialized)
        }
    }
    /// Error if not initialized
    pub fn initialized(&self) -> ProgramResult {
        if self.version != StateVersion::Uninitialized {
            Ok(())
        } else {
            Err(ProgramError::UninitializedAccount)
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn len() {
        assert_eq!(
            StakePool::LEN,
            StakePool::default().try_to_vec().unwrap().len()
        );
        assert_eq!(
            PoolLock::LEN,
            PoolLock::default().try_to_vec().unwrap().len()
        );
        assert_eq!(
            PoolTransit::LEN,
            PoolTransit::default().try_to_vec().unwrap().len()
        );
    }
}

/// gets tier for ticket
pub fn get_tier(tier_balance: [u64; crate::TIERS_COUNT], pool_lock_amount: u64) -> Option<usize> {
    tier_balance
        .iter()
        .enumerate()
        .rfind(|(_, val)| pool_lock_amount >= **val)
        .map(|(i, _)| i)
}
