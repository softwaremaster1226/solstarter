//! Program state processor

use borsh::BorshDeserialize;
use solana_program::{
    account_info::AccountInfo,
    clock::{self, Clock},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::Sysvar,
    sysvar::{self, rent::Rent},
};
use spl_token::state::{Account, Mint};

use crate::{
    borsh::{BorshDeserialiseConst, BorshSerializeConst},
    error::Error,
    instruction::{
        InitializePoolInput, Instruction, LockInput, StakeStartInput, StartPoolInput, UnlockInput,
        UnstakeStartInput,
    },
    invoke::{self},
    math::{self, ErrorAdd},
    program::{
        create_account_with_seed_signed, AccountPatterns, ProgramAccountInfo, ProgramPubkey,
        PubkeyPatterns,
    },
    state::{get_tier, PoolLock, PoolTransit, StakePool, StateVersion, TransitDirection},
};

macro_rules! is_owner {
        (
            $program_id:expr,
            $($account:expr),+
        )
        => {
            {
                $(
                    if *$account.owner != $program_id.pubkey() {
                        return Err(ProgramError::IncorrectProgramId);
                    }
                )+
            }
        }
    }

/// Program state handler.
pub struct Processor {}
impl Processor {
    /// Initialize pool
    #[allow(clippy::too_many_arguments)]
    pub fn initialize_pool<'a>(
        program_id: &ProgramPubkey,
        pool: &AccountInfo<'a>,
        token_account_sos: &AccountInfo<'a>,
        mint_sos: &AccountInfo<'a>,
        pool_mint_xsos: &AccountInfo<'a>,
        program_authority: &AccountInfo<'a>,
        rent: &AccountInfo<'a>,
        _token_program: &AccountInfo<'a>, // Used implicitly
        input: &InitializePoolInput,
    ) -> ProgramResult {
        is_owner!(program_id, pool);
        let (expected_program_authority, _) =
            Pubkey::find_key_program_address(pool.key, program_id);
        if *program_authority.key != expected_program_authority {
            return Err(Error::InvalidAuthority.into());
        }

        let decimals = Mint::unpack_from_slice(&mint_sos.data.borrow())?.decimals;

        invoke::initialize_mint(
            pool_mint_xsos.clone(),
            program_authority.clone(),
            decimals,
            rent.clone(),
        )?;

        invoke::initialize_token_account(
            token_account_sos.clone(),
            mint_sos.clone(),
            program_authority.clone(),
            rent.clone(),
        )?;

        let rent = &Rent::from_account_info(rent)?;

        if !rent.is_exempt(pool.lamports(), pool.data_len()) {
            return Err(ProgramError::AccountNotRentExempt);
        }

        let mut pool_state = StakePool::try_from_slice(&pool.data.borrow())?;

        pool_state.uninitialized()?;
        pool_state.version = StateVersion::V1;
        pool_state.tier_users = [0; crate::TIERS_COUNT];

        pool_state.transit_incoming = input.transit_incoming;
        pool_state.transit_outgoing = input.transit_outgoing;

        pool_state.tier_balance = input.tier_balance;
        pool_state.token_account_sos = *token_account_sos.key;
        pool_state.pool_mint_xsos = *pool_mint_xsos.key;

        pool_state.ido_authority = input.ido_authority;

        pool_state.serialize_const(&mut *pool.try_borrow_mut_data()?)?;

        Ok(())
    }

    /// handler
    #[allow(clippy::too_many_arguments)]
    pub fn stake_start<'a>(
        program_id: &ProgramPubkey,
        pool: &AccountInfo<'a>,
        pool_transit: &AccountInfo<'a>,
        pool_authority: &AccountInfo<'a>,
        pool_token_account_sos: &AccountInfo<'a>,
        pool_transit_token_account_sos: &AccountInfo<'a>,
        mint_sos: &AccountInfo<'a>,
        user_wallet: &AccountInfo<'a>,
        user_token_account_sos: &AccountInfo<'a>,
        rent: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
        _token_program: &AccountInfo<'a>, // Used implicitly
        input: &StakeStartInput,
    ) -> ProgramResult {
        is_owner!(program_id, pool, pool_transit);
        user_wallet.is_signer()?;
        let pool_state = StakePool::try_from_slice(&pool.data.borrow())?;
        pool_state.initialized()?;
        same_key(
            pool_state.token_account_sos,
            pool_token_account_sos,
            Error::WrongAccountSpecified,
        )?;
        let mint_sos_key = Account::unpack_from_slice(&pool_token_account_sos.data.borrow())?.mint;
        if mint_sos_key != mint_sos.pubkey() {
            return Err(Error::WrongAccountSpecified.into());
        }

        let (pool_authority_key, _) = Pubkey::find_key_program_address(pool.key, program_id);

        if *pool_authority.key != pool_authority_key {
            return Err(Error::InvalidAuthority.into());
        }

        invoke::initialize_token_account(
            pool_transit_token_account_sos.clone(),
            mint_sos.clone(),
            pool_authority.clone(),
            rent.clone(),
        )?;

        invoke::token_transfer_with_user_authority(
            user_token_account_sos.clone(),
            pool_transit_token_account_sos.clone(),
            user_wallet.clone(),
            input.amount,
        )?;

        let rent = &Rent::from_account_info(rent)?;

        if !rent.is_exempt(pool_transit.lamports(), pool_transit.data_len()) {
            return Err(ProgramError::AccountNotRentExempt);
        }

        let mut pool_transit_state = PoolTransit::try_from_slice(&pool_transit.data.borrow())?;

        pool_transit_state.uninitialized()?;
        pool_transit_state.version = StateVersion::V1;
        pool_transit_state.direction = TransitDirection::Incoming;
        pool_transit_state.pool = *pool.key;
        pool_transit_state.token_account_sos = *pool_transit_token_account_sos.key;
        pool_transit_state.user_wallet = *user_wallet.key;

        let clock = clock::Clock::from_account_info(clock)?;
        pool_transit_state.transit_from = clock.unix_timestamp;
        pool_transit_state.transit_until = pool_transit_state
            .transit_from
            .error_add(pool_state.transit_incoming)?;

        pool_transit_state.serialize_const(&mut *pool_transit.try_borrow_mut_data()?)?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn stake_finish<'a>(
        program_id: &ProgramPubkey,
        pool: &AccountInfo<'a>,
        pool_authority: &AccountInfo<'a>,
        pool_token_account_sos: &AccountInfo<'a>,
        pool_transit: &AccountInfo<'a>,
        pool_transit_token_account_sos: &AccountInfo<'a>,
        user_token_account_xsos: &AccountInfo<'a>,
        user_wallet: &AccountInfo<'a>,
        pool_mint_xsos: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
        _token_program: &AccountInfo<'a>,
    ) -> ProgramResult {
        is_owner!(program_id, pool, pool_transit);
        user_wallet.is_signer()?;

        let pool_transit_state = PoolTransit::try_from_slice(&pool_transit.data.borrow())?;
        pool_transit_state.initialized()?;

        if pool_transit_state.direction != TransitDirection::Incoming {
            return Err(Error::PoolTransitWrongDirection.into());
        }

        let pool_state = StakePool::try_from_slice(&pool.data.borrow())?;
        same_key(
            pool_state.token_account_sos,
            pool_token_account_sos,
            Error::WrongAccountSpecified,
        )?;

        if pool_mint_xsos.pubkey() != pool_state.pool_mint_xsos {
            return Err(Error::WrongAccountSpecified.into());
        }

        if pool_transit_state.token_account_sos != *pool_transit_token_account_sos.key {
            return Err(ProgramError::InvalidAccountData);
        }

        if pool_transit_state.user_wallet != *user_wallet.key {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let clock = sysvar::clock::Clock::from_account_info(clock)?;
        let remaining_amount =
            Account::unpack_from_slice(&pool_transit_token_account_sos.data.borrow())?.amount;

        let amount_to_claim = finish(pool_transit_state, clock, remaining_amount, pool_transit)?;

        let (_, bump_seed) = Pubkey::find_key_program_address(pool.key, program_id);
        invoke::token_transfer_program_authority(
            pool.key,
            pool_transit_token_account_sos.clone(),
            pool_token_account_sos.clone(),
            pool_authority.clone(),
            bump_seed,
            amount_to_claim,
        )?;

        invoke::token_mint_to(
            pool.key,
            pool_mint_xsos.clone(),
            user_token_account_xsos.clone(),
            pool_authority.clone(),
            bump_seed,
            amount_to_claim,
        )?;

        Ok(())
    }

    /// handler
    #[allow(clippy::too_many_arguments)]
    fn unstake_start<'a>(
        program_id: &ProgramPubkey,
        pool: &AccountInfo<'a>,
        pool_authority: &AccountInfo<'a>,
        pool_token_account_sos: &AccountInfo<'a>,
        pool_transit: &AccountInfo<'a>,
        pool_transit_token_account_sos: &AccountInfo<'a>,
        mint_sos: &AccountInfo<'a>,
        user_wallet: &AccountInfo<'a>,
        user_token_account_xsos: &AccountInfo<'a>,
        mint_xsos: &AccountInfo<'a>,
        rent: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
        _token_program: &AccountInfo<'a>,
        input: &UnstakeStartInput,
    ) -> ProgramResult {
        is_owner!(program_id, pool, pool_transit);
        let pool_state = StakePool::try_from_slice(&pool.data.borrow())?;
        if pool_state.pool_mint_xsos != mint_xsos.pubkey() {
            return Err(Error::WrongAccountSpecified.into());
        }

        same_key(
            pool_state.token_account_sos,
            pool_token_account_sos,
            Error::WrongAccountSpecified,
        )?;

        let mint_sos_key = Account::unpack_from_slice(&pool_token_account_sos.data.borrow())?.mint;

        if mint_sos_key != mint_sos.pubkey() {
            return Err(Error::WrongAccountSpecified.into());
        }

        let clock = sysvar::clock::Clock::from_account_info(clock)?;
        let bump_seed = pool_authority.is_derived(&pool.pubkey(), program_id)?;
        invoke::initialize_token_account(
            pool_transit_token_account_sos.clone(),
            mint_sos.clone(),
            pool_authority.clone(),
            rent.clone(),
        )?;

        invoke::burn_tokens_with_user_authority(
            user_token_account_xsos.clone(),
            mint_xsos.clone(),
            user_wallet.clone(),
            input.amount,
        )?;

        invoke::token_transfer_program_authority(
            pool.key,
            pool_token_account_sos.clone(),
            pool_transit_token_account_sos.clone(),
            pool_authority.clone(),
            bump_seed,
            input.amount,
        )?;

        let mut pool_transit_state = PoolTransit::try_from_slice(&pool_transit.data.borrow())?;
        pool_transit_state.uninitialized()?;
        pool_transit_state.pool = *pool.key;
        pool_transit_state.token_account_sos = *pool_transit_token_account_sos.key;
        pool_transit_state.user_wallet = *user_wallet.key;
        let pool_state = StakePool::try_from_slice(*pool.data.borrow())?;

        pool_transit_state.transit_from = clock.unix_timestamp;
        pool_transit_state.transit_until = pool_transit_state
            .transit_from
            .error_add(pool_state.transit_outgoing)?;

        pool_transit_state.version = StateVersion::V1;
        pool_transit_state.direction = TransitDirection::Outgoing;
        pool_transit_state.serialize_const(&mut *pool_transit.try_borrow_mut_data()?)?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn unstake_finish<'a>(
        program_id: &ProgramPubkey,
        pool: &AccountInfo<'a>,
        pool_transit: &AccountInfo<'a>,
        pool_authority: &AccountInfo<'a>,
        pool_transit_token_account_sos: &AccountInfo<'a>,
        user_wallet: &AccountInfo<'a>,
        user_token_account_sos: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
        _token_program: &AccountInfo<'a>,
    ) -> ProgramResult {
        is_owner!(program_id, pool, pool_transit);
        user_wallet.is_signer()?;

        let clock = sysvar::clock::Clock::from_account_info(clock)?;

        let pool_transit_state = PoolTransit::try_from_slice(&pool_transit.data.borrow())?;

        if pool_transit_state.pool != pool.pubkey() {
            return Err(Error::PoolTransitMustBeOfProvidedPool.into());
        }
        if pool_transit_state.direction != TransitDirection::Outgoing {
            return Err(Error::PoolTransitWrongDirection.into());
        }

        if pool_transit_state.token_account_sos != *pool_transit_token_account_sos.key {
            return Err(ProgramError::InvalidAccountData);
        }

        if pool_transit_state.user_wallet != *user_wallet.key {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let remaining_amount =
            Account::unpack_from_slice(&pool_transit_token_account_sos.data.borrow())?.amount;

        let amount_to_claim = finish(pool_transit_state, clock, remaining_amount, pool_transit)?;

        let (_, bump_seed) = Pubkey::find_key_program_address(pool.key, program_id);

        invoke::token_transfer_program_authority(
            pool.key,
            pool_transit_token_account_sos.clone(),
            user_token_account_sos.clone(),
            pool_authority.clone(),
            bump_seed,
            amount_to_claim,
        )?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn initialize_lock<'b, 'a>(
        program_id: &ProgramPubkey,
        pool: &AccountInfo<'a>,
        user_wallet: &AccountInfo<'a>,
        pool_lock: &AccountInfo<'a>,
        pool_user_authority: &AccountInfo<'a>,
        pool_mint_xsos: &AccountInfo<'a>,
        pool_lock_token_account_xsos: &AccountInfo<'a>,
        rent: &AccountInfo<'a>,
        _system_program: &ProgramAccountInfo<'a, 'b>,
        _token_program: &AccountInfo<'a>,
    ) -> ProgramResult {
        is_owner!(program_id, pool);
        user_wallet.is_signer()?;

        let pool_state = StakePool::try_from_slice(*pool.data.borrow())?;

        let (pool_user_authority_key, bump_seed) =
            Pubkey::find_2key_program_address(pool.key, user_wallet.key, program_id);

        same_key(
            pool_user_authority_key,
            pool_user_authority,
            Error::InvalidAuthority,
        )?;

        if pool_state.pool_mint_xsos != pool_mint_xsos.pubkey() {
            return Err(Error::WrongAccountSpecified.into());
        }

        invoke::initialize_token_account(
            pool_lock_token_account_xsos.clone(),
            pool_mint_xsos.clone(),
            pool_user_authority.clone(),
            rent.clone(),
        )?;

        let pool_lock_key = Pubkey::create_with_seed(
            &pool_user_authority_key,
            crate::LOCK_SEED,
            &program_id.pubkey(),
        )?;

        if pool_lock_key != *pool_lock.key {
            return Err(Error::DerivedPoolLockAccountKeyIsNotEqualToCalculated.into());
        }

        let rent = Rent::from_account_info(rent)?;
        let lamports = rent.minimum_balance(PoolLock::LEN);
        let space = PoolLock::LEN as u64;

        let signature = &[
            &pool.key.to_bytes()[..32],
            &user_wallet.key.to_bytes()[..32],
            &[bump_seed],
        ];

        create_account_with_seed_signed(
            user_wallet,
            pool_lock,
            pool_user_authority,
            crate::LOCK_SEED,
            lamports,
            space,
            program_id,
            signature,
        )?;

        let mut state = PoolLock::try_from_slice(*pool_lock.data.borrow())?;
        state.pool = *pool.key;
        state.version = StateVersion::V1;
        state.token_account_xsos = *pool_lock_token_account_xsos.key;
        state.user_wallet = *user_wallet.key;

        state.serialize_const(&mut *pool_lock.try_borrow_mut_data()?)?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn lock<'a>(
        program_id: &ProgramPubkey,
        pool: &AccountInfo<'a>,
        user_wallet: &AccountInfo<'a>,
        pool_lock: &AccountInfo<'a>,
        pool_user_authority: &AccountInfo<'a>,
        pool_lock_token_account_xsos: &AccountInfo<'a>,
        user_token_account_xsos: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
        _token_program: &AccountInfo<'a>,
        input: &LockInput,
    ) -> ProgramResult {
        is_owner!(program_id, pool, pool_lock);
        let token_state = Account::unpack_from_slice(*pool_lock_token_account_xsos.data.borrow())?;
        let mut pool_state = StakePool::try_from_slice(*pool.data.borrow())?;
        let clock = Clock::from_account_info(&clock)?;

        if clock.unix_timestamp < pool_state.pool_active_until {
            return Err(Error::CannotLockWhenPoolIsActive.into());
        }

        let pool_lock_state = PoolLock::try_from_slice(*pool_lock.data.borrow())?;
        same_key(pool_lock_state.user_wallet, user_wallet, Error::WrongOwner)?;
        same_key(pool_lock_state.pool, pool, Error::LockMustBeRelatedToPool)?;

        let pool_lock_key = Pubkey::create_with_seed(
            &pool_user_authority.key,
            crate::LOCK_SEED,
            &program_id.pubkey(),
        )?;

        same_key(
            pool_lock_key,
            pool_lock,
            Error::DerivedPoolLockAccountKeyIsNotEqualToCalculated,
        )?;

        if *pool_lock_token_account_xsos.key != pool_lock_state.token_account_xsos {
            return Err(ProgramError::InvalidAccountData);
        }

        let old_tier = get_tier(pool_state.tier_balance, token_state.amount);
        let new_value = token_state.amount.error_add(input.amount)?;
        let new_tier = get_tier(pool_state.tier_balance, new_value);
        if let Some(new_tier) = new_tier {
            if let Some(old_tier) = old_tier {
                pool_state.tier_users[old_tier] =
                    pool_state.tier_users[old_tier].error_decrement()?;
            }

            pool_state.tier_users[new_tier] = pool_state.tier_users[new_tier].error_increment()?;
        }

        invoke::token_transfer_with_user_authority(
            user_token_account_xsos.clone(),
            pool_lock_token_account_xsos.clone(),
            user_wallet.clone(),
            input.amount,
        )?;

        pool_state
            .serialize_const(&mut *pool.try_borrow_mut_data().unwrap())
            .unwrap();

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn unlock<'a>(
        program_id: &ProgramPubkey,
        pool: &AccountInfo<'a>,
        user_wallet: &AccountInfo<'a>,
        pool_lock: &AccountInfo<'a>,
        pool_user_authority: &AccountInfo<'a>,
        pool_lock_token_account_xsos: &AccountInfo<'a>,
        user_token_account_xsos: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
        _token_program: &AccountInfo<'a>,
        input: &UnlockInput,
    ) -> ProgramResult {
        is_owner!(program_id, pool, pool_lock);
        user_wallet.is_signer()?;

        let token_state = Account::unpack_from_slice(*pool_lock_token_account_xsos.data.borrow())?;
        let clock = Clock::from_account_info(&clock)?;
        let mut pool_state = StakePool::try_from_slice(*pool.data.borrow())?;

        if clock.unix_timestamp < pool_state.pool_active_until {
            return Err(Error::CannotUnlockWhenPoolIsActive.into());
        }

        let pool_lock_state = PoolLock::try_from_slice(*pool_lock.data.borrow())?;
        same_key(pool_lock_state.user_wallet, user_wallet, Error::WrongOwner)?;
        same_key(pool_lock_state.pool, pool, Error::LockMustBeRelatedToPool)?;

        let pool_lock_key = Pubkey::create_with_seed(
            &pool_user_authority.key,
            crate::LOCK_SEED,
            &program_id.pubkey(),
        )?;

        same_key(
            pool_lock_key,
            pool_lock,
            Error::DerivedPoolLockAccountKeyIsNotEqualToCalculated,
        )?;

        if *pool_lock_token_account_xsos.key != pool_lock_state.token_account_xsos {
            return Err(ProgramError::InvalidAccountData);
        }

        let old_tier = get_tier(pool_state.tier_balance, token_state.amount);

        if let Some(old_tier) = old_tier {
            pool_state.tier_users[old_tier] = pool_state.tier_users[old_tier].error_decrement()?;
        }

        let new_value = token_state.amount.error_sub(input.amount)?;

        let new_tier = get_tier(pool_state.tier_balance, new_value);

        if let Some(new_tier) = new_tier {
            pool_state.tier_balance[new_tier] =
                pool_state.tier_balance[new_tier].error_increment()?;
        }

        let (_, bump_seed) =
            Pubkey::find_2key_program_address(pool.key, user_wallet.key, program_id);

        let signature = &[
            &pool.key.to_bytes()[..32],
            &user_wallet.key.to_bytes()[..32],
            &[bump_seed],
        ];

        invoke::token_transfer_signature(
            pool_lock_token_account_xsos.clone(),
            user_token_account_xsos.clone(),
            pool_user_authority.clone(),
            signature,
            input.amount,
        )?;

        pool_state.serialize_const(&mut *pool.try_borrow_mut_data()?)?;

        Ok(())
    }

    fn start_pool<'a>(
        program_id: &ProgramPubkey,
        pool: &AccountInfo<'a>,
        market_authority: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
        input: &StartPoolInput,
    ) -> ProgramResult {
        is_owner!(program_id, pool);
        market_authority.is_signer()?;
        let mut pool_state = StakePool::try_from_slice(&pool.data.borrow())?;
        let clock = clock::Clock::from_account_info(clock)?;

        if market_authority.pubkey() != pool_state.ido_authority {
            return Err(Error::PoolMustBeRelatedToMarket.into());
        }

        if clock.unix_timestamp > input.pool_active_until {
            return Err(Error::PoolMustBeActiveForSomeTime.into());
        }

        pool_state.pool_active_until = input.pool_active_until;

        pool_state.serialize_const(&mut pool.data.borrow_mut())?;

        Ok(())
    }

    /// Processes an instruction
    pub fn process_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        input: &[u8],
    ) -> ProgramResult {
        let program_id = ProgramPubkey(*program_id);
        let instruction = Instruction::deserialize_const(input)?;
        match instruction {
            Instruction::InitializePool(input) => {
                msg!("Instruction::InitializePool");
                match accounts {
                    [pool, token_account_sos, mint_sos, pool_mint_xsos, program_authority, rent, token_program, ..] => {
                        Self::initialize_pool(
                            &program_id,
                            pool,
                            token_account_sos,
                            mint_sos,
                            pool_mint_xsos,
                            program_authority,
                            rent,
                            token_program,
                            &input,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::StakeStart(input) => {
                msg!("Instruction::StakeStart");
                match accounts {
                    [pool, pool_transit, pool_authority, pool_token_account_sos, pool_transit_token_account_sos, mint_sos, user_wallet, user_token_account_sos, rent, clock, token_program, ..] => {
                        Self::stake_start(
                            &program_id,
                            pool,
                            pool_transit,
                            pool_authority,
                            pool_token_account_sos,
                            pool_transit_token_account_sos,
                            mint_sos,
                            user_wallet,
                            user_token_account_sos,
                            rent,
                            clock,
                            token_program,
                            &input,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::StakeFinish => {
                msg!("Instruction::StakeFinish");
                match accounts {
                    [pool, pool_authority, pool_token_account_sos, pool_transit, pool_transit_token_account_sos, user_token_account_xsos, user_wallet, pool_mint_xsos, clock, token_program, ..] => {
                        Self::stake_finish(
                            &program_id,
                            pool,
                            pool_authority,
                            pool_token_account_sos,
                            pool_transit,
                            pool_transit_token_account_sos,
                            user_token_account_xsos,
                            user_wallet,
                            pool_mint_xsos,
                            clock,
                            token_program,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::UnstakeStart(input) => {
                msg!("Instruction::UnstakeStart");
                match accounts {
                    [pool, pool_authority, pool_token_account_sos, pool_transit, pool_transit_token_account_sos, mint_sos, user_wallet, user_token_account_xsos, mint_xsos, rent, clock, token_program, ..] => {
                        Self::unstake_start(
                            &program_id,
                            pool,
                            pool_authority,
                            pool_token_account_sos,
                            pool_transit,
                            pool_transit_token_account_sos,
                            mint_sos,
                            user_wallet,
                            user_token_account_xsos,
                            mint_xsos,
                            rent,
                            clock,
                            token_program,
                            &input,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::UnstakeFinish => {
                msg!("Instruction::UnstakeFinish");
                match accounts {
                    [pool, pool_transit, pool_authority, pool_transit_account_sos, user_wallet, user_token_account_sos, clock, token_program, ..] => {
                        Self::unstake_finish(
                            &program_id,
                            pool,
                            pool_transit,
                            pool_authority,
                            pool_transit_account_sos,
                            user_wallet,
                            user_token_account_sos,
                            clock,
                            token_program,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::InitializeLock => {
                msg!("Instruction::InitializeLock");
                match accounts {
                    [pool, user_wallet, pool_lock, pool_user_authority, pool_mint_xsos, pool_lock_token_account_xsos, rent, _system_program, _token_program] => {
                        Self::initialize_lock(
                            &program_id,
                            pool,
                            user_wallet,
                            pool_lock,
                            pool_user_authority,
                            pool_mint_xsos,
                            pool_lock_token_account_xsos,
                            rent,
                            &ProgramAccountInfo(_system_program),
                            _token_program,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::Lock(input) => {
                msg!("Instruction::Lock");
                match accounts {
                    [pool, user_wallet, pool_lock, pool_user_authority, pool_lock_token_account_xsos, user_token_account_xsos, clock, token_program, ..] => {
                        Self::lock(
                            &program_id,
                            pool,
                            user_wallet,
                            pool_lock,
                            pool_user_authority,
                            pool_lock_token_account_xsos,
                            user_token_account_xsos,
                            clock,
                            token_program,
                            &input,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::Unlock(input) => {
                msg!("Instruction::Unlock");
                match accounts {
                    [pool, user_wallet, pool_lock, pool_user_authority, pool_lock_token_account_xsos, user_token_account_xsos, clock, token_program, ..] => {
                        Self::unlock(
                            &program_id,
                            pool,
                            user_wallet,
                            pool_lock,
                            pool_user_authority,
                            pool_lock_token_account_xsos,
                            user_token_account_xsos,
                            clock,
                            token_program,
                            &input,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::StartPool(input) => {
                msg!("Instruction::StartPool");
                match accounts {
                    [pool, market_authority, clock, ..] => {
                        Self::start_pool(&program_id, pool, market_authority, clock, &input)
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
        }
    }
}

/// errors if relation is not expected
fn same_key(relation: Pubkey, related: &AccountInfo, error: Error) -> ProgramResult {
    if relation != related.pubkey() {
        return Err(error.into());
    }

    Ok(())
}

/// finishes some or whole of stake to or from pool
fn finish(
    mut pool_transit_state: PoolTransit,
    clock: clock::Clock,
    remaining_amount: u64,
    pool_transit: &AccountInfo,
) -> Result<u64, ProgramError> {
    let amount_claimed = pool_transit_state.amount_claimed;
    let transit_from = pool_transit_state.transit_from;
    let transit_until = pool_transit_state.transit_until;
    let now = clock.unix_timestamp;
    let amount_to_claim = math::finish(
        transit_from,
        now,
        transit_until,
        amount_claimed,
        remaining_amount,
    )
    .ok_or(Error::CannotTransitAnythingNow)?;
    pool_transit_state.amount_claimed = pool_transit_state
        .amount_claimed
        .error_add(amount_to_claim)?;
    pool_transit_state.serialize_const(&mut *pool_transit.try_borrow_mut_data()?)?;
    Ok(amount_to_claim)
}
