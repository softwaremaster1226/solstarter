#![allow(unused_imports)]

use crate::{
    id,
    instruction::{
        self, InitializePoolInput, LockInput, StakeStartInput, UnlockInput, UnstakeStartInput,
    },
    prelude::*,
    state::{PoolTransit, StakePool},
};

use solana_program_test::*;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
    transport::TransportError,
};
use spl_token::state::{Account as TokenAccount, Mint};

/// transaction
#[allow(clippy::too_many_arguments)]
pub fn stake_finish(
    pool: &Keypair,
    pool_token_sos: &Keypair,
    pool_transit_to: &Keypair,
    pool_transit_to_token: &Keypair,
    user_token_xsos: &Keypair,
    user_wallet: &Keypair,
    mint_xsos: &Keypair,
    program_context: &ProgramTestContext,
) -> Transaction {
    let instruction = instruction::stake_finish(
        &pool.pubkey(),
        &pool_token_sos.pubkey(),
        &pool_transit_to.pubkey(),
        &pool_transit_to_token.pubkey(),
        &user_token_xsos.pubkey(),
        &user_wallet.pubkey(),
        &mint_xsos.pubkey(),
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
