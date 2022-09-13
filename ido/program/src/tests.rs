use std::u64;

use crate::{
    error::Error,
    instruction::{
        self, create_market_user_kyc, delete_market_user_kyc, CreateMarketUserKyc, InitializeMarket,
    },
    spl_token_id,
    state::{self, KycRequirement, MarketUserKyc},
    utils::sdk::lock_transaction,
    TIERS_COUNT,
};
use borsh::BorshDeserialize;
use num_traits::ToPrimitive;
use sol_starter_staking::{
    instruction::{InitializePoolInput, StakeStartInput},
    program::{ProgramPubkey, PubkeyPatterns},
    state::{PoolTransit, StakePool},
};
use solana_program::{
    clock::Clock, instruction::InstructionError, program_pack::Pack, pubkey::Pubkey,
    system_instruction,
};
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
    transport::TransportError,
};
use spl_token::state::{Account as TokenAccount, Mint};

pub fn program_test() -> ProgramTest {
    ProgramTest::new(
        "sol_starter_ido",
        crate::id(),
        processor!(crate::processor::Processor::process_instruction),
    )
}

pub async fn get_account(program_context: &mut ProgramTestContext, pubkey: &Pubkey) -> Account {
    program_context
        .banks_client
        .get_account(*pubkey)
        .await
        .expect("account not found")
        .expect("account empty")
}

async fn warp_seconds(program_context: &mut ProgramTestContext, seconds: i64) {
    let ticks_per_slot = program_context.genesis_config().ticks_per_slot();
    assert_eq!(ticks_per_slot, 64);
    assert!(
        seconds as u64 > 10 * ticks_per_slot,
        "clocks are very approximate"
    );

    let before = get_clock(program_context).await.unix_timestamp;
    loop {
        warp(program_context, 100).await;
        let after = get_clock(program_context).await.unix_timestamp;
        if after > before + seconds {
            break;
        }
    }
}

async fn warp(program_context: &mut ProgramTestContext, slots: u64) {
    let slot = program_context.banks_client.get_root_slot().await.unwrap();
    program_context.warp_to_slot(slot + slots).unwrap();
}

async fn get_clock(program_context: &mut ProgramTestContext) -> Clock {
    let clock = program_context
        .banks_client
        .get_account(solana_program::sysvar::clock::id())
        .await
        .unwrap()
        .unwrap();
    let clock: Clock = bincode::deserialize(&clock.data[..]).unwrap();
    clock
}

pub async fn create_account(
    program_context: &mut ProgramTestContext,
    account: &Keypair,
    rent: u64,
    space: u64,
    owner: &ProgramPubkey,
) -> Result<(), TransportError> {
    let mut transaction = Transaction::new_with_payer(
        &[system_instruction::create_account(
            &program_context.payer.pubkey(),
            &account.pubkey(),
            rent,
            space,
            &owner.pubkey(),
        )],
        Some(&program_context.payer.pubkey()),
    );

    transaction.sign(
        &[&program_context.payer, account],
        program_context.last_blockhash,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await?;
    Ok(())
}

pub async fn create_account_user(
    program_context: &mut ProgramTestContext,
    account: &Keypair,
    rent: u64,
    space: u64,
    owner: &Pubkey,
    user_wallet: &Keypair,
) -> Result<(), TransportError> {
    let mut transaction = Transaction::new_with_payer(
        &[system_instruction::create_account(
            &user_wallet.pubkey(),
            &account.pubkey(),
            rent,
            space,
            owner,
        )],
        Some(&user_wallet.pubkey()),
    );

    transaction.sign(&[user_wallet, account], program_context.last_blockhash);
    program_context
        .banks_client
        .process_transaction(transaction)
        .await?;
    Ok(())
}

pub async fn create_mint(
    program_context: &mut ProgramTestContext,
    mint_account: &Keypair,
    mint_rent: u64,
    authority: &Pubkey,
    initialize: bool,
) -> Result<(), TransportError> {
    let mut instructions = vec![system_instruction::create_account(
        &program_context.payer.pubkey(),
        &mint_account.pubkey(),
        mint_rent,
        spl_token::state::Mint::LEN as u64,
        &spl_token_id().pubkey(),
    )];

    if initialize {
        instructions.push(
            spl_token::instruction::initialize_mint(
                &spl_token_id().pubkey(),
                &mint_account.pubkey(),
                authority,
                None,
                0,
            )
            .unwrap(),
        );
    }

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&program_context.payer.pubkey()));

    transaction.sign(
        &[&program_context.payer, mint_account],
        program_context.last_blockhash,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await?;
    Ok(())
}

pub async fn mint_tokens_to(
    program_context: &mut ProgramTestContext,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Keypair,
    amount: u64,
) -> Result<(), TransportError> {
    let mut transaction = Transaction::new_with_payer(
        &[spl_token::instruction::mint_to(
            &spl_token_id().pubkey(),
            mint,
            destination,
            &authority.pubkey(),
            &[&authority.pubkey()],
            amount,
        )
        .unwrap()],
        Some(&program_context.payer.pubkey()),
    );
    transaction.sign(
        &[&program_context.payer, authority],
        program_context.last_blockhash,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await?;
    Ok(())
}

pub async fn create_token_account(
    program_context: &mut ProgramTestContext,
    account: &Keypair,
    account_rent: u64,
    mint: &Pubkey,
    owner: &Pubkey,
) -> Result<(), TransportError> {
    let instructions = vec![
        system_instruction::create_account(
            &program_context.payer.pubkey(),
            &account.pubkey(),
            account_rent,
            spl_token::state::Account::LEN as u64,
            &spl_token_id().pubkey(),
        ),
        spl_token::instruction::initialize_account(
            &spl_token_id().pubkey(),
            &account.pubkey(),
            mint,
            owner,
        )
        .unwrap(),
    ];

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&program_context.payer.pubkey()));

    transaction.sign(
        &[&program_context.payer, account],
        program_context.last_blockhash,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await?;
    Ok(())
}

pub async fn create_market(
    program_context: &mut ProgramTestContext,
    stake_pool: Pubkey,
    market: Keypair,
) -> Keypair {
    let rent = program_context.banks_client.get_rent().await.unwrap();

    let mut transaction = create_initialize_market_transaction(
        &program_context.payer,
        market.pubkey(),
        rent,
        stake_pool,
    );

    transaction.sign(
        &[&program_context.payer, &market],
        program_context.last_blockhash,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    market
}

fn create_initialize_market_transaction(
    payer: &Keypair,
    market: Pubkey,
    rent: solana_program::rent::Rent,
    stake_pool: Pubkey,
) -> Transaction {
    Transaction::new_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &market,
                rent.minimum_balance(state::Market::LEN),
                state::Market::LEN as u64,
                &crate::id(),
            ),
            instruction::initialize_market(
                &crate::program_id(),
                &market,
                &payer.pubkey(),
                InitializeMarket { stake_pool },
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
    )
}

pub struct Pool {
    pub pool: Keypair,
    pub market: Pubkey,
    pub mint_collection: Keypair,
    pub mint_collection_authority: Keypair,
    pub mint_distribution: Keypair,
    pub mint_distribution_authority: Keypair,
    pub account_collection: Keypair,
    pub account_distribution: Keypair,
    pub mint_pool: Keypair,
    pub account_pool_authority: Pubkey,
    pub mint_whitelist_account: Option<Pubkey>,
    pub stake_pool: Pubkey,
    pub pool_lock: Pubkey,
}

impl Pool {
    pub fn new(market: &Pubkey, stake_pool: Pubkey, pool_lock: Pubkey) -> Self {
        let pool = Keypair::new();
        let account_distribution = Keypair::new();
        let account_collection = Keypair::new();
        let mint_pool = Keypair::new();

        let (account_pool_authority, _) =
            Pubkey::find_program_address(&[&market.to_bytes()[..32]], &crate::id());

        Self {
            pool,
            market: *market,
            mint_collection: Keypair::new(),
            mint_collection_authority: Keypair::new(),
            mint_distribution: Keypair::new(),
            mint_distribution_authority: Keypair::new(),
            account_collection,
            account_distribution,
            mint_pool,
            account_pool_authority,
            mint_whitelist_account: None,
            stake_pool,
            pool_lock,
        }
    }

    pub async fn create_pool(
        &mut self,
        program_context: &mut ProgramTestContext,
        mint_whitelist: bool,
        init_args: instruction::InitializePool,
    ) -> Result<(), TransportError> {
        let rent = program_context.banks_client.get_rent().await.unwrap();
        let mint_account_min_rent = rent.minimum_balance(spl_token::state::Mint::LEN);

        let max_rent = 10_000_000;
        let pool_rent = rent.minimum_balance(state::Pool::LEN);

        create_account(
            program_context,
            &self.account_distribution,
            max_rent,
            TokenAccount::LEN as u64,
            &crate::spl_token_id(),
        )
        .await
        .unwrap();

        create_account(
            program_context,
            &self.account_collection,
            max_rent,
            TokenAccount::LEN as u64,
            &crate::spl_token_id(),
        )
        .await
        .unwrap();

        create_account(
            program_context,
            &self.mint_pool,
            max_rent,
            Mint::LEN as u64,
            &crate::spl_token_id(),
        )
        .await
        .unwrap();

        create_account(
            program_context,
            &self.pool,
            pool_rent,
            state::Pool::LEN as u64,
            &crate::program_id(),
        )
        .await
        .unwrap();

        create_mint(
            program_context,
            &self.mint_collection,
            mint_account_min_rent,
            &self.mint_collection_authority.pubkey(),
            true,
        )
        .await
        .unwrap();

        create_mint(
            program_context,
            &self.mint_distribution,
            mint_account_min_rent,
            &self.mint_distribution_authority.pubkey(),
            true,
        )
        .await
        .unwrap();

        let mint_whitelist_keypair = Keypair::new();
        self.mint_whitelist_account = if mint_whitelist {
            create_mint(
                program_context,
                &mint_whitelist_keypair,
                mint_account_min_rent,
                &self.mint_distribution_authority.pubkey(),
                false,
            )
            .await
            .unwrap();
            Some(mint_whitelist_keypair.pubkey())
        } else {
            None
        };

        let mut transaction = Transaction::new_with_payer(
            &[instruction::initialize_pool(
                &crate::program_id(),
                &self.pool.pubkey(),
                &self.market,
                &program_context.payer.pubkey(),
                &self.mint_collection.pubkey(),
                &self.mint_distribution.pubkey(),
                &self.account_collection.pubkey(),
                &self.account_distribution.pubkey(),
                &self.mint_pool.pubkey(),
                self.mint_whitelist_account,
                init_args,
            )
            .unwrap()],
            Some(&program_context.payer.pubkey()),
        );

        transaction.sign(&[&program_context.payer], program_context.last_blockhash);
        program_context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn participate(
        &self,
        program_context: &mut ProgramTestContext,
        user_wallet: &Keypair,
        user_account_from: &Pubkey,
        user_account_to: &Pubkey,
        pool_lock_account: &Pubkey,
        market_user_kyc: Option<&Pubkey>,
        account_whitelist: Option<&Pubkey>,
        mint_whitelist: Option<&Pubkey>,
        amount: u64,
        stage: u8,
    ) -> Result<(), TransportError> {
        let mut transaction = Transaction::new_with_payer(
            &[instruction::participate(
                &crate::program_id(),
                &self.pool.pubkey(),
                &self.market,
                &user_wallet.pubkey(),
                user_account_from,
                &self.account_collection.pubkey(),
                user_account_to,
                pool_lock_account,
                &self.mint_pool.pubkey(),
                &self.pool_lock,
                &self.stake_pool,
                market_user_kyc,
                account_whitelist,
                mint_whitelist,
                instruction::Participate { amount },
                stage,
            )
            .unwrap()],
            Some(&program_context.payer.pubkey()),
        );

        transaction.sign(
            &[&program_context.payer, user_wallet],
            program_context.last_blockhash,
        );
        program_context
            .banks_client
            .process_transaction(transaction)
            .await?;
        Ok(())
    }

    pub async fn claim(
        &self,
        program_context: &mut ProgramTestContext,
        account_from: &Pubkey,
        user_authority: &Keypair,
        account_to: &Pubkey,
        claim_collectibles: bool,
    ) -> Result<(), TransportError> {
        let account = if claim_collectibles {
            self.account_collection.pubkey()
        } else {
            self.account_distribution.pubkey()
        };
        let mut transaction = Transaction::new_with_payer(
            &[instruction::claim(
                &crate::program_id(),
                &self.pool.pubkey(),
                &self.market,
                account_from,
                &user_authority.pubkey(),
                &self.mint_pool.pubkey(),
                &account,
                account_to,
            )
            .unwrap()],
            Some(&program_context.payer.pubkey()),
        );

        transaction.sign(
            &[&program_context.payer, user_authority],
            program_context.last_blockhash,
        );
        program_context
            .banks_client
            .process_transaction(transaction)
            .await?;
        Ok(())
    }

    pub async fn add_to_whitelist(
        &self,
        program_context: &mut ProgramTestContext,
        account_whitelist: &Pubkey,
    ) -> Result<(), TransportError> {
        let mut transaction = Transaction::new_with_payer(
            &[instruction::add_to_whitelist(
                &crate::program_id(),
                &self.pool.pubkey(),
                &program_context.payer.pubkey(),
                account_whitelist,
                &self.mint_whitelist_account.unwrap(),
            )
            .unwrap()],
            Some(&program_context.payer.pubkey()),
        );

        transaction.sign(&[&program_context.payer], program_context.last_blockhash);
        program_context
            .banks_client
            .process_transaction(transaction)
            .await?;
        Ok(())
    }
}

#[tokio::test]
async fn test_kyc() {
    let mut program_context = program_test();

    program_context.add_program(
        "sol_starter_staking",
        sol_starter_staking::id(),
        processor!(crate::processor::Processor::process_instruction),
    );
    let user_wallet = Keypair::new();
    program_context.add_account(
        user_wallet.pubkey(),
        Account {
            lamports: 1_000_000_000_000_000,
            ..Default::default()
        },
    );

    let market = Keypair::new();
    let tiers_balance = [50, 100, 150, 200];

    let (mut program_context, stake_pool, pool_lock, pool_lock_token) = setup_staking(
        program_context,
        market.pubkey(),
        &user_wallet,
        tiers_balance,
        2500,
    )
    .await;

    let market = create_market(&mut program_context, stake_pool.pubkey(), market).await;
    let now = get_clock(&mut program_context).await.unix_timestamp;
    let init_args = instruction::InitializePool {
        pool_owner: user_wallet.pubkey(),
        price: 5,
        goal_max: 150,
        goal_min: 10,
        amount_min: 3,
        amount_max: 100,
        time_start: now + 60 * 60,
        time_finish: now + 3 * 60 * 60,
        kyc_requirement: KycRequirement::AnyRequired,
        time_table: [0; crate::STAGES_ACTIVE_COUNT],
    };

    let mut pool = Pool::new(&market.pubkey(), stake_pool.pubkey(), pool_lock);
    pool.create_pool(&mut program_context, false, init_args.clone())
        .await
        .unwrap();

    let user_investment_amount = 50;
    let user_collection_account = Keypair::new();

    let rent = program_context.banks_client.get_rent().await.unwrap();
    let token_account_min_rent = rent.minimum_balance(spl_token::state::Account::LEN);

    create_token_account(
        &mut program_context,
        &user_collection_account,
        token_account_min_rent,
        &pool.mint_collection.pubkey(),
        &user_wallet.pubkey(),
    )
    .await
    .unwrap();
    mint_tokens_to(
        &mut program_context,
        &pool.mint_collection.pubkey(),
        &user_collection_account.pubkey(),
        &pool.mint_collection_authority,
        user_investment_amount,
    )
    .await
    .unwrap();

    let user_pool_token_account = Keypair::new();
    create_token_account(
        &mut program_context,
        &user_pool_token_account,
        token_account_min_rent,
        &pool.mint_pool.pubkey(),
        &user_wallet.pubkey(),
    )
    .await
    .unwrap();

    // Rewind slots to do investment
    warp_seconds(&mut program_context, 2 * 60 * 60).await;

    pool.participate(
        &mut program_context,
        &user_wallet,
        &user_collection_account.pubkey(),
        &user_pool_token_account.pubkey(),
        &pool_lock_token,
        None,
        None,
        None,
        user_investment_amount,
        2,
    )
    .await
    .unwrap_err();

    let (transaction, market_user_kyc) =
        create_market_user_kyc_transaction(market.pubkey(), &program_context, &user_wallet);

    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let account = program_context
        .banks_client
        .get_account(market_user_kyc)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(account.owner, crate::id());

    let account_state = program_context
        .banks_client
        .get_account_data_with_borsh::<MarketUserKyc>(market_user_kyc)
        .await
        .unwrap();

    assert_eq!(account_state.user_wallet, user_wallet.pubkey());
    assert_eq!(account_state.market, market.pubkey());
    assert_ne!(account_state.expiration, 0);

    pool.participate(
        &mut program_context,
        &user_wallet,
        &user_collection_account.pubkey(),
        &user_pool_token_account.pubkey(),
        &pool_lock_token,
        Some(&market_user_kyc),
        None,
        None,
        user_investment_amount,
        2,
    )
    .await
    .unwrap();

    let transaction =
        delete_user_market_kyc_transaction(&market.pubkey(), &program_context, &user_wallet);

    warp_seconds(&mut program_context, 60 * 60).await;

    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    pool.participate(
        &mut program_context,
        &user_wallet,
        &user_collection_account.pubkey(),
        &user_pool_token_account.pubkey(),
        &pool_lock_token,
        Some(&market_user_kyc),
        None,
        None,
        user_investment_amount,
        4,
    )
    .await
    .unwrap_err();

    let account = program_context
        .banks_client
        .get_account(market_user_kyc)
        .await
        .unwrap();
    assert!(account.is_none());
}

fn delete_user_market_kyc_transaction(
    market: &Pubkey,
    program_context: &ProgramTestContext,
    user_wallet: &Keypair,
) -> Transaction {
    let instruction = delete_market_user_kyc(
        &crate::program_id(),
        &market,
        &program_context.payer.pubkey(),
        &user_wallet.pubkey(),
    )
    .unwrap();
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));
    transaction.sign(&[&program_context.payer], program_context.last_blockhash);
    transaction
}

fn calc_market_user_kyc(market: &Pubkey,
    user_wallet: &Pubkey,) -> Pubkey {
    let (market_user_authority_key, _) =
        Pubkey::find_2key_program_address(&market, &user_wallet, &crate::program_id());
    let market_user_kyc =
        Pubkey::create_with_seed(&market_user_authority_key, crate::KYC_SEED, &crate::id()).unwrap();
    market_user_kyc
}

fn create_market_user_kyc_transaction(
    market: Pubkey,
    program_context: &ProgramTestContext,
    user_wallet: &Keypair,
) -> (Transaction, Pubkey) {
    let instruction = create_market_user_kyc(
        &market,
        &program_context.payer.pubkey(),
        &user_wallet.pubkey(),
        CreateMarketUserKyc {
            expiration: 1_000_000_000_000_000,
        },
    ).unwrap();

    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));
    transaction.sign(&[&program_context.payer], program_context.last_blockhash);
    (transaction, calc_market_user_kyc(&market, &user_wallet.pubkey()))
}

#[tokio::test]
async fn test_initialize_pool() {
    let mut program_context = program_test().start_with_context().await;
    let stake_pool = Pubkey::new_unique();
    let market = Keypair::new();
    let market = create_market(&mut program_context, stake_pool, market).await;

    let now = get_clock(&mut program_context).await.unix_timestamp;

    let input = instruction::InitializePool {
        pool_owner: program_context.payer.pubkey(),
        price: 5,
        goal_max: 100,
        goal_min: 90,
        amount_min: 3,
        amount_max: 10,
        time_start: now + 60 * 60,
        time_finish: now + 10 * 60 * 60,
        kyc_requirement: KycRequirement::NotRequired,
        time_table: [0; crate::STAGES_ACTIVE_COUNT],
    };

    let pool_lock = Pubkey::new_unique();
    let mut pool = Pool::new(&market.pubkey(), stake_pool, pool_lock);
    pool.create_pool(&mut program_context, false, input)
        .await
        .unwrap();

    // Check that pool is initialized
    let pool_info = get_account(&mut program_context, &pool.pool.pubkey()).await;
    let pool_info = state::Pool::try_from_slice(&pool_info.data.as_slice()).unwrap();

    pool_info.initialized().unwrap();
}

#[tokio::test]
async fn test_participate() {
    let mut program_context = program_test();

    program_context.add_program(
        "sol_starter_staking",
        sol_starter_staking::id(),
        processor!(crate::processor::Processor::process_instruction),
    );
    let user_wallet = Keypair::new();
    program_context.add_account(
        user_wallet.pubkey(),
        Account {
            lamports: 1_000_000_000_000_000,
            ..Default::default()
        },
    );

    let market = Keypair::new();
    let tiers_balance = [50, 100, 150, 200];
    let pool_lock_amount = 2500;

    let (mut program_context, stake_pool, pool_lock, pool_lock_token) = setup_staking(
        program_context,
        market.pubkey(),
        &user_wallet,
        tiers_balance,
        pool_lock_amount,
    )
    .await;

    let now = get_clock(&mut program_context).await.unix_timestamp;
    let init_args = instruction::InitializePool {
        pool_owner: user_wallet.pubkey(),
        price: 5,
        goal_max: 1_000_000,
        goal_min: 10,
        amount_min: 3,
        amount_max: 1_000_000,
        time_start: now + 60 * 60,
        time_finish: now + 10 * 60 * 60,
        kyc_requirement: KycRequirement::NotRequired,
        time_table: [60 * 60, 60 * 60],
    };
    let user_investment_amount = 50;

    let market = create_market(&mut program_context, stake_pool.pubkey(), market).await;

    let mut pool = Pool::new(&market.pubkey(), stake_pool.pubkey(), pool_lock);
    pool.create_pool(&mut program_context, false, init_args.clone())
        .await
        .unwrap();

    let rent = program_context.banks_client.get_rent().await.unwrap();
    let token_account_min_rent = rent.minimum_balance(spl_token::state::Account::LEN);

    let user_collection_account = Keypair::new();
    create_token_account(
        &mut program_context,
        &user_collection_account,
        token_account_min_rent,
        &pool.mint_collection.pubkey(),
        &user_wallet.pubkey(),
    )
    .await
    .unwrap();

    mint_tokens_to(
        &mut program_context,
        &pool.mint_collection.pubkey(),
        &user_collection_account.pubkey(),
        &pool.mint_collection_authority,
        user_investment_amount,
    )
    .await
    .unwrap();

    let user_account_to = Keypair::new();
    create_token_account(
        &mut program_context,
        &user_account_to,
        token_account_min_rent,
        &pool.mint_pool.pubkey(),
        &user_wallet.pubkey(),
    )
    .await
    .unwrap();

    warp_seconds(&mut program_context, 1 * 60 * 60).await;

    let error = pool
        .participate(
            &mut program_context,
            &user_wallet,
            &user_collection_account.pubkey(),
            &user_account_to.pubkey(),
            &pool_lock_token,
            None,
            None,
            None,
            5,
            0,
        )
        .await
        .unwrap_err();

    let _expected_error = TransportError::TransactionError(TransactionError::InstructionError(
        0,
        InstructionError::Custom(Error::CanParticipateOnlyInStartedPool.to_u32().unwrap()),
    ));
    assert!(matches!(error, _expected_error));

    let transaction = start_pool_transaction(&program_context, &pool);
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let pool_account = program_context
        .banks_client
        .get_account_data_with_borsh::<crate::state::Pool>(pool.pool.pubkey())
        .await
        .unwrap();
    assert_eq!(
        pool_account.tier_allocation,
        [
            50000000000000,
            100000000000000,
            150000000000000,
            200000000000000
        ]
    );
    assert_eq!(pool_account.tier_remaining, [0, 0, 0, 200000000000000]);

    warp_seconds(&mut program_context, 1 * 60 * 60).await;

    pool.participate(
        &mut program_context,
        &user_wallet,
        &user_collection_account.pubkey(),
        &user_account_to.pubkey(),
        &pool_lock_token,
        None,
        None,
        None,
        user_investment_amount,
        1,
    )
    .await
    .unwrap();

    let user_pool_token_account_info =
        get_account(&mut program_context, &user_account_to.pubkey()).await;
    let user_pool_token_account_info =
        spl_token::state::Account::unpack_from_slice(user_pool_token_account_info.data.as_slice())
            .unwrap();
    assert_eq!(user_pool_token_account_info.amount, user_investment_amount);
}

fn start_pool_transaction(program_context: &ProgramTestContext, pool: &Pool) -> Transaction {
    let mut transaction = Transaction::new_with_payer(
        &[instruction::start_pool(
            &crate::program_id(),
            &program_context.payer.pubkey(),
            &pool.stake_pool,
            &pool.market.pubkey(),
            &pool.pool.pubkey(),
        )
        .unwrap()],
        Some(&program_context.payer.pubkey()),
    );
    transaction.sign(&[&program_context.payer], program_context.last_blockhash);
    transaction
}

#[tokio::test]
async fn test_claim() {
    let mut program_context = program_test();

    program_context.add_program(
        "sol_starter_staking",
        sol_starter_staking::id(),
        processor!(crate::processor::Processor::process_instruction),
    );
    let user_wallet = Keypair::new();
    program_context.add_account(
        user_wallet.pubkey(),
        Account {
            lamports: 1_000_000_000_000_000,
            ..Default::default()
        },
    );

    let market = Keypair::new();
    let tiers_balance = [50, 100, 150, 200];
    let (mut program_context, stake_pool, pool_lock, pool_lock_token) = setup_staking(
        program_context,
        market.pubkey(),
        &user_wallet,
        tiers_balance,
        2500,
    )
    .await;

    let market = create_market(&mut program_context, stake_pool.pubkey(), market).await;
    let now = get_clock(&mut program_context).await.unix_timestamp;
    let init_args = instruction::InitializePool {
        pool_owner: user_wallet.pubkey(),
        price: 5,
        goal_max: 150,
        goal_min: 10,
        amount_min: 3,
        amount_max: 100,
        time_start: now + 60 * 60,
        time_finish: now + 3 * 60 * 60,
        kyc_requirement: KycRequirement::NotRequired,
        time_table: [0; crate::STAGES_ACTIVE_COUNT],
    };

    let mut pool = Pool::new(&market.pubkey(), stake_pool.pubkey(), pool_lock);
    pool.create_pool(&mut program_context, false, init_args.clone())
        .await
        .unwrap();

    let rent = program_context.banks_client.get_rent().await.unwrap();
    let token_account_min_rent = rent.minimum_balance(spl_token::state::Account::LEN);

    let user_investment_amount = 50;
    let user_collection_account = Keypair::new();

    create_token_account(
        &mut program_context,
        &user_collection_account,
        token_account_min_rent,
        &pool.mint_collection.pubkey(),
        &user_wallet.pubkey(),
    )
    .await
    .unwrap();
    mint_tokens_to(
        &mut program_context,
        &pool.mint_collection.pubkey(),
        &user_collection_account.pubkey(),
        &pool.mint_collection_authority,
        user_investment_amount,
    )
    .await
    .unwrap();

    let user_pool_token_account = Keypair::new();
    create_token_account(
        &mut program_context,
        &user_pool_token_account,
        token_account_min_rent,
        &pool.mint_pool.pubkey(),
        &user_wallet.pubkey(),
    )
    .await
    .unwrap();

    // Rewind slots to do investment
    warp_seconds(&mut program_context, 2 * 60 * 60).await;
    pool.participate(
        &mut program_context,
        &user_wallet,
        &user_collection_account.pubkey(),
        &user_pool_token_account.pubkey(),
        &pool_lock_token,
        None,
        None,
        None,
        user_investment_amount,
        2,
    )
    .await
    .unwrap();

    mint_tokens_to(
        &mut program_context,
        &pool.mint_distribution.pubkey(),
        &pool.account_distribution.pubkey(),
        &pool.mint_distribution_authority,
        100000000 * crate::state::Pool::PRECISION,
    )
    .await
    .unwrap();

    let user_distribution_token_account = Keypair::new();
    create_token_account(
        &mut program_context,
        &user_distribution_token_account,
        token_account_min_rent,
        &pool.mint_distribution.pubkey(),
        &user_wallet.pubkey(),
    )
    .await
    .unwrap();

    warp_seconds(&mut program_context, 2 * 60 * 60).await;

    pool.claim(
        &mut program_context,
        &user_pool_token_account.pubkey(),
        &user_wallet,
        &user_distribution_token_account.pubkey(),
        false,
    )
    .await
    .unwrap();

    let user_distribution_token_account_info = get_account(
        &mut program_context,
        &user_distribution_token_account.pubkey(),
    )
    .await;
    let user_distribution_token_account_info = spl_token::state::Account::unpack_from_slice(
        user_distribution_token_account_info.data.as_slice(),
    )
    .unwrap();

    assert_eq!(
        user_investment_amount * crate::state::Pool::PRECISION / init_args.price,
        user_distribution_token_account_info.amount
    );
}

#[tokio::test]
async fn test_add_to_whitelist() {
    let mut program_context = program_test();

    program_context.add_program(
        "sol_starter_staking",
        sol_starter_staking::id(),
        processor!(crate::processor::Processor::process_instruction),
    );
    let user_wallet = Keypair::new();
    program_context.add_account(
        user_wallet.pubkey(),
        Account {
            lamports: 1_000_000_000_000_000,
            ..Default::default()
        },
    );

    let market = Keypair::new();
    let tiers_balance = [50, 100, 150, 200];
    let (mut program_context, stake_pool, pool_lock, _) = setup_staking(
        program_context,
        market.pubkey(),
        &user_wallet,
        tiers_balance,
        2500,
    )
    .await;

    let market = create_market(&mut program_context, stake_pool.pubkey(), market).await;
    let now = get_clock(&mut program_context).await.unix_timestamp;
    let init_args = instruction::InitializePool {
        pool_owner: program_context.payer.pubkey(),
        price: 5,
        goal_max: 150,
        goal_min: 10,
        amount_min: 3,
        amount_max: 100,
        time_start: now + 60 * 60,
        time_finish: now + 3 * 60 * 60,
        kyc_requirement: KycRequirement::NotRequired,
        time_table: [0; crate::STAGES_ACTIVE_COUNT],
    };

    let mut pool = Pool::new(&market.pubkey(), stake_pool.pubkey(), pool_lock);
    pool.create_pool(&mut program_context, true, init_args.clone())
        .await
        .unwrap();

    let user_wallet = Keypair::new();
    let user_whitelist_account = Keypair::new();
    let rent = program_context.banks_client.get_rent().await.unwrap();
    let token_account_min_rent = rent.minimum_balance(spl_token::state::Account::LEN);
    create_token_account(
        &mut program_context,
        &user_whitelist_account,
        token_account_min_rent,
        &pool.mint_whitelist_account.unwrap(),
        &user_wallet.pubkey(),
    )
    .await
    .unwrap();

    pool.add_to_whitelist(&mut program_context, &user_whitelist_account.pubkey())
        .await
        .unwrap();

    let user_whitelist_account_info =
        get_account(&mut program_context, &user_whitelist_account.pubkey()).await;
    let user_whitelist_account =
        spl_token::state::Account::unpack_from_slice(user_whitelist_account_info.data.as_slice())
            .unwrap();

    assert_eq!(
        user_whitelist_account.amount,
        state::WHITELIST_TOKEN_AMOUNT as u64
    );
}

#[tokio::test]
async fn test_withdraw() {
    let mut program_context = program_test();

    program_context.add_program(
        "sol_starter_staking",
        sol_starter_staking::id(),
        processor!(crate::processor::Processor::process_instruction),
    );
    let user_wallet = Keypair::new();
    program_context.add_account(
        user_wallet.pubkey(),
        Account {
            lamports: 1_000_000_000_000_000,
            ..Default::default()
        },
    );

    let market = Keypair::new();
    let tiers_balance = [50, 100, 150, 200];
    let (mut program_context, stake_pool, pool_lock, pool_lock_token) = setup_staking(
        program_context,
        market.pubkey(),
        &user_wallet,
        tiers_balance,
        2500,
    )
    .await;

    let market = create_market(&mut program_context, stake_pool.pubkey(), market).await;
    let now = get_clock(&mut program_context).await.unix_timestamp;
    let init_args = instruction::InitializePool {
        pool_owner: program_context.payer.pubkey(),
        price: 5,
        goal_max: 150,
        goal_min: 10,
        amount_min: 3,
        amount_max: 100,
        time_start: now + 60 * 60,
        time_finish: now + 3 * 60 * 60,
        kyc_requirement: KycRequirement::NotRequired,
        time_table: [0; crate::STAGES_ACTIVE_COUNT],
    };

    let mut pool = Pool::new(&market.pubkey(), stake_pool.pubkey(), pool_lock);
    pool.create_pool(&mut program_context, false, init_args.clone())
        .await
        .unwrap();

    let user_investment_amount = 50;
    let user_collection_account = Keypair::new();

    let rent = program_context.banks_client.get_rent().await.unwrap();
    let token_account_min_rent = rent.minimum_balance(spl_token::state::Account::LEN);
    create_token_account(
        &mut program_context,
        &user_collection_account,
        token_account_min_rent,
        &pool.mint_collection.pubkey(),
        &user_wallet.pubkey(),
    )
    .await
    .unwrap();
    mint_tokens_to(
        &mut program_context,
        &pool.mint_collection.pubkey(),
        &user_collection_account.pubkey(),
        &pool.mint_collection_authority,
        user_investment_amount,
    )
    .await
    .unwrap();

    let user_pool_token_account = Keypair::new();
    create_token_account(
        &mut program_context,
        &user_pool_token_account,
        token_account_min_rent,
        &pool.mint_pool.pubkey(),
        &user_wallet.pubkey(),
    )
    .await
    .unwrap();

    // Rewind slots to do investment
    warp_seconds(&mut program_context, 2 * 60 * 60).await;

    pool.participate(
        &mut program_context,
        &user_wallet,
        &user_collection_account.pubkey(),
        &user_pool_token_account.pubkey(),
        &pool_lock_token,
        None,
        None,
        None,
        user_investment_amount,
        2,
    )
    .await
    .unwrap();

    warp_seconds(&mut program_context, 2 * 60 * 60).await;

    let collectible_account_for_withdraw = Keypair::new();
    create_token_account(
        &mut program_context,
        &collectible_account_for_withdraw,
        token_account_min_rent,
        &pool.mint_collection.pubkey(),
        &user_wallet.pubkey(),
    )
    .await
    .unwrap();

    let account_collection_info =
        get_account(&mut program_context, &pool.account_collection.pubkey()).await;
    let account_collection_info =
        spl_token::state::Account::unpack_from_slice(account_collection_info.data.as_slice())
            .unwrap();
    let collection_balance_before = account_collection_info.amount;

    let mut transaction = Transaction::new_with_payer(
        &[instruction::withdraw(
            &crate::program_id(),
            &pool.pool.pubkey(),
            &pool.market,
            &program_context.payer.pubkey(),
            &pool.account_collection.pubkey(),
            &collectible_account_for_withdraw.pubkey(),
        )
        .unwrap()],
        Some(&program_context.payer.pubkey()),
    );

    transaction.sign(&[&program_context.payer], program_context.last_blockhash);
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let withdraw_acc_info = get_account(
        &mut program_context,
        &collectible_account_for_withdraw.pubkey(),
    )
    .await;
    let withdraw_acc_info =
        spl_token::state::Account::unpack_from_slice(withdraw_acc_info.data.as_slice()).unwrap();

    assert_eq!(withdraw_acc_info.amount, collection_balance_before);
}

async fn setup_staking(
    program_test: ProgramTest,
    ido_market: Pubkey,
    user_wallet: &Keypair,
    tier_balance: [u64; TIERS_COUNT],
    pool_lock_amount: u64,
) -> (ProgramTestContext, Keypair, Pubkey, Pubkey) {
    let mut program_context = program_test.start_with_context().await;
    let rent = &program_context.banks_client.get_rent().await.unwrap();

    let pool = Keypair::new();
    let mint_sos = Keypair::new();
    let mint_sos_authority = Keypair::new();
    let mint_xsos = Keypair::new();
    let pool_token_sos = Keypair::new();

    let pool_transit_from = Keypair::new();
    let pool_transit_from_token = Keypair::new();

    let pool_transit_to = Keypair::new();
    let pool_transit_to_token = Keypair::new();

    let user_token_sos = Keypair::new();
    let user_token_xsos = Keypair::new();

    let rent = rent.minimum_balance(1_000);

    let pool_lock_token = Keypair::new();

    create_account(
        &mut program_context,
        &pool_lock_token,
        rent,
        TokenAccount::LEN as u64,
        &spl_token_id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &pool_transit_from,
        rent,
        PoolTransit::LEN as u64,
        &sol_starter_staking::program_id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &pool_transit_from_token,
        rent,
        TokenAccount::LEN as u64,
        &spl_token_id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &pool_transit_to_token,
        rent,
        TokenAccount::LEN as u64,
        &spl_token_id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &pool_transit_to,
        rent,
        PoolTransit::LEN as u64,
        &sol_starter_staking::program_id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &pool,
        rent,
        StakePool::LEN as u64,
        &sol_starter_staking::program_id(),
    )
    .await
    .unwrap();
    create_account(
        &mut program_context,
        &pool_token_sos,
        rent,
        TokenAccount::LEN as u64,
        &spl_token_id(),
    )
    .await
    .unwrap();
    create_account_user(
        &mut program_context,
        &user_token_sos,
        rent,
        TokenAccount::LEN as u64,
        &spl_token_id().pubkey(),
        user_wallet,
    )
    .await
    .unwrap();
    create_account_user(
        &mut program_context,
        &user_token_xsos,
        rent,
        TokenAccount::LEN as u64,
        &spl_token_id().pubkey(),
        user_wallet,
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &mint_xsos,
        rent,
        Mint::LEN as u64,
        &spl_token_id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &mint_sos,
        rent,
        Mint::LEN as u64,
        &spl_token_id(),
    )
    .await
    .unwrap();

    let instruction = spl_token::instruction::initialize_mint(
        &spl_token_id().pubkey(),
        &mint_sos.pubkey(),
        &mint_sos_authority.pubkey(),
        None,
        2,
    )
    .unwrap();
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));

    transaction.sign(&[&program_context.payer], program_context.last_blockhash);
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let instruction = spl_token::instruction::initialize_account(
        &spl_token_id().pubkey(),
        &user_token_sos.pubkey(),
        &mint_sos.pubkey(),
        &user_wallet.pubkey(),
    )
    .unwrap();
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));

    transaction.sign(&[&program_context.payer], program_context.last_blockhash);
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let instruction = sol_starter_staking::instruction::initialize_pool(
        &pool.pubkey(),
        &pool_token_sos.pubkey(),
        &mint_sos.pubkey(),
        &mint_xsos.pubkey(),
        InitializePoolInput {
            tier_balance,
            transit_incoming: 3 * 100 * 60,
            transit_outgoing: 3 * 100 * 60,
            ido_authority: Pubkey::find_key_program_address(&ido_market, &crate::program_id()).0,
        },
    )
    .unwrap();

    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));

    transaction.sign(&[&program_context.payer], program_context.last_blockhash);
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let instruction = spl_token::instruction::mint_to(
        &spl_token_id().pubkey(),
        &mint_sos.pubkey(),
        &user_token_sos.pubkey(),
        &mint_sos_authority.pubkey(),
        &[],
        1_000_000,
    )
    .unwrap();
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));

    transaction.sign(
        &[&program_context.payer, &mint_sos_authority],
        program_context.last_blockhash,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let instruction = sol_starter_staking::instruction::stake_start(
        &pool.pubkey(),
        &pool_transit_to.pubkey(),
        &pool_token_sos.pubkey(),
        &pool_transit_to_token.pubkey(),
        &mint_sos.pubkey(),
        &user_wallet.pubkey(),
        &user_token_sos.pubkey(),
        StakeStartInput { amount: 10000 },
    )
    .unwrap();
    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&user_wallet.pubkey()));

    transaction.sign(&[user_wallet], program_context.last_blockhash);
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let instruction = spl_token::instruction::initialize_account(
        &spl_token_id().pubkey(),
        &user_token_xsos.pubkey(),
        &mint_xsos.pubkey(),
        &user_wallet.pubkey(),
    )
    .unwrap();
    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&user_wallet.pubkey()));

    transaction.sign(&[user_wallet], program_context.last_blockhash);
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    warp_seconds(&mut program_context, 3 * 100 * 60).await;

    let transaction = crate::utils::sdk::stake_finish(
        &pool,
        &pool_token_sos,
        &pool_transit_to,
        &pool_transit_to_token,
        &user_token_xsos,
        &user_wallet,
        &mint_xsos,
        &program_context,
    );

    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let instruction = sol_starter_staking::instruction::initialize_lock(
        &pool.pubkey(),
        &user_wallet.pubkey(),
        &mint_xsos.pubkey(),
        &pool_lock_token.pubkey(),
    )
    .unwrap();
    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&user_wallet.pubkey()));

    transaction.sign(&[user_wallet], program_context.last_blockhash);
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let pool_lock_token_key = pool_lock_token.pubkey();
    let transaction = lock_transaction(
        &pool,
        user_wallet,
        pool_lock_token,
        user_token_xsos,
        pool_lock_amount,
        &program_context,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let pool_user_authority = sol_starter_staking::instruction::find_2key_program_address(&pool.pubkey(), &user_wallet.pubkey());
    let pool_lock = Pubkey::create_with_seed(
        &pool_user_authority,
        sol_starter_staking::LOCK_SEED,
        &sol_starter_staking::id(),
    )
    .unwrap();

    (program_context, pool, pool_lock, pool_lock_token_key)
}
