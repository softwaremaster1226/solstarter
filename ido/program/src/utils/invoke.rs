//! Invoke methods
use sol_starter_staking::program::ProgramPubkey;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
};
use spl_token::instruction::{initialize_account, initialize_mint as initialize_token_mint};

use crate::spl_token_id;

/// Create account
pub fn create_account<'a>(
    funder: AccountInfo<'a>,
    account_to_create: AccountInfo<'a>,
    required_lamports: u64,
    space: u64,
    owner: &ProgramPubkey,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    invoke_signed(
        &system_instruction::create_account(
            &funder.key,
            &account_to_create.key,
            required_lamports,
            space,
            &owner.pubkey(),
        ),
        &[funder.clone(), account_to_create.clone()],
        &[&signer_seeds],
    )
}

/// transfer lamports
pub fn transfer_program<'a>(
    from: AccountInfo<'a>,
    to: AccountInfo<'a>,
    lamports: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    invoke_signed(
        &system_instruction::transfer(&from.key, &to.key, lamports),
        &[from.clone(), to.clone()],
        &[&signer_seeds],
    )
}

/// Initialize token account
pub fn initialize_token_account<'a>(
    account_to_initialize: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    owner: AccountInfo<'a>,
    rent_account: AccountInfo<'a>,
) -> ProgramResult {
    invoke(
        &initialize_account(
            &spl_token_id().pubkey(),
            &account_to_initialize.key,
            mint.key,
            owner.key,
        )?,
        &[account_to_initialize, mint, owner, rent_account],
    )
}

/// Initialize mint
pub fn initialize_mint<'a>(
    mint_to_initialize: AccountInfo<'a>,
    mint_authority: AccountInfo<'a>,
    decimals: u8,
    rent_account: AccountInfo<'a>,
) -> ProgramResult {
    invoke(
        &initialize_token_mint(
            &spl_token_id().pubkey(),
            &mint_to_initialize.key,
            mint_authority.key,
            None,
            decimals,
        )?,
        &[mint_to_initialize, mint_authority, rent_account],
    )
}

/// Transfer tokens with program address
#[allow(clippy::too_many_arguments)]
pub fn token_transfer<'a>(
    pool: &Pubkey,
    source: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    bump_seed: u8,
    amount: u64,
) -> Result<(), ProgramError> {
    let authority_signature_seeds = [&pool.to_bytes()[..32], &[bump_seed]];
    let signers = &[&authority_signature_seeds[..]];

    let tx = spl_token::instruction::transfer(
        &spl_token_id().pubkey(),
        source.key,
        destination.key,
        authority.key,
        &[&authority.key],
        amount,
    )?;
    invoke_signed(&tx, &[source, destination, authority], signers)
}

/// Transfer tokens with user transfer authority
pub fn token_transfer_with_user_authority<'a>(
    source: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    amount: u64,
) -> Result<(), ProgramError> {
    let tx = spl_token::instruction::transfer(
        &spl_token_id().pubkey(),
        source.key,
        destination.key,
        authority.key,
        &[&authority.key],
        amount,
    )?;
    invoke(&tx, &[source, destination, authority])
}

/// Issue a spl_token `MintTo` instruction
#[allow(clippy::too_many_arguments)]
pub fn token_mint_to<'a>(
    pool: &Pubkey,
    mint: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    bump_seed: u8,
    amount: u64,
) -> Result<(), ProgramError> {
    let authority_signature_seeds = [&pool.to_bytes()[..32], &[bump_seed]];
    let signers = &[&authority_signature_seeds[..]];
    let ix = spl_token::instruction::mint_to(
        &spl_token_id().pubkey(),
        mint.key,
        destination.key,
        authority.key,
        &[],
        amount,
    )?;

    invoke_signed(&ix, &[mint, destination, authority], signers)
}

/// Burn tokens with user authority
pub fn burn_tokens_with_user_authority<'a>(
    burn_account: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    amount: u64,
) -> Result<(), ProgramError> {
    let tx = spl_token::instruction::burn(
        &spl_token_id().pubkey(),
        burn_account.key,
        mint.key,
        authority.key,
        &[],
        amount,
    )?;

    invoke(&tx, &[burn_account, mint, authority])
}
