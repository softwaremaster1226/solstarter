//! Instruction types

use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use solana_program::{clock::UnixTimestamp, system_program};
use solana_program::{
    instruction::AccountMeta, program_error::ProgramError, pubkey::Pubkey, sysvar,
};

use crate::program::PubkeyPatterns;

/// input
#[repr(C)]
#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct InitializePoolInput {
    /// Balances qualifying for different tiers
    pub tier_balance: [u64; crate::TIERS_COUNT],

    /// authority of IDO which controls the pool                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             pool
    pub ido_authority: Pubkey,

    /// Seconds for tokens stake lock
    pub transit_incoming: UnixTimestamp,

    /// Seconds for tokens unstake lock
    pub transit_outgoing: UnixTimestamp,
}

/// input
#[repr(C)]
#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct StakeStartInput {
    /// Amount
    pub amount: u64,
}

/// input
#[repr(C)]
#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct UnstakeStartInput {
    /// amount
    pub amount: u64,
}

/// input
#[repr(C)]
#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct LockInput {
    /// amount
    pub amount: u64,
}

/// input
#[repr(C)]
#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct UnlockInput {
    /// amount
    pub amount: u64,
}

/// input
#[repr(C)]
#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema)]
pub struct StartPoolInput {
    /// active market pool prevents unlock
    pub pool_active_until: UnixTimestamp,
}

/// Splits stake and lock to make xSOS liquid.
/// Forces xSOS token transfers via program authority to track tiers.
#[repr(C)]
#[derive(Debug, BorshDeserialize, BorshSerialize, BorshSchema)]
pub enum Instruction {
    /// Initializes pool with valid mints and token account. Mints and token account created off chain.
    ///
    /// Accounts:
    /// - *write*          `pool`                    uninitialized pool account
    /// - *write*          `pool_token_account_sos`  uninitialized token account to store SOS tokens
    /// - *read*           `mint_sos`                SOS token mint account
    /// - *write*          `pool_mint_xsos`          uninitialized mint account to mint XSOS tokens
    /// - *read, derived*  `pool_authority`          used to `initialize pool_mint_xsos` and `pool_token_account_sos`
    /// - *read, system*   `rent`
    /// - *read*           `token_program`
    ///
    InitializePool(InitializePoolInput),

    /// Transfers token from user account to transit account. Transit is initialized with [crate::state::TransitDirection::Incoming].
    ///
    /// Accounts:
    /// - *read*              `pool`                              initialized pool account, to read transit settings
    /// - *write*             `pool_transit`                      uninitialized transit account
    /// - *read, derived*     `pool_authority`                    used to initialize `pool_transit_token_account_sos`
    /// - *read*              `pool_token_account_sos`            Pool token account sos
    /// - *write*             `pool_transit_token_account_sos`    uninitialized account to store tokens in transit
    /// - *read*              `mint_sos`                          SOS mint account used to initialize pool transit token account
    /// - *read, signer*      `user_wallet`                       owner of `user_token_account_sos`
    /// - *write*             `user_token_account_sos`            source token to transfer from
    /// - *read, system*      `rent`
    /// - *read, system*      `clock`
    /// - *read*              `_token_program`
    ///
    StakeStart(StakeStartInput),

    /// Moves SOS tokens to pool. Mints xSOS tokens into user account if time in transit elapsed   
    /// Allows to transfer amount of tokens linearly proportional to passed time since stake requested till finish.     
    ///
    /// Accounts:
    /// - *read*               `pool`                               initialized pool account
    /// - *read*               `pool_authority`                     to sign cross program invocation into token program
    /// - *write*              `pool_token_account_sos`             account of pool to transfer tokens to
    /// - *read*               `pool_transit`                       initialized transit account
    /// - *write*              `pool_transit_token_account_sos`     account of pool to transfer SOS tokens from
    /// - *write*              `user_token_account_xsos`            account under of user authority    
    /// - *read, signer*       `user_wallet`
    /// - *write*              `pool_mint_xsos`                     used to mint tokens to user
    /// - *read, system*       `clock`        
    /// - *read*               `token_program`
    ///
    StakeFinish,

    /// Moves tokens from [crate::state::StakingPool] into [crate::state::PoolTransit].
    ///
    /// - initialize transit token
    /// - burn xSOS token from user account
    /// - transfer SOS tokens from pool into transit        
    /// - initialize and update transit with timer
    ///    
    /// Accounts:
    /// - *read*                   `pool`                               initialized pool account
    /// - *read, derived*          `pool_authority`                     pool authority account
    /// - *write*                  `pool_token_account_sos`             pool token account to transfer tokens from into transit
    /// - *write, new*             `pool_transit`                       must be uninitialized
    /// - *write, derived*         `pool_transit_token_account_sos`     pool account to transfer SOS tokens from
    /// - *read*                   `mint_sos`            
    /// - *read, signer*           `user_wallet`        
    /// - *write*                  `user_token_account_xsos`            under user authority move sos tokens from
    /// - *write*                  `mint_xsos`                          burn xSOS tokens
    /// - *read, system*           `rent`
    /// - *read, system*           `clock`     
    /// - *read*                   `token_program`
    UnstakeStart(UnstakeStartInput),

    /// Transit SOS tokens to any user owned account if time elapsed.
    /// Allows to transfer amount of tokens linearly proportional to passed time since unstake requested till finish.
    ///
    /// Accounts:
    /// - *read*               `pool`
    /// - *read*               `pool_transit`                        Account with [TransitState]
    /// - *read, derived*      `pool_authority`                      Derived from pool and program_id                       
    /// - *write*              `pool_transit_account_sos`            source    
    /// - *read, signer*       `user_wallet`
    /// - *write*              `user_token_account_sos`              destination   
    /// - *read, system*       `clock`
    /// - *read*               `_token_program`
    UnstakeFinish,

    /// Creates and initializes [crate::state::PoolLock] account.
    ///
    /// Accounts:
    /// - *read*                   `pool`                            initialized pool account
    /// - *read, signer, payer*    `user_wallet`                     Must be used to derive address of `pool_lock`
    /// - *read, derived*          `pool_lock`                       Uninitialized
    /// - *read, derived*          `pool_user_authority`             Authority derived from pool and user  
    /// - *read*                   `pool_mint_xsos`                  Pool mint
    /// - *write*                  `pool_lock_token_account_xsos`    Under pool authority (user can transfer only via this program)
    /// - *read, system*           `rent`                            Used to make sure lock account created rent exempt
    /// - *read, system*           `_system_program`                 Used to create lock account
    /// - *read*                   `_token_program`                  Used to initialize lock token account  
    InitializeLock,

    /// Transfers xSOS from user to lock. Updates tiers in pool.
    ///
    /// Accounts:
    /// - *write*                 `pool`
    /// - *read, signer*          `user_wallet`    
    /// - *read, derived*         `pool_lock`                       Lock account with relevant keys
    /// - *read, derived*         `pool_user_authority`             Authority derived from pool and user
    /// - *write*                 `pool_lock_token_account_xsos`    under pool authority (user can transfer only via this program)
    /// - *write*                 `user_token_account_xsos`         source    
    /// - *read, system*          `clock`                           Used to calculate lock period
    /// - *read*                  `_token_program`    
    Lock(LockInput),

    /// Moves xSOS from lock to user. Updates tiers in pool.
    ///
    /// Accounts:
    /// - *write*              `pool`
    /// - *read, signer*       `user_wallet`                     
    /// - *read, derived*      `pool_lock`                       Lock account with relevant keys
    /// - *read, derived*      `pool_user_authority`             Authority derived from pool and user
    /// - *write*              `pool_lock_token_account_xsos`    source
    /// - *write*              `user_token_account_xsos`         destination
    /// - *read, system*       `clock`                           Unlock period must lapsed
    /// - *read*               `_token_program`    
    Unlock(UnlockInput),

    /// Accounts:
    // - *write*                      `pool`
    // - *read, derived,signer*       `market_authority`  IDO market derived authority (from ido_market and IDO program_id )
    // - *read, system*               `clock`             Pool must be active for some time
    StartPool(StartPoolInput),
}

/// Calculate authority pubkey
pub fn find_key_program_address(owner: &Pubkey) -> Pubkey {
    let (authority, _) = Pubkey::find_key_program_address(owner, &crate::program_id());
    authority
}

/// Calculate authority from 2 pubkeys
pub fn find_2key_program_address(key1: &Pubkey, key2: &Pubkey) -> Pubkey {
    let (authority, _) = Pubkey::find_2key_program_address(key1, key2, &crate::program_id());
    authority
}

/// create instruction
#[allow(clippy::too_many_arguments)]
pub fn initialize_pool(
    pool: &Pubkey,
    token_account_sos: &Pubkey,
    mint_sos: &Pubkey,
    pool_mint_xsos: &Pubkey,
    input: InitializePoolInput,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*pool, false),
        AccountMeta::new(*token_account_sos, false),
        AccountMeta::new_readonly(*mint_sos, false),
        AccountMeta::new(*pool_mint_xsos, false),
        AccountMeta::new_readonly(find_key_program_address(pool), false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    Ok(solana_program::instruction::Instruction::new_with_borsh(
        crate::id(),
        &Instruction::InitializePool(input),
        accounts,
    ))
}

/// create instruction
#[allow(clippy::too_many_arguments)]
pub fn stake_start(
    pool: &Pubkey,
    pool_transit: &Pubkey,
    pool_token_account_sos: &Pubkey,
    pool_transit_token_account_sos: &Pubkey,
    mint_sos: &Pubkey,
    user_wallet: &Pubkey,
    user_token_account_sos: &Pubkey,
    input: StakeStartInput,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*pool, false),
        AccountMeta::new(*pool_transit, false),
        AccountMeta::new_readonly(find_key_program_address(pool), false),
        AccountMeta::new_readonly(*pool_token_account_sos, false),
        AccountMeta::new(*pool_transit_token_account_sos, false),
        AccountMeta::new_readonly(*mint_sos, false),
        AccountMeta::new_readonly(*user_wallet, true),
        AccountMeta::new(*user_token_account_sos, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    Ok(solana_program::instruction::Instruction::new_with_borsh(
        crate::id(),
        &Instruction::StakeStart(input),
        accounts,
    ))
}

/// create instruction
#[allow(clippy::too_many_arguments)]
pub fn stake_finish(
    pool: &Pubkey,
    pool_token_account_sos: &Pubkey,
    pool_transit: &Pubkey,
    pool_transit_token_account_sos: &Pubkey,
    user_token_account_xsos: &Pubkey,
    user_wallet: &Pubkey,
    pool_mint_xsos: &Pubkey,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*pool, false),
        AccountMeta::new_readonly(find_key_program_address(pool), false),
        AccountMeta::new(*pool_token_account_sos, false),
        AccountMeta::new_readonly(*pool_transit, false),
        AccountMeta::new(*pool_transit_token_account_sos, false),
        AccountMeta::new(*user_token_account_xsos, false),
        AccountMeta::new_readonly(*user_wallet, true),
        AccountMeta::new(*pool_mint_xsos, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    Ok(solana_program::instruction::Instruction::new_with_borsh(
        crate::id(),
        &Instruction::StakeFinish,
        accounts,
    ))
}

/// create instruction
#[allow(clippy::too_many_arguments)]
pub fn unstake_start(
    pool: &Pubkey,
    pool_token_account_sos: &Pubkey,
    pool_transit: &Pubkey,
    pool_transit_token_account_sos: &Pubkey,
    mint_sos: &Pubkey,
    user_wallet: &Pubkey,
    user_token_account_xsos: &Pubkey,
    mint_xsos: &Pubkey,
    input: UnstakeStartInput,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*pool, false),
        AccountMeta::new_readonly(find_key_program_address(pool), false),
        AccountMeta::new(*pool_token_account_sos, false),
        AccountMeta::new(*pool_transit, false),
        AccountMeta::new(*pool_transit_token_account_sos, false),
        AccountMeta::new_readonly(*mint_sos, false),
        AccountMeta::new_readonly(*user_wallet, true),
        AccountMeta::new(*user_token_account_xsos, false),
        AccountMeta::new(*mint_xsos, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    Ok(solana_program::instruction::Instruction::new_with_borsh(
        crate::id(),
        &Instruction::UnstakeStart(input),
        accounts,
    ))
}

/// create instruction
pub fn unstake_finish(
    pool: &Pubkey,
    pool_transit: &Pubkey,
    pool_transit_account_sos: &Pubkey,
    user_wallet: &Pubkey,
    user_token_account_sos: &Pubkey,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*pool, false),
        AccountMeta::new_readonly(*pool_transit, false),
        AccountMeta::new_readonly(find_key_program_address(pool), false),
        AccountMeta::new(*pool_transit_account_sos, false),
        AccountMeta::new_readonly(*user_wallet, true),
        AccountMeta::new(*user_token_account_sos, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    Ok(solana_program::instruction::Instruction::new_with_borsh(
        crate::id(),
        &Instruction::UnstakeFinish,
        accounts,
    ))
}

/// create instruction
#[allow(clippy::too_many_arguments)]
pub fn initialize_lock(
    pool: &Pubkey,
    user_wallet: &Pubkey,
    pool_mint_xsos: &Pubkey,
    pool_lock_token_account_xsos: &Pubkey,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let pool_user_authority = find_2key_program_address(pool, user_wallet);
    let pool_lock = Pubkey::create_with_seed(&pool_user_authority, crate::LOCK_SEED, &crate::id())?;
    let accounts = vec![
        AccountMeta::new_readonly(*pool, false),
        AccountMeta::new_readonly(*user_wallet, true),
        AccountMeta::new(pool_lock, false),
        AccountMeta::new_readonly(pool_user_authority, false),
        AccountMeta::new_readonly(*pool_mint_xsos, false),
        AccountMeta::new(*pool_lock_token_account_xsos, false),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    Ok(solana_program::instruction::Instruction::new_with_borsh(
        crate::id(),
        &Instruction::InitializeLock,
        accounts,
    ))
}

/// create
#[allow(clippy::too_many_arguments)]
pub fn lock(
    pool: &Pubkey,
    user_wallet: &Pubkey,
    pool_lock_token_account_xsos: &Pubkey,
    user_token_account_xsos: &Pubkey,
    input: LockInput,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let pool_user_authority = find_2key_program_address(pool, user_wallet);
    let pool_lock = Pubkey::create_with_seed(&pool_user_authority, crate::LOCK_SEED, &crate::id())?;

    let accounts = vec![
        AccountMeta::new(*pool, false),
        AccountMeta::new_readonly(*user_wallet, true),
        AccountMeta::new(pool_lock, false),
        AccountMeta::new_readonly(pool_user_authority, false),
        AccountMeta::new(*pool_lock_token_account_xsos, false),
        AccountMeta::new(*user_token_account_xsos, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    Ok(solana_program::instruction::Instruction::new_with_borsh(
        crate::id(),
        &Instruction::Lock(input),
        accounts,
    ))
}

/// create instruction
#[allow(clippy::too_many_arguments)]
pub fn unlock(
    pool: &Pubkey,
    user_wallet: &Pubkey,
    pool_lock_token_account_xsos: &Pubkey,
    user_token_account_xsos: &Pubkey,
    input: UnlockInput,
) -> Result<solana_program::instruction::Instruction, ProgramError> {
    let pool_user_authority = find_2key_program_address(pool, user_wallet);
    let pool_lock = Pubkey::create_with_seed(&pool_user_authority, crate::LOCK_SEED, &crate::id())?;

    let accounts = vec![
        AccountMeta::new(*pool, false),
        AccountMeta::new_readonly(*user_wallet, true),
        AccountMeta::new(pool_lock, false),
        AccountMeta::new_readonly(pool_user_authority, false),
        AccountMeta::new(*pool_lock_token_account_xsos, false),
        AccountMeta::new(*user_token_account_xsos, false),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    Ok(solana_program::instruction::Instruction::new_with_borsh(
        crate::id(),
        &Instruction::Unlock(input),
        accounts,
    ))
}

/// Creates [Instructions::StartPool]
pub fn start_pool(
    pool: &Pubkey,
    market_authority: &Pubkey,
    input: StartPoolInput,
) -> solana_program::instruction::Instruction {
    let accounts = vec![
        AccountMeta::new(*pool, false),
        AccountMeta::new_readonly(*market_authority, true),
        AccountMeta::new_readonly(sysvar::clock::id(), false),
    ];
    solana_program::instruction::Instruction::new_with_borsh(
        crate::id(),
        &Instruction::StartPool(input),
        accounts,
    )
}
