use crate::{
    id,
    instruction::{
        self, InitializePoolInput, LockInput, StakeStartInput, UnlockInput, UnstakeStartInput,
    },
    prelude::*,
    state::{PoolTransit, StakePool},
};
use solana_program::{clock::Clock, program_pack::Pack, pubkey::Pubkey, system_instruction};
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
    transport::TransportError,
};
use spl_token::state::{Account as TokenAccount, Mint};

pub fn program_test() -> ProgramTest {
    let mut program_test = ProgramTest::new(
        "sol_starter_staking",
        id(),
        processor!(crate::processor::Processor::process_instruction),
    );
    program_test.add_program("spl_token", spl_token::id(), None);
    program_test
}

pub async fn get_account(program_context: &mut ProgramTestContext, pubkey: &Pubkey) -> Account {
    program_context
        .banks_client
        .get_account(*pubkey)
        .await
        .expect("account not found")
        .expect("account empty")
}

pub async fn create_account(
    program_context: &mut ProgramTestContext,
    account: &Keypair,
    rent: u64,
    space: u64,
    program_id: &Pubkey,
) -> Result<(), TransportError> {
    let mut transaction = Transaction::new_with_payer(
        &[system_instruction::create_account(
            &program_context.payer.pubkey(),
            &account.pubkey(),
            rent,
            space,
            program_id,
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

//#[tokio::test] // each run of this test eats 0.3 GB of SDD and does not clean ups...
#[allow(dead_code)]
async fn timewarp() {
    let mut program_context = program_test().start_with_context().await;
    let clock = get_clock(&mut program_context).await;

    let now = clock.unix_timestamp;
    warp(&mut program_context, 10).await;
    let clock = get_clock(&mut program_context).await;
    let warped = clock.unix_timestamp;
    assert!(warped - now < 10);

    let before = get_clock(&mut program_context).await.unix_timestamp;
    warp_seconds(&mut program_context, 100 * 60).await;
    let warped = get_clock(&mut program_context).await.unix_timestamp;
    assert!(warped - before > 5500,);
    assert!(warped - before < 6500,);

    let before = get_clock(&mut program_context).await.unix_timestamp;
    warp_seconds(&mut program_context, 100 * 60).await;
    let warped = get_clock(&mut program_context).await.unix_timestamp;
    assert!(warped - before > 5500,);
    assert!(warped - before < 6500,);

    let before = get_clock(&mut program_context).await.unix_timestamp;
    warp_seconds(&mut program_context, 100 * 60).await;
    let warped = get_clock(&mut program_context).await.unix_timestamp;
    assert!(warped - before > 5500,);
    assert!(warped - before < 6500,);

    let before = get_clock(&mut program_context).await.unix_timestamp;
    warp_seconds(&mut program_context, 100 * 60).await;
    let warped = get_clock(&mut program_context).await.unix_timestamp;
    assert!(warped - before > 5500,);
    assert!(warped - before < 6500,);

    let before = get_clock(&mut program_context).await.unix_timestamp;
    warp_seconds(&mut program_context, 100 * 60).await;
    let warped = get_clock(&mut program_context).await.unix_timestamp;
    assert!(warped - before > 5500,);
    assert!(warped - before < 6500,);
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

#[tokio::test]
async fn flow() {
    let mut program_context = program_test().start_with_context().await;
    let rent = &program_context.banks_client.get_rent().await.unwrap();

    let ticks_per_slot = program_context.genesis_config().ticks_per_slot() as i64;

    let pool = Keypair::new();
    let ido_market = Pubkey::new_unique();
    let mint_sos = Keypair::new();
    let mint_sos_authority = Keypair::new();
    let mint_xsos = Keypair::new();
    let pool_token_account_sos = Keypair::new();

    let pool_transit_from = Keypair::new();
    let pool_transit_from_token = Keypair::new();

    let pool_transit_to = Keypair::new();
    let pool_transit_to_token = Keypair::new();

    let user_wallet = Keypair::from_bytes(&program_context.payer.to_bytes()[..]).unwrap();
    let user_token_sos = Keypair::new();
    let user_token_xsos = Keypair::new();

    let rent = rent.minimum_balance(1_000);

    let pool_lock_token_xsos = Keypair::new();

    create_account(
        &mut program_context,
        &pool_lock_token_xsos,
        rent,
        TokenAccount::LEN as u64,
        &spl_token::id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &pool_transit_from,
        rent,
        PoolTransit::LEN as u64,
        &crate::id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &pool_transit_from_token,
        rent,
        TokenAccount::LEN as u64,
        &spl_token::id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &pool_transit_to_token,
        rent,
        TokenAccount::LEN as u64,
        &spl_token::id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &pool_transit_to,
        rent,
        PoolTransit::LEN as u64,
        &crate::id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &pool,
        rent,
        StakePool::LEN as u64,
        &crate::id(),
    )
    .await
    .unwrap();
    create_account(
        &mut program_context,
        &pool_token_account_sos,
        rent,
        TokenAccount::LEN as u64,
        &spl_token::id(),
    )
    .await
    .unwrap();
    create_account(
        &mut program_context,
        &user_token_sos,
        rent,
        TokenAccount::LEN as u64,
        &spl_token::id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &user_token_xsos,
        rent,
        TokenAccount::LEN as u64,
        &spl_token::id(),
    )
    .await
    .unwrap();
    create_account(
        &mut program_context,
        &mint_xsos,
        rent,
        Mint::LEN as u64,
        &spl_token::id(),
    )
    .await
    .unwrap();

    create_account(
        &mut program_context,
        &mint_sos,
        rent,
        Mint::LEN as u64,
        &spl_token::id(),
    )
    .await
    .unwrap();

    let instruction = spl_token::instruction::initialize_mint(
        &spl_token::id(),
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
        &spl_token::id(),
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

    let instruction = instruction::initialize_pool(
        &pool.pubkey(),
        &pool_token_account_sos.pubkey(),
        &mint_sos.pubkey(),
        &mint_xsos.pubkey(),
        InitializePoolInput {
            tier_balance: [1000, 2000, 3000, 4000],
            ido_authority: ido_market,
            transit_incoming: 3 * 100 * 60,
            transit_outgoing: 3 * 100 * 60,
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
        &spl_token::id(),
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

    let instruction = instruction::stake_start(
        &pool.pubkey(),
        &pool_transit_to.pubkey(),
        &pool_token_account_sos.pubkey(),
        &pool_transit_to_token.pubkey(),
        &mint_sos.pubkey(),
        &user_wallet.pubkey(),
        &user_token_sos.pubkey(),
        StakeStartInput { amount: 10000 },
    )
    .unwrap();
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));

    transaction.sign(
        &[&program_context.payer, &user_wallet],
        program_context.last_blockhash,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let instruction = spl_token::instruction::initialize_account(
        &spl_token::id(),
        &user_token_xsos.pubkey(),
        &mint_xsos.pubkey(),
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

    warp_seconds(&mut program_context, 100 * 60).await;

    let transaction = crate::utils::sdk::stake_finish(
        &pool,
        &pool_token_account_sos,
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

    let account_state =
        get_token_account_state(&mut program_context, &pool_token_account_sos).await;
    assert_eq!(account_state.amount, 3410);

    warp_seconds(&mut program_context, 2 * 100 * 60).await;

    let transaction = crate::utils::sdk::stake_finish(
        &pool,
        &pool_token_account_sos,
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

    let account_state =
        get_token_account_state(&mut program_context, &pool_token_account_sos).await;
    assert_eq!(account_state.amount, 10000);

    let instruction = instruction::initialize_lock(
        &pool.pubkey(),
        &user_wallet.pubkey(),
        &mint_xsos.pubkey(),
        &pool_lock_token_xsos.pubkey(),
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

    // lock below tierunw
    let instruction = instruction::lock(
        &pool.pubkey(),
        &user_wallet.pubkey(),
        &pool_lock_token_xsos.pubkey(),
        &user_token_xsos.pubkey(),
        LockInput { amount: 500 },
    )
    .unwrap();
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));

    transaction.sign(
        &[&program_context.payer, &user_wallet],
        program_context.last_blockhash,
    );

    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let account_state = program_context
        .banks_client
        .get_account_data_with_borsh::<StakePool>(pool.pubkey())
        .await
        .unwrap();

    assert_eq!(account_state.tier_users, [0, 0, 0, 0]);

    let account_state = program_context
        .banks_client
        .get_account(pool_lock_token_xsos.pubkey())
        .await
        .unwrap()
        .unwrap();

    let account_state = TokenAccount::unpack_from_slice(&account_state.data[..]).unwrap();
    assert_eq!(account_state.amount, 500);

    // end lock below tier

    // lock more to reach tier
    let instruction = instruction::lock(
        &pool.pubkey(),
        &user_wallet.pubkey(),
        &pool_lock_token_xsos.pubkey(),
        &user_token_xsos.pubkey(),
        LockInput { amount: 2000 },
    )
    .unwrap();
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));

    transaction.sign(
        &[&program_context.payer, &user_wallet],
        program_context.last_blockhash,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let account_state = get_token_account_state(&mut program_context, &pool_lock_token_xsos).await;
    assert_eq!(account_state.amount, 2500);
    let account_state = get_token_account_state(&mut program_context, &user_token_xsos).await;
    assert_eq!(account_state.amount, 7500);

    let account_state = program_context
        .banks_client
        .get_account(pool.pubkey())
        .await
        .unwrap()
        .unwrap();
    let account_state = StakePool::try_from_slice(&account_state.data[..]).unwrap();
    assert_eq!(account_state.tier_users, [0, 1, 0, 0]);
    // end lock to reach tier

    let instruction = instruction::unlock(
        &pool.pubkey(),
        &user_wallet.pubkey(),
        &pool_lock_token_xsos.pubkey(),
        &user_token_xsos.pubkey(),
        UnlockInput { amount: 2500 },
    )
    .unwrap();
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));

    transaction.sign(
        &[&program_context.payer, &user_wallet],
        program_context.last_blockhash,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let account_state = program_context
        .banks_client
        .get_account_data_with_borsh::<StakePool>(pool.pubkey())
        .await
        .unwrap();
    assert_eq!(account_state.tier_users, [0, 0, 0, 0]);

    let account_state =
        get_token_account_state(&mut program_context, &pool_token_account_sos).await;
    assert_eq!(account_state.amount, 10_000);

    let account_state = get_token_account_state(&mut program_context, &pool_lock_token_xsos).await;
    assert_eq!(account_state.amount, 0);

    let account_state = get_token_account_state(&mut program_context, &user_token_xsos).await;
    assert_eq!(account_state.amount, 10_000);

    let instruction = instruction::unstake_start(
        &pool.pubkey(),
        &pool_token_account_sos.pubkey(),
        &pool_transit_from.pubkey(),
        &pool_transit_from_token.pubkey(),
        &mint_sos.pubkey(),
        &user_wallet.pubkey(),
        &user_token_xsos.pubkey(),
        &mint_xsos.pubkey(),
        UnstakeStartInput { amount: 420 },
    )
    .unwrap();
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));

    transaction.sign(
        &[&program_context.payer, &user_wallet],
        program_context.last_blockhash,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let account_state = program_context
        .banks_client
        .get_account_data_with_borsh::<PoolTransit>(pool_transit_from.pubkey())
        .await
        .unwrap();

    assert!(account_state.transit_from < account_state.transit_until - 5 * ticks_per_slot as i64);

    let account_state =
        get_token_account_state(&mut program_context, &pool_transit_from_token).await;
    assert_eq!(account_state.amount, 420);

    let account_state =
        get_token_account_state(&mut program_context, &pool_token_account_sos).await;
    assert_eq!(account_state.amount, 10000 - 420);

    let account_state = get_token_account_state(&mut program_context, &user_token_xsos).await;
    assert_eq!(account_state.amount, 10000 - 420);

    let transaction = unstake_finish(
        &pool,
        &pool_transit_from,
        &pool_transit_from_token,
        &user_wallet,
        &user_token_sos,
        &program_context,
    );

    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap_err();

    warp_seconds(&mut program_context, 100 * 60).await;

    let transaction = unstake_finish(
        &pool,
        &pool_transit_from,
        &pool_transit_from_token,
        &user_wallet,
        &user_token_sos,
        &program_context,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let account_state = get_token_account_state(&mut program_context, &user_token_sos).await;

    assert_eq!(account_state.amount, 990143);

    warp_seconds(&mut program_context, 2 * 100 * 60).await;

    let transaction = unstake_finish(
        &pool,
        &pool_transit_from,
        &pool_transit_from_token,
        &user_wallet,
        &user_token_sos,
        &program_context,
    );
    program_context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    let account_state = get_token_account_state(&mut program_context, &user_token_sos).await;

    assert_eq!(account_state.amount, 990420);
}

async fn get_token_account_state(
    program_context: &mut ProgramTestContext,
    token: &Keypair,
) -> TokenAccount {
    let data = get_account(program_context, &token.pubkey()).await;
    TokenAccount::unpack_from_slice(&data.data[..]).unwrap()
}

fn unstake_finish(
    pool: &Keypair,
    pool_transit_from: &Keypair,
    pool_transit_from_token: &Keypair,
    user_wallet: &Keypair,
    user_token_sos: &Keypair,
    program_context: &ProgramTestContext,
) -> Transaction {
    let instruction = instruction::unstake_finish(
        &pool.pubkey(),
        &pool_transit_from.pubkey(),
        &pool_transit_from_token.pubkey(),
        &user_wallet.pubkey(),
        &user_token_sos.pubkey(),
    )
    .unwrap();
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&program_context.payer.pubkey()));
    transaction.sign(
        &[&program_context.payer, user_wallet],
        program_context.last_blockhash,
    );
    transaction
}
