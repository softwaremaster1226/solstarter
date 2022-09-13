//! Instruction types

use sol_starter_staking::utils::program::{ProgramPubkey, PubkeyPatterns};

use crate::{
    error::Error,
    state::{KycRequirement, UnixTimeSmallDuration},
    CollectionToken,
};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    clock::{Clock, UnixTimestamp},
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction as SolanaInstruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program, sysvar,
};
/// Init pool instruction parameters
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct InitializePool {
    /// pool owner
    pub pool_owner: Pubkey,
    /// The price for the distributed token in collected tokens `price * account_collection.amount / 1B = account_distribution.amount`
    pub price: u64,
    /// Maximum amount of [crate::state::Pool::account_collection] to be collected
    pub goal_max: u64,
    /// Minimum amount of [crate::state::Pool::account_collection] to be collected. If the collected amount is less than `goal_min` the pool should refund all the collected tokens.
    pub goal_min: u64,
    /// The minimum  amount of one single investment transaction.
    pub amount_min: u64,
    /// The maximum  amount of one single investment transaction.
    pub amount_max: u64,
    /// Time when the pool starts accepting investments into [crate::state::Pool::account_collection]
    pub time_start: UnixTimestamp,
    /// Time when the pool stops accepting investments (and starts token distribution by allowing claiming purchased account_distribution tokens).
    pub time_finish: UnixTimestamp,
    /// KYC requirement
    pub kyc_requirement: KycRequirement,
    /// stages non overlapped time
    pub time_table: [UnixTimeSmallDuration; crate::STAGES_ACTIVE_COUNT],
}

impl InitializePool {
    /// validates
    pub fn validate(&self, clock: &Clock) -> ProgramResult {
        if self.goal_min == 0 || self.goal_max == 0 || self.goal_min > self.goal_max {
            return Err(Error::InvalidGoalNumbers.into());
        }

        if self.amount_min == 0 || self.amount_max == 0 || self.amount_min > self.amount_max {
            return Err(Error::InvalidGoalNumbers.into());
        }

        if self.time_start < clock.unix_timestamp
            || self.time_finish < clock.unix_timestamp
            || self.time_start > self.time_finish
        {
            return Err(Error::InvalidPoolTimeFrame.into());
        }

        if self.time_table.iter().sum::<u32>() as i64 > self.time_finish - self.time_start {
            return Err(Error::InvalidTimeTable.into());
        }

        Ok(())
    }
}

/// input
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct Participate {
    /// value holding the amount of collected tokens to transfer to the pool
    pub amount: CollectionToken,
}

/// input
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct InitializeMarket {
    /// reference to stake pool
    pub stake_pool: Pubkey,
}

/// Instruction definition
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub enum Instruction {
    /// Initializes new market account and sets up its owner account.
    ///
    /// Accounts:
    ///  - *write*         `market`         Market account to initialize
    ///  - *read, signer*  `market_owner`   Account which will be the owner of this market and will sign initialization transactions for individual pools
    ///  - *system*        `rent`           Check if market account has enough SOL on it to be rent-exempt
    InitializeMarket(InitializeMarket),

    /// Initializes new pool, attaches it to the market, creates all necessary accounts.
    /// Creates all the required accounts, you just need to supply derived account public keys as parameters seed by `market` `pool` and .
    ///
    /// Accounts:
    /// - *read*                    `market`                  Market account this pool will belong to
    /// - *write*                   `pool`                    New pool account to initialize
    /// - *read, signer, payer*     `market_owner`            Market owner account, has to sign this transaction. Also pays for accounts creation
    /// - *read*                    `mint_collection`         Mint for the tokens to be collected into the pool (users will be paying with these tokens)
    /// - *read*                    `mint_distribution`       Mint for the tokens distributed by the pool (the one sold through the pool)
    /// - *write, derived*          `account_collection`      Account to store collected tokens, should be a program account, will be created by the program
    /// - *write, derived*          `account_distribution`    Account to store distributed tokens, should be a program account, will be created by the program
    /// - *write, derived*          `mint_pool`               Account for the pool mint, should be a program account, will be created by the program
    /// - *read*                    `pool_authority`          Pool authority account, will be the owner of all new accounts
    /// - *read, system*            `rent`                    System Rent account, used to verify rent balances for all the accounts involved
    /// - *read, system*            `clock`                   System Clock account, used to verify pool start and finish time
    /// - *read*                    `_token_program`          Used to call token program for token account and mint initialization
    /// - *read, system*            `_system_program`         Used to create accounts.
    /// - *write, option, derived*  `mint_whitelist`          Account for the pool whitelist mint, should be a program account, will be created by the program                
    InitializePool(InitializePool),

    /// Issued by the user participating in the pool tokensale. Only allowed for the pool after their start time, but before the finish time.
    ///
    /// Accounts:
    ///                             
    // - *read*             `market`
    // - *write*            `pool`                            Initialized and currently active pool account
    // - *read*             `pool_authority`                  Pool authority account
    // - *read*             `pool_user_authority`             Pool/user authority account
    // - *write, signer*    `user_wallet`                     Single-use authority which can spend tokens from the `user_account_from`, identifies KYC record owner if needed
    // - *write*            `user_account_from`               Account sending collected token from the user to the pool, you should approve spending on this account by the transaction signer before issuing this instruction
    // - *write*            `account_collection`              Receives collected tokens, should be pool's collected token's account
    // - *write*            `user_account_to`                 Token account to receive back pool tokens (which can be later exchanged for the distributed tokens)
    // - *read*             `pool_lock_account`               Token account with `user_wallet` owner
    // - *write*            `mint_pool`                       Pool mint account, will mint new tokens to the previous account
    ///- *read, derived*    `market_user_kyc`                 If pool is [KycRequirement::NotRequired] than this MUST be account holding [crate::state::MarketUserKyc], else it should be `user_wallet`
    ///- *read*             `pool_lock`                       [staking::state::PoolLock] owned `user_wallet`
    ///- *read*             `stake_pool`                      [staking::state::StakePool] aligned to `market`
    ///- *write, derived*   `user_pool_stage`                 Marker account forcing one time participation of `user_wallet` per stage
    // - *read*             `_token_program_id`               Used to call transfer and mint for the collected and pool tokens
    // - *read, system*     `_system_program`                 Used to initialize accounts
    // - *read, system*     `rent`                            Used to check if pool is currently active
    // - *read, system*     `clock`                           Used to check if pool is currently active
    // - *write, option*    `account_whitelist`               Token account holding whitelist tokens, if the pool is whitelist-only a single token will be burned by this instruction. You need to issue approval for the signing authority to burn this 1 token
    // - *write, option*    `account_mint_whitelist`          Again, only for whitelist pools, the mint which will be burning user's whitelist tokens (the same as the pool's whitelist mint)
    Participate(Participate),

    /// Claims purchased distribution tokens after the pool finish time (if [crate::state::Pool::goal_min] is reached) or refunds collected tokens (if not).
    ///
    /// Accounts:           
    ///                        
    /// - *read*            `market`                    
    /// - *read*            `pool`                  Finished pool account to collect funds from
    /// - *read*            `pool_authority`        Pool authority, used to control pool token accounts and mints
    /// - *write*           `account_from`          User token account holding pool tokens (received after pool participation), will be burned by this action
    /// - *read, signer*    `user_authority`        Single-use user authority approved for burning tokens from the previous account
    /// - *write*           `mint_pool`             Pool mint which will be burning pool tokens
    /// - *write*           `account_pool`          Pool token account to claim funds from. If the pool was successful then it is the distribution account. Otherwise collection pool account needs to be specified to refund tokens to the user
    /// - *write*           `account_to`            User account to receive claimed tokens (just as with the previous account can either be collected or distributed token account)    
    /// - *read*            `_token_program_id`     used for burning pool tokens and transfers
    /// - *read, system*    `clock`                 used to check if the pool is finished collecting funds    
    Claim,

    /// Called by the pool owner before the pool starts to add particular users to the pool whitelist.
    ///    
    /// - *read*             `pool`                 Pool account
    /// - *read, signer*     `pool_authority`       Pool authority account controlling whitelist mint account
    /// - *write*            `pool_owner`           Pool owner account, should sign this instruction
    /// - *write*            `account_whitelist`    User account to receive a new minted whitelist token
    /// - *write*            `mint_whitelist`       Pool whitelist mint account, which will mint the whitelist token to the account above
    /// - *read*            `_token_program_id`     used for burning pool tokens and transfers    
    AddToWhitelist,

    /// Called by the pool owner after the pool is over to collect the user investments (in collected tokens) and leftover distributed tokens.
    /// Or if the pool failed to reach its [crate::state::Pool::goal_min] returns all of the distribution tokens.
    ///
    /// Accounts:
    ///
    /// - *read*           `market`
    /// - *read*           `pool`             Pool account after the sale is over
    /// - *read*           `pool_authority`   Authority
    /// - *read, signer*   `pool_owner`       Pool owner account, should sign this instruction
    /// - *write*          `account_from`     Account to collect funds from. Should be pool's collection or distribution token account
    /// - *write*          `account_to`       Pool owner's token account to receive tokens from the previous account (either collected or distributed token)
    /// - *read*           `_token_program`   Used to transfer tokens
    /// - *read, system*   `clock`            used to check if pool sale is over
    Withdraw,

    ///  Creates new account to store market user KYC data
    ///
    /// Accounts:
    /// - *read*                   `market`                Market for which KYC(validated credentials) are actual.
    /// - *read*                   `market_user_authority` Program address from `market` and 'user_wallet'
    /// - *write, derived*         `market_user_kyc`       From market authority as the base and `user_wallet` as the key)
    /// - *read, signer, payer*    `market_owner`          Market owner
    /// - *read*                   `user_wallet`           User wallet
    /// - *read, system*           `rent`                  New account will be rent exempt
    /// - *read, system*           `clock`                 Must provide KYC which actual for some time
    /// - *read, system*           `_system_program`       Implicitly used to create account
    CreateMarketUserKyc(CreateMarketUserKyc),

    /// Transfers all SOLs from `user_kyc` to `market_owner` so account is deleted.
    ///
    /// Accounts:
    /// - *read*                   `market`
    /// - *read*                   `market_user_authority`  Derived from `market` and 'user_wallet'
    /// - *write, derived*         `market_user_kyc`        Account to burn       
    /// - *read, signer*           `market_owner`           Owner of `market`
    /// - *read*                   `user_wallet`            Related KYC related `user_wallet`
    /// - *read, system*           `_system_program`                 
    DeleteMarketUserKyc,

    /// Starts pool.
    ///
    /// Accounts:
    /// - *read*            `market`                    Market to start pool at
    /// - *read, signer*    `market_or_pool_owner`      Either one of two are allowed to start pool
    /// - *write*           `stake_pool`                Stake pool used for IDO
    /// - *read, derived*   `market_authority`          Used to sign start stake pull CPI, derived from `market`
    /// - *write*           `pool`                      Pool to start.
    /// - *read, system*    `clock`                     Used to check time start and  finish
    /// - *read*            `_staking_program`          Implicitly used for CPI
    StartPool,
}

/// instruction input
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct CreateMarketUserKyc {
    /// expiration of wallet
    pub expiration: UnixTimestamp,
}

/// Create `InitializeMarket` instruction
pub fn initialize_market(
    program_id: &ProgramPubkey,
    market: &Pubkey,
    market_owner: &Pubkey,
    input: InitializeMarket,
) -> Result<SolanaInstruction, ProgramError> {
    let data = Instruction::InitializeMarket(input);

    let accounts = vec![
        AccountMeta::new(*market, false),
        AccountMeta::new_readonly(*market_owner, true),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];

    Ok(SolanaInstruction::new_with_borsh(
        program_id.pubkey(),
        &data,
        accounts,
    ))
}

/// Create `InitializePool` instruction
#[allow(clippy::too_many_arguments)]
pub fn initialize_pool(
    program_id: &ProgramPubkey,
    pool: &Pubkey,
    market: &Pubkey,
    market_owner: &Pubkey,
    mint_collection: &Pubkey,
    mint_distribution: &Pubkey,
    account_collection: &Pubkey,
    account_distribution: &Pubkey,
    mint_pool: &Pubkey,
    mint_whitelist: Option<Pubkey>,
    input: InitializePool,
) -> Result<SolanaInstruction, ProgramError> {
    let data = Instruction::InitializePool(input);

    let (pool_authority, _) = Pubkey::find_key_program_address(pool, program_id);

    let mut accounts = vec![
        AccountMeta::new_readonly(*market, false),
        AccountMeta::new(*pool, false),
        AccountMeta::new_readonly(*market_owner, true),
        AccountMeta::new_readonly(*mint_collection, false),
        AccountMeta::new_readonly(*mint_distribution, false),
        AccountMeta::new(*account_collection, false),
        AccountMeta::new(*account_distribution, false),
        AccountMeta::new(*mint_pool, false),
        AccountMeta::new_readonly(pool_authority, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    if let Some(mint_whitelist) = mint_whitelist {
        accounts.push(AccountMeta::new(mint_whitelist, false));
    }

    Ok(SolanaInstruction::new_with_borsh(
        program_id.pubkey(),
        &data,
        accounts,
    ))
}

/// Create `Participate` instruction
#[allow(clippy::too_many_arguments)]
pub fn participate(
    program_id: &ProgramPubkey,
    pool: &Pubkey,
    market: &Pubkey,
    user_wallet: &Pubkey,
    user_account_from: &Pubkey,
    account_collection: &Pubkey,
    user_account_to: &Pubkey,
    pool_lock_account: &Pubkey,
    mint_pool: &Pubkey,
    pool_lock: &Pubkey,
    stake_pool: &Pubkey,
    market_user_kyc: Option<&Pubkey>,
    account_whitelist: Option<&Pubkey>,
    mint_whitelist: Option<&Pubkey>,
    input: Participate,
    stage: u8,
) -> Result<SolanaInstruction, ProgramError> {
    let data = Instruction::Participate(input);

    let (pool_authority, _) = Pubkey::find_key_program_address(pool, program_id);

    let (pool_user_authority, _) = Pubkey::find_2key_program_address(pool, user_wallet, program_id);

    let user_pool_stage = Pubkey::create_with_seed(&pool_user_authority,
        format!("{}", stage).as_str(),
        &program_id.pubkey(),
    )?;

    let market_user_kyc_or_user_wallet = market_user_kyc.unwrap_or(user_wallet);

    let mut accounts = vec![
        AccountMeta::new_readonly(*market, false),
        AccountMeta::new(*pool, false),
        AccountMeta::new_readonly(pool_authority, false),
        AccountMeta::new_readonly(pool_user_authority, false),
        AccountMeta::new(*user_wallet, true),
        AccountMeta::new(*user_account_from, false),
        AccountMeta::new(*account_collection, false),
        AccountMeta::new(*user_account_to, false),
        AccountMeta::new_readonly(*pool_lock_account, false),
        AccountMeta::new(*mint_pool, false),
        AccountMeta::new_readonly(*market_user_kyc_or_user_wallet, false),
        AccountMeta::new(user_pool_stage, false),
        AccountMeta::new_readonly(*pool_lock, false),
        AccountMeta::new_readonly(*stake_pool, false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
    ];

    if let Some(account_whitelist) = account_whitelist {
        accounts.push(AccountMeta::new(*account_whitelist, false));
    }

    if let Some(mint_whitelist) = mint_whitelist {
        accounts.push(AccountMeta::new(*mint_whitelist, false))
    }

    Ok(SolanaInstruction::new_with_borsh(
        program_id.pubkey(),
        &data,
        accounts,
    ))
}

/// Create `Claim` instruction
#[allow(clippy::too_many_arguments)]
pub fn claim(
    program_id: &ProgramPubkey,
    pool: &Pubkey,
    market: &Pubkey,
    account_from: &Pubkey,
    user_authority: &Pubkey,
    mint_pool: &Pubkey,
    account_pool: &Pubkey,
    account_to: &Pubkey,
) -> Result<SolanaInstruction, ProgramError> {
    let (pool_authority, _) = Pubkey::find_key_program_address(pool, program_id);

    let accounts = vec![
        AccountMeta::new_readonly(*market, false),
        AccountMeta::new_readonly(*pool, false),
        AccountMeta::new_readonly(pool_authority, false),
        AccountMeta::new(*account_from, false),
        AccountMeta::new_readonly(*user_authority, true),
        AccountMeta::new(*mint_pool, false),
        AccountMeta::new(*account_pool, false),
        AccountMeta::new(*account_to, false),
        AccountMeta::new(spl_token::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
    ];
    Ok(SolanaInstruction::new_with_borsh(
        program_id.pubkey(),
        &Instruction::Claim,
        accounts,
    ))
}

/// Create `AddToWhitelist` instruction
pub fn add_to_whitelist(
    program_id: &ProgramPubkey,
    pool: &Pubkey,
    pool_owner: &Pubkey,
    account_whitelist: &Pubkey,
    mint_whitelist: &Pubkey,
) -> Result<SolanaInstruction, ProgramError> {
    let input = Instruction::AddToWhitelist;

    let (pool_authority, _) = Pubkey::find_key_program_address(pool, program_id);

    let accounts = vec![
        AccountMeta::new_readonly(*pool, false),
        AccountMeta::new_readonly(pool_authority, false),
        AccountMeta::new_readonly(*pool_owner, true),
        AccountMeta::new(*account_whitelist, false),
        AccountMeta::new(*mint_whitelist, false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
    ];
    Ok(SolanaInstruction::new_with_borsh(
        program_id.pubkey(),
        &input,
        accounts,
    ))
}

/// Create `Withdraw` instruction
pub fn withdraw(
    program_id: &ProgramPubkey,
    pool: &Pubkey,
    market: &Pubkey,
    pool_owner: &Pubkey,
    account_from: &Pubkey,
    account_to: &Pubkey,
) -> Result<SolanaInstruction, ProgramError> {
    let init_data = Instruction::Withdraw;
    let data = init_data
        .try_to_vec()
        .or(Err(ProgramError::InvalidArgument))?;

    let (pool_authority, _) = Pubkey::find_key_program_address(pool, program_id);

    let accounts = vec![
        AccountMeta::new_readonly(*market, false),
        AccountMeta::new_readonly(*pool, false),
        AccountMeta::new_readonly(pool_authority, false),
        AccountMeta::new_readonly(*pool_owner, true),
        AccountMeta::new(*account_from, false),
        AccountMeta::new(*account_to, false),
        AccountMeta::new(spl_token::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
    ];
    Ok(SolanaInstruction {
        program_id: program_id.pubkey(),
        accounts,
        data,
    })
}

/// Create [CreateMarketUserKyc] instruction
pub fn create_market_user_kyc(
    market: &Pubkey,
    market_owner: &Pubkey,
    user_wallet: &Pubkey,
    input: CreateMarketUserKyc,
) -> Result<SolanaInstruction, ProgramError> {
    let (market_user_authority_key, _) =
        Pubkey::find_2key_program_address(&market, &user_wallet, &crate::program_id());
    let market_user_kyc =
        Pubkey::create_with_seed(&market_user_authority_key, crate::KYC_SEED, &crate::id())?;

    let accounts = vec![
        AccountMeta::new_readonly(*market, false),
        AccountMeta::new_readonly(market_user_authority_key, false),
        AccountMeta::new(market_user_kyc, false),
        AccountMeta::new_readonly(*market_owner, true),
        AccountMeta::new_readonly(*user_wallet, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
Ok(
    SolanaInstruction::new_with_borsh(
        crate::program_id().pubkey(),
        &Instruction::CreateMarketUserKyc(input),
        accounts,
    ))
}

/// Create [DeleteMarketUserKyc] instruction
pub fn delete_market_user_kyc(
    program_id: &ProgramPubkey,
    market: &Pubkey,
    market_owner: &Pubkey,
    user_wallet: &Pubkey,
) -> Result<SolanaInstruction, ProgramError> {
    let (market_user_authority_key, _) =
        Pubkey::find_2key_program_address(&market, &user_wallet, &crate::program_id());

    let market_user_kyc =
        Pubkey::create_with_seed(&market_user_authority_key, crate::KYC_SEED, &crate::id())?;

    let accounts = vec![
        AccountMeta::new_readonly(*market, false),
        AccountMeta::new_readonly(market_user_authority_key, false),
        AccountMeta::new(market_user_kyc, false),
        AccountMeta::new_readonly(*market_owner, true),
        AccountMeta::new_readonly(*user_wallet, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    Ok(SolanaInstruction::new_with_borsh(
        program_id.pubkey(),
        &Instruction::DeleteMarketUserKyc,
        accounts,
    ))
}

/// Create [StartPool] instruction
pub fn start_pool(
    program_id: &ProgramPubkey,
    market_or_pool_owner: &Pubkey,
    stake_pool: &Pubkey,
    market: &Pubkey,
    pool: &Pubkey,
) -> Result<SolanaInstruction, ProgramError> {
    let market_authority = Pubkey::find_key_program_address(market, &crate::program_id()).0;
    let accounts = vec![
        AccountMeta::new_readonly(*market, false),
        AccountMeta::new_readonly(*market_or_pool_owner, true),
        AccountMeta::new(*stake_pool, false),
        AccountMeta::new(market_authority, false),
        AccountMeta::new(*pool, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(sol_starter_staking::id(), false),
    ];
    Ok(SolanaInstruction::new_with_borsh(
        program_id.pubkey(),
        &Instruction::StartPool,
        accounts,
    ))
}
