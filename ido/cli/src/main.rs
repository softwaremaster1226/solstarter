use clap::{
    crate_description, crate_name, crate_version, value_t, value_t_or_exit, App, AppSettings, Arg,
    SubCommand,
};
use sol_starter_ido::{
    instruction::{
        add_to_whitelist, initialize_market, initialize_pool, participate, start_pool, withdraw,
        InitializeMarket, InitializePool, Participate,
    },
    state::{Market, MintWhitelist, Pool},
};
use sol_starter_staking::{
    instruction::initialize_lock, instruction::initialize_pool as initialize_stake_pool,
    instruction::InitializePoolInput as InitializeStakePoolInput, state::StakePool, TIERS_COUNT,
};

use borsh::BorshDeserialize;
use regex::Regex;
use serde::Deserialize;
use solana_clap_utils::{
    input_parsers::pubkey_of,
    input_validators::{is_keypair, is_parsable, is_pubkey, is_url},
    keypair::signer_from_path,
};
use solana_client::rpc_client::RpcClient;
use solana_program::{
    clock::UnixTimestamp, instruction::Instruction, program_pack::Pack, pubkey::Pubkey,
    system_instruction::create_account_with_seed,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    native_token::lamports_to_sol,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_token::state::{Account as TokenAccount, Mint};
use std::{process::exit, str::FromStr};

#[allow(dead_code)]
struct Config {
    rpc_client: RpcClient,
    verbose: bool,
    owner: Box<dyn Signer>,
    fee_payer: Box<dyn Signer>,
    commitment_config: CommitmentConfig,
}

type Error = Box<dyn std::error::Error>;
type CommandResult = Result<Option<Transaction>, Error>;

#[derive(Debug, Deserialize)]
struct Record {
    wallet: String,
    whitelist_token_acc: String,
}

impl Record {
    fn process_record(
        &self,
        instructions: &mut Vec<Instruction>,
        config: &Config,
        pool: &Pubkey,
        mint_whitelist: &Pubkey,
    ) -> Result<(), Error> {
        if self.wallet.is_empty() {
            return Err("Wallet account is missing in file".into());
        }
        let wallet_key = Pubkey::from_str(&self.wallet)?;
        let whitelist_key;

        if self.whitelist_token_acc.is_empty() {
            let calculated_key = spl_associated_token_account::get_associated_token_address(
                &wallet_key,
                mint_whitelist,
            );

            if !token_account_initialized(config, &calculated_key) {
                println!("Will be created token account: {:?}", calculated_key);
                instructions.push(
                    spl_associated_token_account::create_associated_token_account(
                        &config.fee_payer.pubkey(),
                        &wallet_key,
                        mint_whitelist,
                    ),
                );
            } else {
                is_mint_right(config, &calculated_key, mint_whitelist)?;
            }
            whitelist_key = calculated_key;
        } else {
            let key = Pubkey::from_str(&self.whitelist_token_acc)?;
            is_mint_right(config, &key, mint_whitelist)?;
            whitelist_key = key;
        }

        instructions.push(add_to_whitelist(
            &sol_starter_ido::program_id(),
            pool,
            &config.owner.pubkey(),
            &whitelist_key,
            mint_whitelist,
        )?);

        Ok(())
    }
}

fn check_fee_payer_balance(config: &Config, required_balance: u64) -> Result<(), Error> {
    let balance = config.rpc_client.get_balance(&config.fee_payer.pubkey())?;
    if balance < required_balance {
        Err(format!(
            "Fee payer, {}, has insufficient balance: {} required, {} available",
            config.fee_payer.pubkey(),
            lamports_to_sol(required_balance),
            lamports_to_sol(balance)
        )
        .into())
    } else {
        Ok(())
    }
}

fn create_pool_lock_account(
    config: &Config,
    instructions: &mut Vec<Instruction>,
    stake_pool: &Pubkey,
    mint_xsos: &Pubkey,
) -> Result<Pubkey, Error> {
    let pool_lock_seed = "pool_lock_key";
    let key_to_create =
        Pubkey::create_with_seed(&config.owner.pubkey(), pool_lock_seed, &spl_token::id())?;

    let lock_acc_data = config.rpc_client.get_account_data(&key_to_create)?;
    if lock_acc_data.is_empty() {
        println!(
            "New lock token account will be created and initialized: {:?}",
            key_to_create
        );

        let token_account_balance = config
            .rpc_client
            .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)?;

        instructions.push(create_account_with_seed(
            &config.fee_payer.pubkey(),
            &key_to_create,
            &config.owner.pubkey(),
            pool_lock_seed,
            token_account_balance,
            TokenAccount::LEN as u64,
            &spl_token::id(),
        ));

        instructions.push(initialize_lock(
            stake_pool,
            &config.owner.pubkey(),
            mint_xsos,
            &key_to_create,
        )?);
    }

    Ok(key_to_create)
}

fn ui_to_tokens(value: f64, precision: u64) -> u64 {
    (value * precision as f64).round() as u64
}

fn tokens_to_ui(value: u64, precision: u64) -> f64 {
    (value / precision) as f64
}

fn is_csv_file(s: String) -> Result<(), String> {
    let re = Regex::new(r".+\.csv$").unwrap();
    if re.is_match(s.as_ref()) {
        return Ok(());
    }
    Err(String::from("Receive wrong path to csv file"))
}

fn calculate_and_create_associated_key(
    config: &Config,
    mint: &Pubkey,
    instructions: &mut Vec<Instruction>,
) -> Pubkey {
    let calculated_key =
        spl_associated_token_account::get_associated_token_address(&config.owner.pubkey(), &mint);

    if !token_account_initialized(config, &calculated_key) {
        println!(
            "New associated token account was created: {:?}",
            calculated_key
        );
        instructions.push(
            spl_associated_token_account::create_associated_token_account(
                &config.fee_payer.pubkey(),
                &config.owner.pubkey(),
                &mint,
            ),
        );
    }
    calculated_key
}

fn token_account_initialized(config: &Config, key: &Pubkey) -> bool {
    let token_acc_data = config.rpc_client.get_account_data(&key).ok();
    if let Some(acc_data) = token_acc_data {
        let token_acc = TokenAccount::unpack(acc_data.as_slice());

        if token_acc.is_ok() {
            return true;
        }
    }
    false
}

fn is_mint_right(config: &Config, token_key: &Pubkey, mint: &Pubkey) -> Result<(), Error> {
    let token_acc_data = config.rpc_client.get_account_data(token_key)?;
    let token_acc = TokenAccount::unpack(token_acc_data.as_slice())?;
    if token_acc.mint != *mint {
        return Err("Wrong mint in whitelist token account".into());
    }
    Ok(())
}

fn command_create_market(
    config: &Config,
    stake_token: Pubkey,
    transit_incoming: UnixTimestamp,
    transit_outgoing: UnixTimestamp,
    tier_balance: [u64; TIERS_COUNT],
) -> CommandResult {
    let mut instructions = vec![];
    let mut required_balance: u64 = 0;

    let stake_pool_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(StakePool::LEN)?;
    let market_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Market::LEN)?;
    let token_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)?;
    let mint_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Mint::LEN)?;

    // Creating market account
    let market_account = Keypair::new();
    println!("IDO market account: {:?}", market_account.pubkey());
    instructions.push(system_instruction::create_account(
        &config.fee_payer.pubkey(),
        &market_account.pubkey(),
        market_account_balance,
        Market::LEN as u64,
        &sol_starter_ido::id(),
    ));
    required_balance += market_account_balance;

    // Creating stake pool account
    let stake_pool_account = Keypair::new();
    println!("Stake pool account: {:?}", stake_pool_account.pubkey());
    instructions.push(system_instruction::create_account(
        &config.fee_payer.pubkey(),
        &stake_pool_account.pubkey(),
        stake_pool_account_balance,
        StakePool::LEN as u64,
        &sol_starter_staking::id(),
    ));
    required_balance += stake_pool_account_balance;

    // Creating stake pool mint
    let stake_mint_account = Keypair::new();
    println!("Stake pool mint: {:?}", stake_mint_account.pubkey());
    instructions.push(system_instruction::create_account(
        &config.fee_payer.pubkey(),
        &stake_mint_account.pubkey(),
        mint_account_balance,
        Mint::LEN as u64,
        &spl_token::id(),
    ));
    required_balance += mint_account_balance;

    // Creating stake pool token account
    let stake_token_account = Keypair::new();
    println!(
        "Stake pool token account: {:?}",
        stake_token_account.pubkey()
    );
    instructions.push(system_instruction::create_account(
        &config.fee_payer.pubkey(),
        &stake_token_account.pubkey(),
        token_account_balance,
        TokenAccount::LEN as u64,
        &spl_token::id(),
    ));
    required_balance += token_account_balance;

    // Initializing stake pool
    instructions.push(initialize_stake_pool(
        &stake_pool_account.pubkey(),
        &stake_token_account.pubkey(),
        &stake_token,
        &stake_mint_account.pubkey(),
        InitializeStakePoolInput {
            tier_balance,
            ido_authority: Pubkey::find_program_address(
                &[&market_account.to_bytes()[..32]],
                &sol_starter_ido::id(),
            )
            .0,
            transit_incoming,
            transit_outgoing,
        },
    )?);

    // Initialize market account
    instructions.push(initialize_market(
        &sol_starter_ido::program_id(),
        &market_account.pubkey(),
        &config.owner.pubkey(),
        InitializeMarket {
            stake_pool: stake_pool_account.pubkey(),
        },
    )?);

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        required_balance + fee_calculator.calculate_fee(&transaction.message()),
    )?;
    let signers = vec![
        config.fee_payer.as_ref(),
        &market_account,
        &stake_pool_account,
        &stake_mint_account,
        &stake_token_account,
        config.owner.as_ref(),
    ];
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_create_pool(
    config: &Config,
    market: &Pubkey,
    mint_collection: &Pubkey,
    mint_distribution: &Pubkey,
    init_args: InitializePool,
    is_whitelist: bool,
) -> CommandResult {
    let mut instructions = vec![];
    let mut required_balance: u64 = 0;

    // Query minimum rent-exempt balances
    let pool_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Pool::LEN)?;
    let token_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)?;
    let mint_account_balance = config
        .rpc_client
        .get_minimum_balance_for_rent_exemption(Mint::LEN)?;

    // Create account for the pool
    let pool_keypair = Keypair::new();
    println!("IDO pool account: {:?}", pool_keypair.pubkey());
    instructions.push(system_instruction::create_account(
        &config.fee_payer.pubkey(),
        &pool_keypair.pubkey(),
        pool_account_balance,
        Pool::LEN as u64,
        &sol_starter_ido::id(),
    ));
    required_balance += pool_account_balance;

    // Create account for token collection
    let account_collection_keypair = Keypair::new();
    println!(
        "Token collection account: {:?}",
        account_collection_keypair.pubkey()
    );
    instructions.push(system_instruction::create_account(
        &config.fee_payer.pubkey(),
        &account_collection_keypair.pubkey(),
        token_account_balance,
        TokenAccount::LEN as u64,
        &spl_token::id(),
    ));
    required_balance += token_account_balance;

    // Create account for token distribution
    let account_distribution_keypair = Keypair::new();
    println!(
        "Token distribution account: {:?}",
        account_distribution_keypair.pubkey()
    );
    instructions.push(system_instruction::create_account(
        &config.fee_payer.pubkey(),
        &account_distribution_keypair.pubkey(),
        token_account_balance,
        TokenAccount::LEN as u64,
        &spl_token::id(),
    ));
    required_balance += token_account_balance;

    // Create account for the pool mint
    let pool_mint_keypair = Keypair::new();
    println!("Pool mint account: {:?}", pool_mint_keypair.pubkey());
    instructions.push(system_instruction::create_account(
        &config.fee_payer.pubkey(),
        &pool_mint_keypair.pubkey(),
        mint_account_balance,
        Mint::LEN as u64,
        &spl_token::id(),
    ));
    required_balance += mint_account_balance;

    // (Optional) Create account for the whitelist mint
    let whitelist_mint_keypair = Keypair::new();
    let mint_whitelist = if is_whitelist {
        println!(
            "Whitelist mint account: {:?}",
            whitelist_mint_keypair.pubkey()
        );
        instructions.push(system_instruction::create_account(
            &config.fee_payer.pubkey(),
            &whitelist_mint_keypair.pubkey(),
            mint_account_balance,
            Mint::LEN as u64,
            &spl_token::id(),
        ));
        required_balance += mint_account_balance;
        Some(whitelist_mint_keypair.pubkey())
    } else {
        None
    };

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(
        config,
        required_balance + fee_calculator.calculate_fee(&transaction.message()),
    )?;
    let mut signers = vec![
        config.fee_payer.as_ref(),
        &pool_keypair,
        &account_collection_keypair,
        &account_distribution_keypair,
        &pool_mint_keypair,
    ];
    if mint_whitelist.is_some() {
        signers.push(&whitelist_mint_keypair);
    }
    transaction.sign(&signers, recent_blockhash);

    let signature = config
        .rpc_client
        .send_and_confirm_transaction_with_spinner_and_commitment(
            &transaction,
            config.commitment_config,
        )?;
    println!(
        "Tx hash of preparation signature with accounts creation: {:?}",
        signature
    );

    instructions.clear();
    // Initialize pool
    instructions.push(initialize_pool(
        &sol_starter_ido::program_id(),
        &pool_keypair.pubkey(),
        market,
        &config.owner.pubkey(),
        mint_collection,
        mint_distribution,
        &account_collection_keypair.pubkey(),
        &account_distribution_keypair.pubkey(),
        &pool_mint_keypair.pubkey(),
        mint_whitelist,
        init_args,
    )?);

    let mut transaction =
        Transaction::new_with_payer(&instructions, Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;
    let signers = vec![config.fee_payer.as_ref(), config.owner.as_ref()];
    transaction.sign(&signers, recent_blockhash);
    Ok(Some(transaction))
}

fn command_start_pool(config: &Config, market: &Pubkey, pool_to_start: &Pubkey) -> CommandResult {
    let market_data = config.rpc_client.get_account_data(market)?;
    let market_data = Market::try_from_slice(market_data.as_slice())?;

    let mut transaction = Transaction::new_with_payer(
        &[start_pool(
            &sol_starter_ido::program_id(),
            &config.owner.pubkey(),
            &market_data.stake_pool,
            market,
            pool_to_start,
        )
        .unwrap()],
        Some(&config.fee_payer.pubkey()),
    );

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;

    transaction.sign(
        &[config.fee_payer.as_ref(), config.owner.as_ref()],
        recent_blockhash,
    );

    Ok(Some(transaction))
}

fn command_add_to_whitelist(config: &Config, pool: &Pubkey, whitelist_accs: &str) -> CommandResult {
    let pool_data = config.rpc_client.get_account_data(pool)?;
    let pool_data = Pool::try_from_slice(pool_data.as_slice())?;

    let whitelist_mint;

    if let MintWhitelist::Key(pool_whitelist_mint) = pool_data.mint_whitelist {
        whitelist_mint = pool_whitelist_mint;
    } else {
        return Err("Pool doesn't have mint whitelist".into());
    }

    let max_process_per_tx = 10;
    let mut all_instructions: Vec<Vec<Instruction>> = Vec::new();
    let mut instructions_fraction: Vec<Instruction> = Vec::new();

    let mut rdr = csv::Reader::from_path(whitelist_accs)?;

    for result in rdr.deserialize().enumerate() {
        let record: Record = result.1?;
        if (result.0 + 1) % max_process_per_tx == 0 {
            all_instructions.push(instructions_fraction.clone());
            instructions_fraction.clear();
        }
        record.process_record(&mut instructions_fraction, config, pool, &whitelist_mint)?;
    }
    all_instructions.push(instructions_fraction);

    println!("Will be sent {:?} transaction(s)", all_instructions.len());

    for instructions_set in all_instructions.iter().enumerate() {
        let mut transaction = Transaction::new_with_payer(
            instructions_set.1.as_ref(),
            Some(&config.fee_payer.pubkey()),
        );
        let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
        check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;

        transaction.sign(
            &[config.fee_payer.as_ref(), config.owner.as_ref()],
            recent_blockhash,
        );

        let signature = config
            .rpc_client
            .send_and_confirm_transaction_with_spinner_and_commitment(
                &transaction,
                config.commitment_config,
            )?;

        println!(
            "Hash of {:?} transaction: {:?}",
            instructions_set.0 + 1,
            signature
        );
    }

    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn command_participate(
    config: &Config,
    pool: &Pubkey,
    user_acc_from: &Pubkey,
    user_acc_to: &Pubkey,
    amount: u64,
    stage: u8,
    pool_lock_token: Option<Pubkey>,
    market_user_kyc: Option<Pubkey>,
    account_whitelist: Option<Pubkey>,
) -> CommandResult {
    let mut instructions: Vec<Instruction> = Vec::new();

    let pool_data = config.rpc_client.get_account_data(pool)?;
    let pool_data = Pool::try_from_slice(pool_data.as_slice())?;

    let market_data = config.rpc_client.get_account_data(&pool_data.market)?;
    let market_data = Market::try_from_slice(market_data.as_slice())?;

    let stake_pool_data = config
        .rpc_client
        .get_account_data(&market_data.stake_pool)?;
    let stake_pool_data = StakePool::try_from_slice(stake_pool_data.as_slice())?;

    let pool_lock_token = pool_lock_token.unwrap_or(create_pool_lock_account(
        config,
        &mut instructions,
        &market_data.stake_pool,
        &stake_pool_data.pool_mint_xsos,
    )?);

    let pool_user_authority = Pubkey::find_program_address(
        &[
            &market_data.stake_pool.to_bytes()[..32],
            &config.owner.pubkey().to_bytes()[..32],
        ],
        &sol_starter_ido::id(),
    )
    .0;
    let pool_lock = Pubkey::create_with_seed(
        &pool_user_authority,
        sol_starter_staking::LOCK_SEED,
        &sol_starter_staking::id(),
    )?;

    let mint_whitelist;

    if let MintWhitelist::Key(k) = pool_data.mint_whitelist {
        mint_whitelist = k;
    } else {
        mint_whitelist = Pubkey::default();
    };

    let market_user_kyc = market_user_kyc.unwrap_or_default();
    let account_whitelist = account_whitelist.unwrap_or_default();

    instructions.push(participate(
        &sol_starter_ido::program_id(),
        pool,
        &pool_data.market,
        &config.owner.pubkey(),
        user_acc_from,
        &pool_data.account_collection,
        user_acc_to,
        &pool_lock_token,
        &pool_data.mint_pool,
        &pool_lock,
        &market_data.stake_pool,
        if market_user_kyc != Pubkey::default() {
            Some(&market_user_kyc)
        } else {
            None
        },
        if account_whitelist != Pubkey::default() {
            Some(&account_whitelist)
        } else {
            None
        },
        if mint_whitelist != Pubkey::default() {
            Some(&mint_whitelist)
        } else {
            None
        },
        Participate { amount },
        stage,
    )?);

    let mut transaction =
        Transaction::new_with_payer(instructions.as_ref(), Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;

    transaction.sign(
        &[config.fee_payer.as_ref(), config.owner.as_ref()],
        recent_blockhash,
    );

    Ok(Some(transaction))
}

fn command_withdraw(
    config: &Config,
    pool: &Pubkey,
    account_from: &Pubkey,
    account_to: Option<Pubkey>,
) -> CommandResult {
    let pool_data = config.rpc_client.get_account_data(pool)?;
    let pool_data = Pool::try_from_slice(pool_data.as_slice())?;

    let acc_from_data = config.rpc_client.get_account_data(account_from)?;
    let acc_from_data = TokenAccount::unpack(acc_from_data.as_slice())?;

    let mut instructions: Vec<Instruction> = Vec::new();

    let account_to = account_to.unwrap_or_else(|| {
        calculate_and_create_associated_key(config, &acc_from_data.mint, &mut instructions)
    });

    instructions.push(withdraw(
        &sol_starter_ido::program_id(),
        pool,
        &pool_data.market,
        &config.owner.pubkey(),
        account_from,
        &account_to,
    )?);

    let mut transaction =
        Transaction::new_with_payer(instructions.as_ref(), Some(&config.fee_payer.pubkey()));

    let (recent_blockhash, fee_calculator) = config.rpc_client.get_recent_blockhash()?;
    check_fee_payer_balance(config, fee_calculator.calculate_fee(&transaction.message()))?;

    transaction.sign(
        &[config.fee_payer.as_ref(), config.owner.as_ref()],
        recent_blockhash,
    );

    Ok(Some(transaction))
}

fn command_pool_info(config: &Config, pool: &Pubkey) -> CommandResult {
    let pool_data = config.rpc_client.get_account_data(pool)?;
    let pool_data = Pool::try_from_slice(pool_data.as_slice())?;

    println!(
        "\nData version: {:?}
        \nMarket: {:?}
        \nToken account for tokens used as investment: {:?}
        \nToken account for tokens to be distributed: {:?}
        \nMint for the pool tokens (minted on purchase): {:?}
        \nMint whitelist: {:?}
        \nKYC requirement: {:?}
        \nPrice: {:?}
        \nMaximum amount to be collected: {:?}
        \nMinimum amount of be collected: {:?}
        \nMin investment size: {:?}
        \nMax investment size: {:?}
        \nTime when the pool starts accepting investments: {:?}
        \nTime when the pool stops accepting investments (and starts token distribution): {:?}
        \nAmount collected: {:?}
        \nAmount to distribute in distribution tokens: {:?}
        \nPool owner: {:?}
        \nPool authority: {:?}
        \nStores amounts available for each user tier: {:?}
        \nTotal allocations for each tier: {:?}
        \nNon overlapped time for stages: {:?}",
        pool_data.version,
        pool_data.market,
        pool_data.account_collection,
        pool_data.account_distribution,
        pool_data.mint_pool,
        pool_data.mint_whitelist,
        pool_data.kyc_requirement,
        tokens_to_ui(pool_data.price, Pool::PRECISION),
        tokens_to_ui(pool_data.goal_max_collected, Pool::PRECISION),
        tokens_to_ui(pool_data.goal_min_collected, Pool::PRECISION),
        tokens_to_ui(pool_data.amount_investment_min, Pool::PRECISION),
        tokens_to_ui(pool_data.amount_investment_max, Pool::PRECISION),
        pool_data.time_start,
        pool_data.time_finish,
        tokens_to_ui(pool_data.amount_collected, Pool::PRECISION),
        tokens_to_ui(pool_data.amount_to_distribute, Pool::PRECISION),
        pool_data.owner,
        pool_data.authority,
        pool_data.tier_allocation,
        pool_data.tier_remaining,
        pool_data.time_table,
    );

    Ok(None)
}

fn main() {
    let matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg({
            let arg = Arg::with_name("config_file")
                .short("C")
                .long("config")
                .value_name("PATH")
                .takes_value(true)
                .global(true)
                .help("Configuration file to use");
            if let Some(ref config_file) = *solana_cli_config::CONFIG_FILE {
                arg.default_value(&config_file)
            } else {
                arg
            }
        })
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .takes_value(false)
                .global(true)
                .help("Show additional information"),
        )
        .arg(
            Arg::with_name("json_rpc_url")
                .long("url")
                .value_name("URL")
                .takes_value(true)
                .validator(is_url)
                .help("JSON RPC URL for the cluster.  Default from the configuration file."),
        )
        .arg(
            Arg::with_name("owner")
                .long("owner")
                .value_name("KEYPAIR")
                .validator(is_keypair)
                .takes_value(true)
                .help(
                    "Specify the market/pool's owner. \
                     This may be a keypair file, the ASK keyword. \
                     Defaults to the client keypair.",
                ),
        )
        .arg(
            Arg::with_name("fee_payer")
                .long("fee-payer")
                .value_name("KEYPAIR")
                .validator(is_keypair)
                .takes_value(true)
                .help(
                    "Specify the fee-payer account. \
                     This may be a keypair file, the ASK keyword. \
                     Defaults to the client keypair.",
                ),
        )
        .subcommand(
            SubCommand::with_name("create-market").about("Create a new market")
            .arg(
                Arg::with_name("stake_token")
                    .long("stake-token")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Token mint account to be used for staking."),
            )
            .arg(
                Arg::with_name("lock_in")
                    .long("lock-in")
                    .validator(is_parsable::<UnixTimestamp>)
                    .value_name("SECONDS")
                    .takes_value(true)
                    .default_value("0")
                    .help("Token lock interval when staking."),
            )
            .arg(
                Arg::with_name("lock_out")
                    .long("lock-out")
                    .validator(is_parsable::<UnixTimestamp>)
                    .value_name("SECONDS")
                    .takes_value(true)
                    .default_value("0")
                    .help("Token lock interval when unstaking."),
            )
            .arg(
                Arg::with_name("tier_1")
                    .long("tier-1")
                    .validator(is_parsable::<f64>)
                    .value_name("AMOUNT")
                    .takes_value(true)
                    .required(true)
                    .help("Staking balance qualifying for the tier 1 (lowest)."),
            )
            .arg(
                Arg::with_name("tier_2")
                    .long("tier-2")
                    .validator(is_parsable::<f64>)
                    .value_name("AMOUNT")
                    .takes_value(true)
                    .required(true)
                    .help("Staking balance qualifying for the tier 2."),
            )
            .arg(
                Arg::with_name("tier_3")
                    .long("tier-3")
                    .validator(is_parsable::<f64>)
                    .value_name("AMOUNT")
                    .takes_value(true)
                    .required(true)
                    .help("Staking balance qualifying for the tier 3."),
            )
            .arg(
                Arg::with_name("tier_4")
                    .long("tier-4")
                    .validator(is_parsable::<f64>)
                    .value_name("AMOUNT")
                    .takes_value(true)
                    .required(true)
                    .help("Staking balance qualifying for the tier 4 (highest)."),
            )
        )
        .subcommand(
            SubCommand::with_name("create-pool")
                .about("Create a new pool")
                .arg(
                    Arg::with_name("market")
                        .long("market")
                        .validator(is_pubkey)
                        .value_name("ADDRESS")
                        .takes_value(true)
                        .required(true)
                        .help("Initialized IDO market account."),
                )
                .arg(
                    Arg::with_name("mint_collection")
                        .long("mint-collection")
                        .validator(is_pubkey)
                        .value_name("ADDRESS")
                        .takes_value(true)
                        .required(true)
                        .help("Mint of the tokens which pool will collect."),
                )
                .arg(
                    Arg::with_name("mint_distribution")
                        .long("mint-distribution")
                        .validator(is_pubkey)
                        .value_name("ADDRESS")
                        .takes_value(true)
                        .required(true)
                        .help("Mint of the tokens which pool will distribute."),
                )
                .arg(
                    Arg::with_name("pool_owner")
                        .long("pool-owner")
                        .validator(is_pubkey)
                        .value_name("ADDRESS")
                        .takes_value(true)
                        .required(true)
                        .help("Owner of the pool, able to issue whitelist tokens and withdraw funds."),
                )
                .arg(
                    Arg::with_name("is_whitelist")
                        .long("is-whitelist")
                        .validator(is_parsable::<bool>)
                        .value_name("BOOLEAN")
                        .takes_value(true)
                        .required(true)
                        .help("Should be created mint_whitelist or not."),
                )
                .arg(
                    Arg::with_name("is_kyc")
                        .long("is-kyc")
                        .validator(is_parsable::<bool>)
                        .value_name("BOOLEAN")
                        .takes_value(true)
                        .required(true)
                        .help("Should IDO be KYC-only."),
                )
                .arg(
                    Arg::with_name("price")
                        .long("price")
                        .validator(is_parsable::<f64>)
                        .value_name("VALUE")
                        .takes_value(true)
                        .required(true)
                        .help("Distributed tokens price."),
                )
                .arg(
                    Arg::with_name("goal_max")
                        .long("goal-max")
                        .validator(is_parsable::<f64>)
                        .value_name("AMOUNT")
                        .takes_value(true)
                        .required(true)
                        .help("IDO maximum goal in collection tokens."),
                )
                .arg(
                    Arg::with_name("goal_min")
                        .long("goal-min")
                        .validator(is_parsable::<f64>)
                        .value_name("AMOUNT")
                        .takes_value(true)
                        .required(true)
                        .help("IDO minimum goal in collection tokens."),
                )
                .arg(
                    Arg::with_name("amount_min")
                        .long("amount-min")
                        .validator(is_parsable::<f64>)
                        .value_name("AMOUNT")
                        .takes_value(true)
                        .required(true)
                        .help("Min investment size in collection tokens."),
                )
                .arg(
                    Arg::with_name("amount_max")
                        .long("amount-max")
                        .validator(is_parsable::<f64>)
                        .value_name("AMOUNT")
                        .takes_value(true)
                        .required(true)
                        .help("Max investment size in collection tokens."),
                )
                .arg(
                    Arg::with_name("time_start")
                        .long("time-start")
                        .validator(is_parsable::<UnixTimestamp>)
                        .value_name("SECONDS")
                        .takes_value(true)
                        .required(true)
                        .help("Time when the pool starts accepting investments, unix timestamp."),
                )
                .arg(
                    Arg::with_name("time_finish")
                        .long("time-finish")
                        .validator(is_parsable::<UnixTimestamp>)
                        .value_name("SECONDS")
                        .takes_value(true)
                        .required(true)
                        .help("Time when the pool stops accepting investments (and starts token distribution), unix timestamp."),
                )
                .arg(
                    Arg::with_name("stage_1")
                        .long("stage-1")
                        .validator(is_parsable::<u32>)
                        .value_name("SECONDS")
                        .takes_value(true)
                        .required(true)
                        .help("Length of the first IDO stage (individual user allocations), in seconds."),
                )
                .arg(
                    Arg::with_name("stage_2")
                        .long("stage-2")
                        .validator(is_parsable::<u32>)
                        .value_name("SECONDS")
                        .takes_value(true)
                        .required(true)
                        .help("Length of the second IDO stage (tier allocations), in seconds."),
                )
        )
        .subcommand(
            SubCommand::with_name("start-pool")
                .about("Start a new pool")
                .arg(
                    Arg::with_name("market")
                        .long("market")
                        .validator(is_pubkey)
                        .value_name("ADDRESS")
                        .takes_value(true)
                        .required(true)
                        .help("Initialized IDO market account."),
                )
                .arg(
                    Arg::with_name("pool")
                    .long("pool")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Pool to start."),
                )
        )
        .subcommand(
            SubCommand::with_name("add-to-whitelist")
                .about("Add particular users to the pool whitelist")
                .arg(
                    Arg::with_name("pool")
                        .long("pool")
                        .validator(is_pubkey)
                        .value_name("ADDRESS")
                        .takes_value(true)
                        .required(true)
                        .help("Initialized IDO pool account."),
                )
                .arg(
                    Arg::with_name("whitelist-accounts")
                    .long("whitelist-accs")
                    .validator(is_csv_file)
                    .value_name("PATH")
                    .takes_value(true)
                    .required(true)
                    .help("CSV file with whitelist token accounts mint tokens to."),
                )
        )
        .subcommand(
            SubCommand::with_name("participate")
                .about("Participate in pool by sending collection tokens")
                .arg(
                    Arg::with_name("pool")
                        .long("pool")
                        .validator(is_pubkey)
                        .value_name("ADDRESS")
                        .takes_value(true)
                        .required(true)
                        .help("Initialized IDO pool account."),
                )
                .arg(
                    Arg::with_name("user-acc-from")
                    .long("user-acc-from")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Account sending collected token from the user to the pool."),
                )
                .arg(
                    Arg::with_name("user-acc-to")
                    .long("user-acc-to")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Token account to receive back pool tokens."),
                )
                .arg(
                    Arg::with_name("amount")
                        .long("amount")
                        .validator(is_parsable::<f64>)
                        .value_name("AMOUNT")
                        .takes_value(true)
                        .required(true)
                        .help("Amount of collected tokens to transfer to the pool."),
                )
                .arg(
                    Arg::with_name("stage")
                        .long("stage")
                        .validator(is_parsable::<u8>)
                        .value_name("NUMBER")
                        .takes_value(true)
                        .required(true)
                        .help("Stage."),
                )
                .arg(
                    Arg::with_name("pool-lock-token")
                    .long("pool-lock-token")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Pool lock token."),
                )
                .arg(
                    Arg::with_name("market-user-kyc")
                    .long("market-user-kyc")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("User KYC account."),
                )
                .arg(
                    Arg::with_name("account-whitelist")
                    .long("account-whitelist")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("User whitelist token account."),
                )
        )
        .subcommand(
            SubCommand::with_name("withdraw")
                .about("Collect leftover distributed or collected tokens after pool is over.")
                .arg(
                    Arg::with_name("pool")
                        .long("pool")
                        .validator(is_pubkey)
                        .value_name("ADDRESS")
                        .takes_value(true)
                        .required(true)
                        .help("Initialized IDO pool account."),
                )
                .arg(
                    Arg::with_name("account-from")
                    .long("account-from")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .required(true)
                    .help("Account to collect funds from. Should be pool's collection or distribution token account"),
                )
                .arg(
                    Arg::with_name("account-to")
                    .long("account-to")
                    .validator(is_pubkey)
                    .value_name("ADDRESS")
                    .takes_value(true)
                    .help("Pool owner's token account to receive tokens from the previous account (either collected or distributed token)"),
                )
        )
        .subcommand(
            SubCommand::with_name("pool-info")
                .about("Get pool information.")
                .arg(
                    Arg::with_name("pool")
                        .validator(is_pubkey)
                        .value_name("ADDRESS")
                        .takes_value(true)
                        .required(true)
                        .help("Initialized IDO pool account."),
                )
        )
        .get_matches();

    let mut wallet_manager = None;
    let config = {
        let cli_config = if let Some(config_file) = matches.value_of("config_file") {
            solana_cli_config::Config::load(config_file).unwrap_or_default()
        } else {
            solana_cli_config::Config::default()
        };
        let json_rpc_url = value_t!(matches, "json_rpc_url", String)
            .unwrap_or_else(|_| cli_config.json_rpc_url.clone());

        let owner = signer_from_path(
            &matches,
            &cli_config.keypair_path,
            "owner",
            &mut wallet_manager,
        )
        .unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });
        let fee_payer = signer_from_path(
            &matches,
            &cli_config.keypair_path,
            "fee_payer",
            &mut wallet_manager,
        )
        .unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });
        let verbose = matches.is_present("verbose");

        Config {
            rpc_client: RpcClient::new(json_rpc_url),
            verbose,
            owner,
            fee_payer,
            commitment_config: CommitmentConfig::confirmed(),
        }
    };

    solana_logger::setup_with_default("solana=info");

    let _ = match matches.subcommand() {
        ("create-market", Some(arg_matches)) => {
            let stake_token: Pubkey = pubkey_of(arg_matches, "stake_token").unwrap();
            let transit_incoming = value_t_or_exit!(arg_matches, "lock_in", UnixTimestamp);
            let transit_outgoing = value_t_or_exit!(arg_matches, "lock_out", UnixTimestamp);

            let stake_token_account = config.rpc_client.get_account(&stake_token).unwrap();
            let stake_token_account = Mint::unpack(&stake_token_account.data).unwrap();
            let token_precision = <u64>::pow(10, stake_token_account.decimals.into());

            let tier_1 = value_t_or_exit!(arg_matches, "tier_1", f64);
            let tier_1 = ui_to_tokens(tier_1, token_precision);
            let tier_2 = value_t_or_exit!(arg_matches, "tier_2", f64);
            let tier_2 = ui_to_tokens(tier_2, token_precision);
            let tier_3 = value_t_or_exit!(arg_matches, "tier_3", f64);
            let tier_3 = ui_to_tokens(tier_3, token_precision);
            let tier_4 = value_t_or_exit!(arg_matches, "tier_4", f64);
            let tier_4 = ui_to_tokens(tier_4, token_precision);
            let tier_balance = [tier_1, tier_2, tier_3, tier_4];
            command_create_market(
                &config,
                stake_token,
                transit_incoming,
                transit_outgoing,
                tier_balance,
            )
        }
        ("create-pool", Some(arg_matches)) => {
            let market: Pubkey = pubkey_of(arg_matches, "market").unwrap();
            let mint_collection: Pubkey = pubkey_of(arg_matches, "mint_collection").unwrap();
            let mint_distribution: Pubkey = pubkey_of(arg_matches, "mint_distribution").unwrap();
            let pool_owner: Pubkey = pubkey_of(arg_matches, "pool_owner").unwrap();

            let price = ui_to_tokens(value_t_or_exit!(arg_matches, "price", f64), Pool::PRECISION);

            let is_whitelist = value_t_or_exit!(arg_matches, "is_whitelist", bool);
            let kyc_requirement = if value_t_or_exit!(arg_matches, "is_kyc", bool) {
                sol_starter_ido::state::KycRequirement::AnyRequired
            } else {
                sol_starter_ido::state::KycRequirement::NotRequired
            };

            let mint_collection_account = config.rpc_client.get_account(&mint_collection).unwrap();
            let mint_collection_account = Mint::unpack(&mint_collection_account.data).unwrap();
            let token_precision = <u64>::pow(10, mint_collection_account.decimals.into());

            let goal_max = value_t_or_exit!(arg_matches, "goal_max", f64);
            let goal_max = ui_to_tokens(goal_max, token_precision);
            let goal_min = value_t_or_exit!(arg_matches, "goal_min", f64);
            let goal_min = ui_to_tokens(goal_min, token_precision);
            let amount_max = value_t_or_exit!(arg_matches, "amount_max", f64);
            let amount_max = ui_to_tokens(amount_max, token_precision);
            let amount_min = value_t_or_exit!(arg_matches, "amount_min", f64);
            let amount_min = ui_to_tokens(amount_min, token_precision);

            let init_args = InitializePool {
                pool_owner,
                price,
                goal_max,
                goal_min,
                amount_min,
                amount_max,
                time_start: value_t_or_exit!(arg_matches, "time_start", UnixTimestamp),
                time_finish: value_t_or_exit!(arg_matches, "time_finish", UnixTimestamp),
                kyc_requirement,
                time_table: [
                    value_t_or_exit!(arg_matches, "stage_1", u32),
                    value_t_or_exit!(arg_matches, "stage_2", u32),
                ],
            };

            command_create_pool(
                &config,
                &market,
                &mint_collection,
                &mint_distribution,
                init_args,
                is_whitelist,
            )
        }
        ("start-pool", Some(arg_matches)) => {
            let market: Pubkey = pubkey_of(arg_matches, "market").unwrap();
            let pool_to_start: Pubkey = pubkey_of(arg_matches, "pool").unwrap();

            command_start_pool(&config, &market, &pool_to_start)
        }
        ("add-to-whitelist", Some(arg_matches)) => {
            let pool: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            let whitelist_accs_file = value_t_or_exit!(arg_matches, "whitelist-accounts", String);

            command_add_to_whitelist(&config, &pool, &whitelist_accs_file)
        }
        ("participate", Some(arg_matches)) => {
            let pool_key: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            let user_acc_from: Pubkey = pubkey_of(arg_matches, "user-acc-from").unwrap();
            let user_acc_to: Pubkey = pubkey_of(arg_matches, "user-acc-to").unwrap();

            let pool = config.rpc_client.get_account(&pool_key).unwrap();
            let pool = Pool::try_from_slice(&pool.data).unwrap();
            let pool_token_mint = config.rpc_client.get_account(&pool.mint_pool).unwrap();
            let pool_token_mint = Mint::unpack(&pool_token_mint.data).unwrap();
            let token_precision = <u64>::pow(10, pool_token_mint.decimals.into());

            let amount = value_t_or_exit!(arg_matches, "amount", f64);
            let amount = ui_to_tokens(amount, token_precision);

            let stage = value_t_or_exit!(arg_matches, "stage", u8);

            let pool_lock_token: Option<Pubkey> = pubkey_of(arg_matches, "pool-lock-token");
            let market_user_kyc: Option<Pubkey> = pubkey_of(arg_matches, "market-user-kyc");
            let account_whitelist: Option<Pubkey> = pubkey_of(arg_matches, "account-whitelist");

            command_participate(
                &config,
                &pool_key,
                &user_acc_from,
                &user_acc_to,
                amount,
                stage,
                pool_lock_token,
                market_user_kyc,
                account_whitelist,
            )
        }
        ("withdraw", Some(arg_matches)) => {
            let pool: Pubkey = pubkey_of(arg_matches, "pool").unwrap();
            let account_from: Pubkey = pubkey_of(arg_matches, "account-from").unwrap();
            let account_to: Option<Pubkey> = pubkey_of(arg_matches, "account-to");

            command_withdraw(&config, &pool, &account_from, account_to)
        }
        ("pool-info", Some(arg_matches)) => {
            let pool: Pubkey = pubkey_of(arg_matches, "pool").unwrap();

            command_pool_info(&config, &pool)
        }
        _ => unreachable!(),
    }
    .and_then(|transaction| {
        if let Some(transaction) = transaction {
            let signature = config
                .rpc_client
                .send_and_confirm_transaction_with_spinner_and_commitment(
                    &transaction,
                    config.commitment_config,
                )?;
            println!("Signature: {}", signature);
        }
        Ok(())
    })
    .map_err(|err| {
        eprintln!("{:?}", err);
        exit(1);
    });
}
