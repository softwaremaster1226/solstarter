//! Program state processor

use crate::{
    error::Error,
    instruction::{
        CreateMarketUserKyc, InitializeMarket, InitializePool, Instruction, Participate,
    },
    state::*,
    utils::{invoke::*, math::*, program::AccountPatterns},
};
use borsh::{BorshDeserialize, BorshSerialize};
use num_traits::ToPrimitive;
use sol_starter_staking::{
    instruction::StartPoolInput,
    program::{
        create_account_with_seed_signed, ProgramPubkey,
        PubkeyPatterns,
    },
    state::{PoolLock, StakePool},
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    system_instruction::SystemError,
    sysvar::clock::Clock,
    sysvar::rent::Rent,
    sysvar::Sysvar,
};
use spl_token::state::{Account, Mint};

/// checks that program is owner of system account
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
    /// Initialize market
    pub fn initialize_market(
        program_id: &ProgramPubkey,
        market: &AccountInfo,
        market_owner: &AccountInfo,
        rent: &AccountInfo,
        input: &InitializeMarket,
    ) -> ProgramResult {
        is_owner!(&program_id, market);
        let rent = &Rent::from_account_info(rent)?;
        let mut market_state = Market::try_from_slice(&market.data.borrow()).unwrap();
        market_state.uninitialized()?;
        market_owner.is_signer()?;
        if !rent.is_exempt(market.lamports(), market.data_len()) {
            return Err(ProgramError::AccountNotRentExempt);
        }

        market_state.version = MARKET_VERSION;
        market_state.owner = *market_owner.key;
        market_state.stake_pool = input.stake_pool;

        market_state.serialize(&mut *market.data.borrow_mut())?;

        Ok(())
    }

    /// Initialize pool
    #[allow(clippy::too_many_arguments)]
    pub fn initialize_pool<'a, 'b>(
        program_id: &ProgramPubkey,
        market: &AccountInfo<'a>,
        pool: &AccountInfo<'a>,
        market_owner: &AccountInfo<'a>,
        mint_collection: &AccountInfo<'a>,
        mint_distribution: &AccountInfo<'a>,
        account_collection: &AccountInfo<'a>,
        account_distribution: &AccountInfo<'a>,
        mint_pool: &AccountInfo<'a>,
        pool_authority: &AccountInfo<'a>,
        rent: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
        _token_program: &AccountInfo<'a>,
        _system_program: &AccountInfo<'a>,
        mint_whitelist: Option<&'b AccountInfo<'a>>,
        input: &InitializePool,
    ) -> ProgramResult {
        is_owner!(&program_id, pool, market);
        let rent_state = &Rent::from_account_info(rent)?;
        let clock = &Clock::from_account_info(clock)?;
        input.validate(clock)?;

        let mut pool_state = Pool::try_from_slice(&pool.data.borrow())?;
        pool_state.uninitialized()?;

        if !rent_state.is_exempt(pool.lamports(), pool.data_len()) {
            return Err(ProgramError::AccountNotRentExempt);
        }

        validate_market_owner(market, market_owner)?;

        if mint_collection.key == mint_distribution.key {
            return Err(Error::WrongTokenMint.into());
        }

        let mint_collection_state = Mint::unpack(&mint_collection.data.borrow())?;
        if !mint_collection_state.is_initialized() {
            return Err(ProgramError::UninitializedAccount);
        }
        let mint_distribution_state = Mint::unpack(&mint_distribution.data.borrow())?;
        if !mint_distribution_state.is_initialized() {
            return Err(ProgramError::UninitializedAccount);
        }

        let (_, authority_bump_seed) = Pubkey::find_key_program_address(pool.key, program_id);

        initialize_token_account(
            account_collection.clone(),
            mint_collection.clone(),
            pool_authority.clone(),
            rent.clone(),
        )?;

        initialize_token_account(
            account_distribution.clone(),
            mint_distribution.clone(),
            pool_authority.clone(),
            rent.clone(),
        )?;

        initialize_mint(
            mint_pool.clone(),
            pool_authority.clone(),
            mint_collection_state.decimals,
            rent.clone(),
        )?;

        pool_state.mint_whitelist = if let Some(mint_whitelist) = mint_whitelist {
            initialize_mint(
                mint_whitelist.clone(),
                pool_authority.clone(),
                0,
                rent.clone(),
            )?;
            MintWhitelist::Key(*mint_whitelist.key)
        } else {
            MintWhitelist::None(DEFAULT_WHITELIST_KEY)
        };

        pool_state.version = POOL_VERSION;
        pool_state.market = *market.key;
        pool_state.account_collection = *account_collection.key;
        pool_state.account_distribution = *account_distribution.key;
        pool_state.mint_pool = mint_pool.pubkey();
        pool_state.price = input.price;
        pool_state.goal_max_collected = input.goal_max;
        pool_state.goal_min_collected = input.goal_min;
        pool_state.amount_investment_min = input.amount_min;
        pool_state.amount_investment_max = input.amount_max;
        pool_state.time_start = input.time_start;
        pool_state.time_finish = input.time_finish;
        pool_state.owner = input.pool_owner;
        pool_state.authority = *pool_authority.key;
        pool_state.authority_bump_seed = authority_bump_seed;
        pool_state.kyc_requirement = input.kyc_requirement;
        pool_state.time_table[..crate::STAGES_ACTIVE_COUNT].copy_from_slice(&input.time_table);

        pool_state.serialize(&mut *pool.data.borrow_mut())?;

        Ok(())
    }

    /// Process participate instruction
    #[allow(clippy::too_many_arguments)]
    pub fn participate<'a, 'b>(
        program_id: &ProgramPubkey,
        market: &AccountInfo<'a>,
        pool: &AccountInfo<'a>,
        pool_authority: &AccountInfo<'a>,
        pool_user_authority: &AccountInfo<'a>,
        user_wallet: &AccountInfo<'a>,
        user_account_from: &AccountInfo<'a>,
        account_collection: &AccountInfo<'a>,
        user_account_to: &AccountInfo<'a>,
        pool_lock_account: &AccountInfo<'a>,
        mint_pool: &AccountInfo<'a>,
        market_user_kyc: &AccountInfo<'a>,
        user_pool_stage: &AccountInfo<'a>,
        pool_lock: &AccountInfo<'a>,
        stake_pool: &AccountInfo<'a>,
        _token_program_id: &AccountInfo<'a>,
        _system_program: &AccountInfo<'a>,
        rent: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
        account_whitelist: Option<&'b AccountInfo<'a>>,
        account_mint_whitelist: Option<&'b AccountInfo<'a>>,
        input: Participate,
    ) -> ProgramResult {
        is_owner!(&program_id, pool, market);
        
        user_wallet.is_signer()?;
        let clock = &Clock::from_account_info(clock)?;
        let rent = &Rent::from_account_info(rent)?;

        let mut pool_state = Pool::try_from_slice(&pool.data.borrow())?;
        pool_state.was_started(clock.unix_timestamp)?;

        let stage = pool_state.get_current_stage(&clock)?;

        let (user_pool_key, user_pool_bump_seed) = Pubkey::find_2key_program_address(pool.key, user_wallet.key, program_id);
        same_key(
            user_pool_key,
            pool_user_authority,
            Error::WrongUserPoolStage,
        )?;

        let seed = format!("{}", stage.to_u8().unwrap_or(0));
        let user_pool_stage_key = Pubkey::create_with_seed(&user_pool_key, seed.as_str(), &program_id.pubkey())?;

        same_key(
            user_pool_stage_key,
            user_pool_stage,
            Error::WrongUserPoolStage,
        )?;

        let signature = &[
            &pool.key.to_bytes()[..32],
            &user_wallet.key.to_bytes()[..32],
            &[user_pool_bump_seed],
        ];
        create_account_with_seed_signed(
            user_wallet,
            user_pool_stage,
            pool_user_authority,
            seed.as_str(),
            rent.minimum_balance(UserPoolStage::LEN),
            UserPoolStage::LEN as u64,
            program_id,
            signature,
        )
        .map_err(|x| {
            if x == ProgramError::Custom(SystemError::AccountAlreadyInUse.to_u32().unwrap()) {
                Error::AccountAlreadyParticipatedOnThisStage.into()
            } else {
                x
            }
        })?;

        let market_state = Market::try_from_slice(&market.data.borrow()).unwrap();
        if market_state.stake_pool != *stake_pool.key {
            return Err(Error::StakePoolMustBelongToMarket.into());
        }

        if pool_state.kyc_requirement != KycRequirement::NotRequired {
            is_owner!(&program_id, market_user_kyc);
            let market_user_kyc = MarketUserKyc::try_from_slice(&market_user_kyc.data.borrow())?;

            if market_user_kyc.market != market.pubkey()
                || market_user_kyc.expiration < clock.unix_timestamp
                || market_user_kyc.user_wallet != user_wallet.pubkey()
            {
                return Err(Error::WrongKycCredentials.into());
            }
        }

        if pool_state.market != *market.key {
            return Err(Error::WrongMarketAddressForCurrentPool.into());
        }

        if *account_collection.key != pool_state.account_collection {
            return Err(Error::WrongCollectAccount.into());
        }

        // NOTE: if these are not setup properly, user deposit many times with zero increase to distributed
        // NOTE: he will still get pool token accumulated leading to non zero distributed
        // NOTE: so user can decrease total distributed in some cases
        if input.amount < pool_state.amount_investment_min
            || input.amount > pool_state.amount_investment_max
        {
            return Err(Error::IncorrectDepositAmount.into());
        }

        if pool_state.amount_collected + input.amount > pool_state.goal_max_collected {
            return Err(Error::PoolAlreadyFull.into());
        }

        if let MintWhitelist::Key(pool_whitelist_mint) = pool_state.mint_whitelist {
            if let (Some(account_whitelist), Some(account_mint_whitelist)) =
                (account_whitelist, account_mint_whitelist)
            {
                if pool_whitelist_mint != *account_mint_whitelist.key {
                    return Err(Error::WhitelistMintInvalid.into());
                }
                burn_tokens_with_user_authority(
                    account_whitelist.clone(),
                    account_mint_whitelist.clone(),
                    user_wallet.clone(),
                    WHITELIST_TOKEN_AMOUNT as u64,
                )?;
            } else {
                return Err(Error::WhitelistMintMissing.into());
            }
        }

        let (amount_collected, tier) = if stage != Stage::FinalStage {
            is_owner!(&sol_starter_staking::program_id(), pool_lock);
            let stake_pool_state = StakePool::try_from_slice(&stake_pool.data.borrow())?;

            let pool_lock = PoolLock::try_from_slice(&pool_lock.data.borrow())?;

            if pool_lock.user_wallet != user_wallet.pubkey() {
                return Err(Error::LockOwnerMustBeUserWallet.into());
            }

            if pool_lock.pool != *stake_pool.key {
                return Err(ProgramError::InvalidArgument);
            }

            if pool_lock.token_account_xsos != pool_lock_account.pubkey() {
                return Err(Error::PoolLockTokenMustBeAttachedToPoolLock.into());
            }

            let pool_lock_account_state = Account::unpack(&pool_lock_account.data.borrow())?;
            pool_state.stage_investment(
                input.amount,
                stage,
                stake_pool_state.tier_balance,
                pool_lock_account_state.amount,
            )?
        } else {
            (input.amount, None)
        };

        pool_state.amount_collected = pool_state.amount_collected.error_add(amount_collected)?;

        pool_state.update_distributed_from_collected(amount_collected, tier, stage)?;

        pool_state.serialize(&mut *pool.data.borrow_mut())?;

        token_transfer_with_user_authority(
            user_account_from.clone(),
            account_collection.clone(),
            user_wallet.clone(),
            amount_collected,
        )?;

        token_mint_to(
            pool.key,
            mint_pool.clone(),
            user_account_to.clone(),
            pool_authority.clone(),
            pool_state.authority_bump_seed,
            amount_collected,
        )?;

        Ok(())
    }

    /// Process [Claim] instruction
    #[allow(clippy::too_many_arguments)]
    pub fn claim<'a>(
        program_id: &ProgramPubkey,
        market: &AccountInfo<'a>,
        pool: &AccountInfo<'a>,
        pool_authority: &AccountInfo<'a>,
        account_from: &AccountInfo<'a>,
        user_authority: &AccountInfo<'a>,
        mint_pool: &AccountInfo<'a>,
        account_pool: &AccountInfo<'a>,
        account_to: &AccountInfo<'a>,
        _token_program_id: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
    ) -> ProgramResult {
        is_owner!(&program_id, pool, market);
        let clock = &Clock::from_account_info(clock)?;
        
        let pool_state = Pool::try_from_slice(&pool.data.borrow())?;
        pool_state.was_started(clock.unix_timestamp)?;

        if pool_state.market != *market.key {
            return Err(Error::WrongMarketAddressForCurrentPool.into());
        }

        if clock.unix_timestamp < pool_state.time_finish {
            return Err(Error::CantClaimFromActivePool.into());
        }

        if *mint_pool.key != pool_state.mint_pool {
            return Err(Error::WrongPoolTokenMint.into());
        }

        let account_from_state = Account::unpack(&account_from.data.borrow())?;

        burn_tokens_with_user_authority(
            account_from.clone(),
            mint_pool.clone(),
            user_authority.clone(),
            account_from_state.amount,
        )?;

        if pool_state.amount_collected >= pool_state.goal_min_collected {
            if *account_pool.key != pool_state.account_distribution {
                return Err(Error::WrongPoolAccountToSendTokensFrom.into());
            }

            let distributed = pool_state.collected_to_distributed(account_from_state.amount)?;
            token_transfer(
                pool.key,
                account_pool.clone(),
                account_to.clone(),
                pool_authority.clone(),
                pool_state.authority_bump_seed,
                distributed,
            )?;
        } else {
            if *account_pool.key != pool_state.account_collection {
                return Err(Error::WrongPoolAccountToSendTokensFrom.into());
            }

            token_transfer(
                pool.key,
                account_pool.clone(),
                account_to.clone(),
                pool_authority.clone(),
                pool_state.authority_bump_seed,
                account_from_state.amount,
            )?;
        }
        Ok(())
    }

    /// Process `AddToWhitelist` instruction
    #[allow(clippy::too_many_arguments)]
    pub fn add_to_whitelist<'a>(
        program_id: &ProgramPubkey,
        pool: &AccountInfo<'a>,
        pool_authority: &AccountInfo<'a>,
        pool_owner: &AccountInfo<'a>,
        account_whitelist: &AccountInfo<'a>,
        mint_whitelist: &AccountInfo<'a>,
        _token_program_id: &AccountInfo<'a>,
    ) -> ProgramResult {
        is_owner!(&program_id, pool);
        let pool_state = Pool::try_from_slice(&pool.data.borrow())?;
        pool_state.initialized()?;
        pool_owner.is_signer()?;

        if *pool_owner.key != pool_state.owner {
            return Err(Error::WrongMarketOwner.into());
        }

        if let MintWhitelist::Key(pool_whitelist_mint) = pool_state.mint_whitelist {
            if pool_whitelist_mint != *mint_whitelist.key {
                return Err(Error::WrongTokenMint.into());
            }
        } else {
            return Err(Error::WhitelistMintNotSet.into());
        }

        token_mint_to(
            pool.key,
            mint_whitelist.clone(),
            account_whitelist.clone(),
            pool_authority.clone(),
            pool_state.authority_bump_seed,
            WHITELIST_TOKEN_AMOUNT as u64,
        )?;
        Ok(())
    }

    /// Process `Withdraw` instruction
    #[allow(clippy::too_many_arguments)]
    pub fn withdraw<'a>(
        program_id: &ProgramPubkey,
        market: &AccountInfo<'a>,
        pool: &AccountInfo<'a>,
        pool_authority: &AccountInfo<'a>,
        pool_owner: &AccountInfo<'a>,
        account_from: &AccountInfo<'a>,
        account_to: &AccountInfo<'a>,
        _token_program: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
    ) -> ProgramResult {
        is_owner!(&program_id, pool);
        pool_owner.is_signer()?;

        let clock = &Clock::from_account_info(clock)?;
        let pool_state = Pool::try_from_slice(&pool.data.borrow())?;
        pool_state.was_started(clock.unix_timestamp)?;
        {
            let market_state = Market::try_from_slice(&market.data.borrow()).unwrap();
            market_state.initialized()?;
        }

        if pool_state.market != market.pubkey() {
            return Err(Error::WrongMarketAddressForCurrentPool.into());
        }
        if *pool_owner.key != pool_state.owner {
            return Err(Error::WrongMarketOwner.into());
        }

        if clock.unix_timestamp < pool_state.time_finish {
            return Err(Error::CantWithdrawFromActivePool.into());
        }

        let account_from_state = Account::unpack(&account_from.data.borrow())?;

        let adjustment = match (*account_from.key, pool_state.success()) {
            (from, true) if from == pool_state.account_collection => Ok(0),
            (from, true) if from == pool_state.account_distribution => {
                Ok(pool_state.amount_to_distribute)
            }
            (from, false) if from == pool_state.account_collection => {
                Ok(pool_state.amount_collected)
            }
            (from, false) if from == pool_state.account_distribution => Ok(0),
            _ => Err(Error::WrongPoolAccountToSendTokensFrom),
        }?;

        let amount_to_withdraw = account_from_state.amount.error_sub(adjustment)?;

        token_transfer(
            pool.key,
            account_from.clone(),
            account_to.clone(),
            pool_authority.clone(),
            pool_state.authority_bump_seed,
            amount_to_withdraw,
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn create_market_user_kyc<'a>(
        program_id: &ProgramPubkey,
        market: &AccountInfo<'a>,
        market_user_authority: &AccountInfo<'a>,
        market_user_kyc: &AccountInfo<'a>,
        market_owner: &AccountInfo<'a>,
        user_wallet: &AccountInfo<'a>,
        rent: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
        _system_program: &AccountInfo<'a>,
        input: &CreateMarketUserKyc,
    ) -> ProgramResult {
        is_owner!(&program_id, market);
        let rent = &Rent::from_account_info(rent)?;
        validate_market_owner(market, market_owner)?;

        let clock = &Clock::from_account_info(clock)?;
        if clock.unix_timestamp > input.expiration {
            return Err(Error::InputTimeMustBeInFuture.into());
        }

        let (market_user_authority_key, bump) =
            Pubkey::find_2key_program_address(&market.pubkey(), &user_wallet.pubkey(), program_id);

        same_key(
            market_user_authority_key,
            market_user_authority,
            Error::MarketAuthorityMustBeDerivedFromMarket,
        )?;

        let market_user_kyc_key = Pubkey::create_with_seed(
            &market_user_authority.pubkey(),
            crate::KYC_SEED,
            &program_id.pubkey(),
        )?;

        same_key(market_user_kyc_key, market_user_kyc, Error::WrongKycAccount)?;

        let signature = &[
            &market.key.to_bytes()[..32],
            &user_wallet.key.to_bytes()[..32],
            &[bump],
        ];

        create_account_with_seed_signed(
            market_owner,
            market_user_kyc,
            market_user_authority,
            crate::KYC_SEED,
            rent.minimum_balance(MarketUserKyc::LEN),
            MarketUserKyc::LEN as u64,
            program_id,
            signature,
        )?;

        let mut user_kyc_state =
            MarketUserKyc::try_from_slice(*market_user_kyc.data.borrow()).unwrap();
        user_kyc_state.uninitialized()?;
        user_kyc_state.market = market.pubkey();
        user_kyc_state.expiration = input.expiration;
        user_kyc_state.user_wallet = user_wallet.pubkey();
        user_kyc_state.version = USER_KYC_VERSION;
        user_kyc_state.serialize(&mut *market_user_kyc.data.borrow_mut())?;
        Ok(())
    }

    fn delete_market_user_kyc<'a>(
        program_id: &ProgramPubkey,
        market: &AccountInfo<'a>,
        market_user_authority: &AccountInfo<'a>,
        market_user_kyc: &AccountInfo<'a>,
        market_owner: &AccountInfo<'a>,
        user_wallet: &AccountInfo<'a>,
        _system_program: &AccountInfo<'a>,
    ) -> ProgramResult {
        is_owner!(&program_id, market, market_user_kyc);
        validate_market_owner(market, market_owner)?;

        let (market_user_authority_key, _) =
            Pubkey::find_2key_program_address(&market.pubkey(), &user_wallet.pubkey(), program_id);

        same_key(
            market_user_authority_key,
            market_user_authority,
            Error::MarketAuthorityMustBeDerivedFromMarket,
        )?;

        let market_user_kyc_key = Pubkey::create_with_seed(
            &market_user_authority.pubkey(),
            crate::KYC_SEED,
            &program_id.pubkey(),
        )?;

        same_key(market_user_kyc_key, market_user_kyc, Error::WrongKycAccount)?;

        crate::utils::program::burn_account(market_user_kyc, market_owner);

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn start_pool<'a>(
        program_id: &ProgramPubkey,
        market: &AccountInfo<'a>,
        market_or_pool_owner: &AccountInfo<'a>,
        stake_pool: &AccountInfo<'a>,
        market_authority: &AccountInfo<'a>,
        pool: &AccountInfo<'a>,
        clock: &AccountInfo<'a>,
        _staking_program: &AccountInfo<'a>,
    ) -> ProgramResult {
        is_owner!(&program_id, market, pool);
        market_or_pool_owner.is_signer()?;

        let mut pool_state = Pool::try_from_slice(*pool.data.borrow()).unwrap();
        pool_state.initialized()?;

        {
            let clock = &Clock::from_account_info(clock)?;
            if clock.unix_timestamp < pool_state.time_start
                || clock.unix_timestamp > pool_state.time_finish
            {
                return Err(Error::InvalidPoolTimeFrame.into());
            }
        }

        let market_state = Market::try_from_slice(&market.data.borrow()).unwrap();
        market_state.initialized()?;

        if market_state.stake_pool != stake_pool.pubkey() {
            return Err(Error::StakePoolMustBelongToMarket.into());
        }

        if pool_state.market != market.pubkey() {
            return Err(Error::WrongMarketAddressForCurrentPool.into());
        }

        if pool_state.owner != market_or_pool_owner.pubkey()
            && market_state.owner != market_or_pool_owner.pubkey()
        {
            return Err(Error::MarketOrPoolOwnerRequired.into());
        }

        let stake_pool_state = StakePool::try_from_slice(*stake_pool.data.borrow()).unwrap();

        pool_state
            .set_tier_allocations(stake_pool_state.tier_users, stake_pool_state.tier_balance)?;

        let (_, market_authority_bump) =
            Pubkey::find_key_program_address(&market.pubkey(), &crate::program_id());

        let market_authority_signature =
            &[&market.pubkey().to_bytes()[..32], &[market_authority_bump]];

        solana_program::program::invoke_signed(
            &sol_starter_staking::instruction::start_pool(
                &stake_pool.pubkey(),
                &market_authority.pubkey(),
                StartPoolInput {
                    pool_active_until: pool_state.time_finish,
                },
            ),
            &[stake_pool.clone(), market_authority.clone(), clock.clone()],
            &[&market_authority_signature[..]],
        )?;

        market_state.serialize(&mut *market.data.borrow_mut())?;

        pool_state.serialize(&mut *pool.data.borrow_mut())?;

        Ok(())
    }

    /// Processes an instruction
    pub fn process_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        input: &[u8],
    ) -> ProgramResult {
        let instruction =
            Instruction::try_from_slice(input).or(Err(ProgramError::InvalidInstructionData))?;
        let program_id = ProgramPubkey(*program_id);
        match instruction {
            Instruction::InitializeMarket(input) => {
                msg!("Instruction::InitializeMarket");
                match accounts {
                    [market, market_owner, rent, ..] => {
                        Self::initialize_market(&program_id, market, market_owner, rent, &input)
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::InitializePool(input) => {
                msg!("Instruction::InitializePool");
                match accounts {
                    [market, pool, market_owner, mint_collection, mint_distribution, account_collection, account_distribution, mint_pool, pool_authority, rent, clock, token_program, system_program, ..] => {
                        Self::initialize_pool(
                            &program_id,
                            market,
                            pool,
                            market_owner,
                            mint_collection,
                            mint_distribution,
                            account_collection,
                            account_distribution,
                            mint_pool,
                            pool_authority,
                            rent,
                            clock,
                            token_program,
                            system_program,
                            accounts.get(13),
                            &input,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::Participate(input) => {
                msg!("Instruction::Participate");
                match accounts {
                    [market, pool, pool_authority, pool_user_authority, user_wallet, user_account_from, account_collection, user_account_to, pool_lock_account, mint_pool, market_user_kyc, user_pool_stage, pool_lock, stake_pool, _token_program_id, _system_program, rent, clock, ..] => {
                        Self::participate(
                            &program_id,
                            market,
                            pool,
                            pool_authority,
                            pool_user_authority,
                            user_wallet,
                            user_account_from,
                            account_collection,
                            user_account_to,
                            pool_lock_account,
                            mint_pool,
                            market_user_kyc,
                            user_pool_stage,
                            pool_lock,
                            stake_pool,
                            _token_program_id,
                            _system_program,
                            rent,
                            clock,
                            accounts.get(18),
                            accounts.get(19),
                            input,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::Claim => {
                msg!("Instruction::Claim");
                match accounts {
                    [market, pool, pool_authority, account_from, user_authority, mint_pool, account_pool, account_to, token_program_id, clock, ..] => {
                        Self::claim(
                            &program_id,
                            market,
                            pool,
                            pool_authority,
                            account_from,
                            user_authority,
                            mint_pool,
                            account_pool,
                            account_to,
                            token_program_id,
                            clock,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::AddToWhitelist => {
                msg!("Instruction::AddToWhitelist");
                match accounts {
                    [pool, pool_authority, pool_owner, account_whitelist, mint_whitelist, token_program, ..] => {
                        Self::add_to_whitelist(
                            &program_id,
                            pool,
                            pool_authority,
                            pool_owner,
                            account_whitelist,
                            mint_whitelist,
                            token_program,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::Withdraw => {
                msg!("Instruction::Withdraw");
                match accounts {
                    [market, pool, pool_authority, pool_owner, account_from, account_to, token_program, clock, ..] => {
                        Self::withdraw(
                            &program_id,
                            market,
                            pool,
                            pool_authority,
                            pool_owner,
                            account_from,
                            account_to,
                            token_program,
                            clock,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::CreateMarketUserKyc(input) => {
                msg!("Instruction::CreateMarketUserKyc");
                match accounts {
                    [market, market_user_authority, market_user_kyc, market_owner, user_wallet, rent, clock, _system_program, ..] => {
                        Self::create_market_user_kyc(
                            &program_id,
                            market,
                            market_user_authority,
                            market_user_kyc,
                            market_owner,
                            user_wallet,
                            rent,
                            clock,
                            _system_program,
                            &input,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::DeleteMarketUserKyc => {
                msg!("Instruction::DeleteMarketUserKyc");
                match accounts {
                    [market, market_user_authority, market_user_kyc, market_owner, user_wallet, _system_program, ..] => {
                        Self::delete_market_user_kyc(
                            &program_id,
                            market,
                            market_user_authority,
                            market_user_kyc,
                            market_owner,
                            user_wallet,
                            _system_program,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
            Instruction::StartPool => {
                msg!("Instruction::StartPool");
                match accounts {
                    [market, market_or_pool_owner, stake_pool, market_authority, pool, clock, _staking_program, ..] => {
                        Self::start_pool(
                            &program_id,
                            market,
                            market_or_pool_owner,
                            stake_pool,
                            market_authority,
                            pool,
                            clock,
                            _staking_program,
                        )
                    }
                    _ => Err(ProgramError::NotEnoughAccountKeys),
                }
            }
        }
    }
}

/// errors if relation is not expected
#[inline]
fn same_key(relation: Pubkey, related: &AccountInfo, error: Error) -> ProgramResult {
    if relation != related.pubkey() {
        return Err(error.into());
    }

    Ok(())
}

fn validate_market_owner(
    market: &AccountInfo,
    market_owner: &AccountInfo,
) -> Result<Market, ProgramError> {
    let market_state = Market::try_from_slice(&market.data.borrow()).unwrap();
    market_state.initialized()?;
    if *market_owner.key != market_state.owner {
        return Err(Error::WrongMarketOwner.into());
    }
    market_owner.is_signer()?;
    Ok(market_state)
}
