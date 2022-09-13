//! Program state definitions
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::cast::FromPrimitive;
use num_traits::ToPrimitive;

use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use sol_starter_staking::state::get_tier;
use solana_program::{
    clock::{Clock, UnixTimestamp},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use std::convert::TryFrom;
use strum::EnumCount;

use crate::{
    error::Error,
    math::{ErrorAddSub, ErrorMulDiv},
    CollectionToken, DistributionToken, TIERS_COUNT,
};

/// Uninitialized version of entity
pub const UNINITIALIZED_VERSION: u8 = 0;

/// Current market version
pub const MARKET_VERSION: u8 = 1;
/// Current version
pub const USER_KYC_VERSION: u8 = 1;
/// Current pool version
pub const POOL_VERSION: u8 = 1;
/// Current user pool version
pub const USER_POOL_STAGE_VERSION: u8 = 1;

/// Whitelist token transfer amount
pub const WHITELIST_TOKEN_AMOUNT: u8 = 1;
/// Default key for mint whitelist
pub const DEFAULT_WHITELIST_KEY: Pubkey = Pubkey::new_from_array([0; 32]);

/// Is a group of pools.
#[repr(C)]
#[derive(Debug, Default, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct Market {
    /// Market state variable version
    pub version: u8,
    /// Market owner can initialize pools for market
    pub owner: Pubkey,
    /// [sol_starter_staking::StakingPool] account to calculate user tier allocations.    
    pub stake_pool: Pubkey,
}

impl Market {
    /// Market LEN
    pub const LEN: usize = 65;
    /// Check if already initialized
    pub fn uninitialized(&self) -> ProgramResult {
        if self.version == UNINITIALIZED_VERSION {
            Ok(())
        } else {
            Err(ProgramError::AccountAlreadyInitialized)
        }
    }
    /// Error if not initialized
    pub fn initialized(&self) -> ProgramResult {
        if self.version != UNINITIALIZED_VERSION {
            Ok(())
        } else {
            Err(ProgramError::UninitializedAccount)
        }
    }
}

/// KYC requirement
#[repr(C)]
#[derive(Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema, Clone, Copy)]
pub enum KycRequirement {
    /// no need for KYC (verification)
    NotRequired,
    /// Any KYC presence is required for participation
    AnyRequired,
}

/// small seconds positive duration
pub type UnixTimeSmallDuration = u32;

/// user pool stage marker account
#[repr(C)]
#[derive(Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct UserPoolStage {}

impl UserPoolStage {
    /// LEN
    pub const LEN: usize = 0;
}

/// Is a campaign to sell tokens, with rate, goal, min/max investment etc.
/// Are created by [Market::market_owner]  with [collected tokens](Self::account_collection) and (given tokens)[Self::account_distribution]
#[repr(C)]
#[derive(Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct Pool {
    /// Data version
    pub version: u8,
    /// Market reference
    pub market: Pubkey,
    /// Token account for tokens used as investment ([CollectionToken])
    pub account_collection: Pubkey,
    /// Token account for tokens to be distributed
    pub account_distribution: Pubkey,
    /// Mint for the pool tokens (minted on purchase)
    pub mint_pool: Pubkey,
    /// None for public pool, mint for the whitelist tokens (who has them can participate in whitelist)
    pub mint_whitelist: MintWhitelist,
    /// KYC requirement
    pub kyc_requirement: KycRequirement,
    /// price * account_collection = account_distribution
    pub price: u64,
    /// Maximum amount to be collected
    pub goal_max_collected: CollectionToken,
    /// Minimum amount of be collected
    pub goal_min_collected: CollectionToken,
    /// Min investment size
    pub amount_investment_min: CollectionToken,
    /// Max investment size
    pub amount_investment_max: CollectionToken,
    /// Time when the pool starts accepting investments
    pub time_start: UnixTimestamp,
    /// Time when the pool stops accepting investments (and starts token distribution)
    pub time_finish: UnixTimestamp,
    /// Amount collected
    pub amount_collected: CollectionToken,
    /// Amount to distribute in distribution tokens
    pub amount_to_distribute: DistributionToken,
    /// Pool owner (the one who can sign transaction to claim money from [Self::account_collection] and [Self::account_distribution] accounts)
    pub owner: Pubkey,
    /// Pool authority
    pub authority: Pubkey,
    /// Authority bump seed
    pub authority_bump_seed: u8,
    /// Stores amounts available for each user tier (according to [sol_starter_staking::StakingPool] account).
    pub tier_allocation: [DistributionToken; TIERS_COUNT],

    /// there total allocations for each tier (before dividing by the number of users)
    pub tier_remaining: [DistributionToken; TIERS_COUNT],

    /// non overlapped time for stages
    pub time_table: [UnixTimeSmallDuration; crate::STAGES_ACTIVE_COUNT],
}

/// Mint whitelist enum
#[derive(Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub enum MintWhitelist {
    /// Key of mint whitelist
    Key(Pubkey),
    /// No key provided
    None(Pubkey),
}

impl Pool {
    /// Pool LEN
    pub const LEN: usize = 372;
    /// Check if already initialized
    pub fn uninitialized(&self) -> ProgramResult {
        if self.version == UNINITIALIZED_VERSION {
            Ok(())
        } else {
            Err(ProgramError::AccountAlreadyInitialized)
        }
    }
    /// Error if not initialized
    pub fn initialized(&self) -> ProgramResult {
        if self.version != UNINITIALIZED_VERSION {
            Ok(())
        } else {
            Err(ProgramError::UninitializedAccount)
        }
    }

    /// Price precision
    pub const PRECISION: u64 = 1_000_000_000;

    /// success
    pub fn success(&self) -> bool {
        self.amount_collected >= self.goal_min_collected
    }

    /// Transform collected tokens to distributed
    pub fn collected_to_distributed(
        &self,
        amount_collected: CollectionToken,
    ) -> Result<DistributionToken, ProgramError> {
        let amount_collected = amount_collected as u128;
        let price = self.price as u128;

        // consistent with SOL/lamports logic
        let distributed = amount_collected
            .error_mul(Self::PRECISION as u128)?
            .error_div(price)?;
        DistributionToken::try_from(distributed).map_err(|_| Error::Overflow.into())
    }

    /// The point of having two fields there is to keep exact cumulative amounts we need for the pool.
    /// Each purchase has a potential rounding error when multiplying by price, so we need to sum up all those individual amounts and not recalculate the whole amount by multiplying it by price.                
    pub fn update_distributed_from_collected(
        &mut self,
        amount: CollectionToken,
        tier: Option<usize>,
        stage: Stage,
    ) -> ProgramResult {
        let amount_to_distribute = self.collected_to_distributed(amount)?;
        if stage != Stage::FinalStage {
            if let Some(tier) = tier {
                self.tier_remaining[tier] =
                    self.tier_remaining[tier].error_sub(amount_to_distribute)?;
            }
        }

        self.amount_to_distribute = self.amount_to_distribute.error_add(amount_to_distribute)?;

        Ok(())
    }

    /// Sets allocations according tiers
    /// ```python
    /// total_raise_distributed = goal_max_collected * PARTS / price
    /// w[i]= tier_balance[i]/tier_balance[0]
    /// total_shares = sum(tier_users[i] * w[i])
    /// share = total_raise_distributed / total_shares
    /// tier_allocation[i]=  share  * w[i]
    /// # one liner
    /// tier_allocation[i]=  total_raise_distributed  * tier_balance[i] / ( tier_balance[0] * sum(tier_users[i] * w[i]))
    /// ```
    ///```rust    
    /// let goal_max_collected = 1_000_000.;
    /// let PARTS = 1_000_000_000.;
    /// let price = 1_000_000_000.;
    /// let tier_balance = [5000., 9000., 16000.,30000.];
    /// let mut w = [0.; 4];
    /// let tier_users = [100., 50., 25., 10.];
    ///
    /// let total_raise_distributed = goal_max_collected * PARTS / price;
    ///
    /// for i in 0..4 {
    ///   w[i] = tier_balance[i] / tier_balance[0];
    /// }
    /// let mut total_shares = 0.;
    /// for i in 0..4 {
    ///   total_shares += tier_users[i] * w[i];
    /// }
    /// let share = total_raise_distributed / total_shares;
    /// assert_eq!(share, 3030.3030303030305);
    /// ```
    ///    
    pub fn set_tier_allocations(
        &mut self,
        tier_users: [u32; crate::TIERS_COUNT],
        tier_balance: [u64; crate::TIERS_COUNT],
    ) -> ProgramResult {
        let tier_balance: Vec<u128> = tier_balance.iter().map(|x| u128::from(*x)).collect();
        let tier_users: Vec<u128> = tier_users.iter().map(|x| u128::from(*x)).collect();
        let price = self.price as u128;
        let goal_max_collected = self.goal_max_collected as u128;

        let mut total_shares: u128 = 0;
        for i in 0..TIERS_COUNT {
            let share = tier_balance[i].error_mul(tier_users[i])?;
            total_shares = total_shares.error_add(share)?;
        }

        for (i, tier_balance) in tier_balance.iter().enumerate().take(TIERS_COUNT) {
            let per_tier_distributed = tier_balance
                .error_mul(goal_max_collected)?
                .error_mul(Self::PRECISION as u128)?
                .error_div(price)?
                .error_div(total_shares)?;
            self.tier_remaining[i] = u64::try_from(per_tier_distributed.error_mul(tier_users[i])?)
                .map_err(|_| Error::Overflow)?;
            let per_tier_distributed =
                u64::try_from(per_tier_distributed).map_err(|_| Error::Overflow)?;
            self.tier_allocation[i] = per_tier_distributed;
        }

        Ok(())
    }

    /// get current stage
    pub fn get_current_stage(&self, clock: &Clock) -> Result<Stage, ProgramError> {
        if self.time_start > clock.unix_timestamp || self.time_finish < clock.unix_timestamp {
            return Err(Error::CantDepositAtCurrentTime.into());
        }

        let mut accumulate = clock.unix_timestamp - self.time_start;

        for (i, value) in self.time_table[..Stage::COUNT - 1].iter().enumerate() {
            let value = *value as i64;
            if accumulate < value {
                return Ok(Stage::from_usize(i).unwrap());
            }
            accumulate -= value;
        }

        Ok(Stage::FinalStage)
    }

    /// Check investment amount according to the stage rules.
    pub fn stage_investment(
        &self,
        amount: CollectionToken,
        stage: Stage,
        tier_balance: [u64; crate::TIERS_COUNT],
        pool_lock_amount: u64,
    ) -> Result<(CollectionToken, Option<usize>), ProgramError> {
        let tier = get_tier(tier_balance, pool_lock_amount);
        let possible_amount = match (stage, tier) {
            (Stage::InitialStage, Some(tier)) => tier_balance[tier],
            (Stage::TierAllocationStage, Some(tier)) => self.tier_remaining[tier],
            (Stage::FinalStage, _) => amount,
            _ => return Err(Error::AccountOnThisTierCannotParticipateOnCurrentStage.into()),
        };
        Ok((amount.min(possible_amount), tier))
    }

    /// errors if not started
    pub fn was_started(&self, now: UnixTimestamp) -> ProgramResult {
        self.initialized()?;
        if self.time_start < now {
            Ok(())
        } else {
            Err(Error::CanParticipateOnlyInStartedPool.into())
        }
    }
}

/// Pool stages
#[repr(C)]
#[derive(
    Debug,
    PartialEq,
    BorshDeserialize,
    BorshSerialize,
    BorshSchema,
    FromPrimitive,
    ToPrimitive,
    strum_macros::EnumCount,
    Copy,
    Clone,
)]
pub enum Stage {
    /// On this stage can invest amount equal [tier_balance] (per investor) based on [pool_lock] amount
    InitialStage,
    /// On this stage can invest any amount equal [tier_remaining] for invester tier (shared by all investors on this thier)
    TierAllocationStage,
    /// On this stage can invest any amount, but not more than remaining total pool collection amount
    FinalStage,
}

impl Stage {
    /// to bytes
    pub fn to_be_bytes(self) -> [u8; 1] {
        self.to_u8().unwrap().to_be_bytes()
    }
}

/// verified credentials reference (from Know Your Customer process)
#[repr(C)]
#[derive(Debug, PartialEq, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct MarketUserKyc {
    /// Data version
    pub version: u8,
    /// market
    pub market: Pubkey,
    /// user
    pub user_wallet: Pubkey,
    /// expiration time of self
    pub expiration: UnixTimestamp,
}

impl MarketUserKyc {
    /// LEN
    pub const LEN: usize = 73;

    /// Error if not initialized
    pub fn uninitialized(&self) -> ProgramResult {
        if self.version == UNINITIALIZED_VERSION {
            Ok(())
        } else {
            Err(ProgramError::AccountAlreadyInitialized)
        }
    }
    /// Error if not uninitialized
    pub fn initialized(&self) -> ProgramResult {
        if self.version != UNINITIALIZED_VERSION {
            Ok(())
        } else {
            Err(ProgramError::UninitializedAccount)
        }
    }
}

#[cfg(test)]
mod tests {
    use sol_starter_staking::TIERS_COUNT;

    use super::*;

    #[test]
    fn test_pack_pool() {
        let goal_max = 10;
        let price = 10;
        let pool = pool_new(price, goal_max);

        let packed = pool.try_to_vec().unwrap();

        assert_eq!(
            packed.len(),
            solana_program::borsh::get_packed_len::<Pool>()
        );
        assert_eq!(Pool::LEN, solana_program::borsh::get_packed_len::<Pool>());

        let unpacked = Pool::try_from_slice(packed.as_slice()).unwrap();

        assert_eq!(pool, unpacked);
    }

    fn pool_new(price: u64, goal_max: u64) -> Pool {
        let pool = Pool {
            version: 1,
            market: Pubkey::new_unique(),
            account_collection: Pubkey::new_unique(),
            account_distribution: Pubkey::new_unique(),
            mint_pool: Pubkey::new_unique(),
            mint_whitelist: MintWhitelist::None(DEFAULT_WHITELIST_KEY),
            price,
            goal_max_collected: goal_max,
            goal_min_collected: 10,
            amount_investment_min: 3,
            amount_investment_max: 30,
            time_start: 10,
            time_finish: 500,
            amount_collected: 10,
            amount_to_distribute: 10,
            owner: Pubkey::new_unique(),
            authority: Pubkey::new_unique(),
            authority_bump_seed: 10,
            kyc_requirement: KycRequirement::NotRequired,
            tier_allocation: [0; TIERS_COUNT],
            time_table: [0; crate::STAGES_ACTIVE_COUNT],
            tier_remaining: [5; TIERS_COUNT],
        };
        pool
    }

    #[test]
    fn pool_math_example() {
        let goal_max = 1_000_000;
        let price = 1_000_000_000;
        let tier_balance = [5000, 9000, 16000, 30000];
        let tier_users = [100, 50, 25, 10];
        let mut pool = pool_new(price, goal_max);
        pool.set_tier_allocations(tier_users, tier_balance).unwrap();
        assert_eq!(pool.tier_allocation[0], 3030);
        assert_eq!(pool.tier_allocation[1], 5454);
        assert_eq!(pool.tier_allocation[2], 9696);
        assert_eq!(pool.tier_allocation[3], 18181);
    }

    #[test]
    fn pool_math_equal() {
        let goal_max = 1_000_000;
        let price = 1_000_000_000;
        let tier_balance = [10, 10, 10, 10];
        let tier_users = [10, 10, 10, 10];
        let mut pool = pool_new(price, goal_max);
        pool.set_tier_allocations(tier_users, tier_balance).unwrap();
        assert_eq!(pool.tier_allocation[0], 25000);
        assert_eq!(pool.tier_allocation[1], 25000);
        assert_eq!(pool.tier_allocation[2], 25000);
        assert_eq!(pool.tier_allocation[3], 25000);
        assert_eq!(pool.tier_remaining[0], 250000);
        assert_eq!(pool.tier_remaining[1], 250000);
        assert_eq!(pool.tier_remaining[2], 250000);
        assert_eq!(pool.tier_remaining[3], 250000);
    }

    #[test]
    fn pool_one() {
        let goal_max = 1_000_000;
        let price = 1_000_000_000;
        let tier_balance = [1000, 2000, 3000, 4000];
        let tier_users = [0, 1, 0, 0];
        let mut pool = pool_new(price, goal_max);
        pool.set_tier_allocations(tier_users, tier_balance).unwrap();
        assert_eq!(pool.tier_allocation[0], 500000);
        assert_eq!(pool.tier_allocation[1], 1000000);
        assert_eq!(pool.tier_allocation[2], 1500000);
        assert_eq!(pool.tier_allocation[3], 2000000);
        assert_eq!(pool.tier_remaining[0], 0);
        assert_eq!(pool.tier_remaining[1], 1000000);
        assert_eq!(pool.tier_remaining[2], 0);
        assert_eq!(pool.tier_remaining[3], 0);
    }

    #[test]
    fn pool_skew() {
        let goal_max = 1_000_000;
        let price = 1_000_000_000;
        let tier_balance = [10, 10, 10, 30];
        let tier_users = [10, 10, 10, 10];
        let mut pool = pool_new(price, goal_max);
        pool.set_tier_allocations(tier_users, tier_balance).unwrap();
        assert_eq!(pool.tier_allocation[0], 16666);
        assert_eq!(pool.tier_allocation[1], 16666);
        assert_eq!(pool.tier_allocation[2], 16666);
        assert_eq!(pool.tier_allocation[3], 3 * 16666 + 2);
        assert_eq!(pool.tier_remaining[0], 16666 * 10);
        assert_eq!(pool.tier_remaining[1], 16666 * 10);
        assert_eq!(pool.tier_remaining[2], 16666 * 10);
        assert_eq!(pool.tier_remaining[3], 500000);

        assert_eq!(
            pool.tier_remaining.iter().sum::<u64>(),
            goal_max * Pool::PRECISION / price - 20
        );
    }

    #[test]
    fn pool_stage_math() {
        let goal_max = 1_000_000;
        let price = 1_000_000_000;
        let pool = Pool {
            time_table: [10, 20],
            ..pool_new(price, goal_max)
        };
        let mut clock = Clock {
            unix_timestamp: pool.time_start,
            ..Clock::default()
        };

        assert_eq!(pool.get_current_stage(&clock).unwrap(), Stage::InitialStage);

        clock.unix_timestamp = 21;
        assert_eq!(
            pool.get_current_stage(&clock).unwrap(),
            Stage::TierAllocationStage
        );

        clock.unix_timestamp = 131;
        assert_eq!(pool.get_current_stage(&clock).unwrap(), Stage::FinalStage);
    }

    #[test]
    fn pool_invest_math() {
        let goal_max = 1_000_000;
        let price = 1_000_000_000;
        let pool = pool_new(price, goal_max);

        assert_eq!(
            pool.stage_investment(10, Stage::InitialStage, [3, 6, 9, 12], 7)
                .unwrap()
                .0,
            6
        );
        assert_eq!(
            pool.stage_investment(10, Stage::TierAllocationStage, [3, 6, 9, 12], 7)
                .unwrap()
                .0,
            5
        );
        assert_eq!(
            pool.stage_investment(10, Stage::FinalStage, [3, 6, 9, 12], 7)
                .unwrap()
                .0,
            10
        );
    }

    #[test]
    fn market() {
        assert_eq!(
            Market::LEN,
            solana_program::borsh::get_packed_len::<Market>()
        );
    }

    #[test]
    fn user() {
        assert_eq!(
            MarketUserKyc::LEN,
            solana_program::borsh::get_packed_len::<MarketUserKyc>()
        );
    }
}
