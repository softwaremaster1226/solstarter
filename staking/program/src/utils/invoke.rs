//! Program state processor
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
};
use spl_token::instruction::initialize_account;

/// Create account
pub fn create_account<'a>(
    funder: AccountInfo<'a>,
    account_to_create: AccountInfo<'a>,
    required_lamports: u64,
    space: u64,
    owner: &Pubkey,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    invoke_signed(
        &system_instruction::create_account(
            &funder.key,
            &account_to_create.key,
            required_lamports,
            space,
            owner,
        ),
        &[funder.clone(), account_to_create.clone()],
        &[&signer_seeds],
    )
}

/// Initialize token account
pub fn initialize_token_account<'a>(
    account_to_initialize: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    owner: AccountInfo<'a>,
    rent: AccountInfo<'a>,
) -> ProgramResult {
    invoke(
        &initialize_account(
            &spl_token::id(),
            &account_to_initialize.key,
            mint.key,
            owner.key,
        )?,
        &[account_to_initialize, mint, owner, rent],
    )
}

/// Initialize mint
pub fn initialize_mint<'a>(
    mint_to_initialize: AccountInfo<'a>,
    mint_authority: AccountInfo<'a>,
    decimals: u8,
    rent: AccountInfo<'a>,
) -> ProgramResult {
    invoke(
        &spl_token::instruction::initialize_mint(
            &spl_token::id(),
            &mint_to_initialize.key,
            mint_authority.key,
            None,
            decimals,
        )?,
        &[mint_to_initialize, mint_authority, rent],
    )
}

/// transfer with authority
pub fn token_transfer_program_authority<'a>(
    owner: &Pubkey,
    source: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    owner_authority: AccountInfo<'a>,
    bump_seed: u8,
    amount: u64,
) -> Result<(), ProgramError> {
    let signature = &[
        &owner.to_bytes()[..32],
        &[bump_seed],
    ];
    token_transfer_signature(source, destination, owner_authority, signature, amount)
}

/// transfer with authority signature
pub fn token_transfer_signature<'a>(
    source: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    owner_authority: AccountInfo<'a>,
    signature: &[&[u8]],
    amount: u64,
) -> Result<(), ProgramError> {
    let tx = spl_token::instruction::transfer(
        &spl_token::id(),
        source.key,
        destination.key,
        owner_authority.key,
        &[&owner_authority.key],
        amount,
    )?;
    invoke_signed(
        &tx,
        &[source, destination, owner_authority],
        &[signature],
    )
}

/// Transfer tokens with user transfer authority
pub fn token_transfer_with_user_authority<'a>(
    source: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    amount: u64,
) -> Result<(), ProgramError> {
    let tx = spl_token::instruction::transfer(
        &spl_token::id(),
        source.key,
        destination.key,
        authority.key,
        &[&authority.key],
        amount,
    )?;
    invoke(&tx, &[source, destination, authority])
}

/// Issue a spl_token `MintTo` instruction
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
        &spl_token::id(),
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
        &spl_token::id(),
        burn_account.key,
        mint.key,
        authority.key,
        &[],
        amount,
    )?;

    invoke(&tx, &[burn_account, mint, authority])
}

/// in program invoke to create program signed seeded account
#[allow(clippy::too_many_arguments)]
pub fn create_derived_account<'a>(
    payer: AccountInfo<'a>,
    account_to_create: AccountInfo<'a>,
    base: AccountInfo<'a>,
    seed: &str,
    required_lamports: u64,
    space: u64,
    owner: &Pubkey,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    solana_program::program::invoke_signed(
        &system_instruction::create_account_with_seed(
            &payer.key,
            &account_to_create.key,
            &base.key,
            seed,
            required_lamports,
            space,
            owner,
        ),
        &[payer.clone(), account_to_create.clone(), base.clone()],
        &[&signer_seeds],
    )
}
