//! In program helpers

use sol_starter_staking::program::ProgramPubkey;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey, system_instruction,
};

/// some reusable methods around accounts
pub trait AccountPatterns {
    /// public key
    fn pubkey(&self) -> Pubkey;

    /// checks if program_id owner of self
    fn is_owner(&self, program_id: &ProgramPubkey) -> ProgramResult;

    /// checks if account is signer
    fn is_signer(&self) -> ProgramResult;
}

impl<'a> AccountPatterns for AccountInfo<'a> {
    fn pubkey(&self) -> Pubkey {
        *self.key
    }

    fn is_owner(&self, program_id: &ProgramPubkey) -> ProgramResult {
        if *self.owner != program_id.pubkey() {
            return Err(ProgramError::IncorrectProgramId);
        }

        Ok(())
    }

    fn is_signer(&self) -> ProgramResult {
        if self.is_signer {
            Ok(())
        } else {
            Err(ProgramError::MissingRequiredSignature)
        }
    }
}

/// Create account with seed signed
#[allow(clippy::too_many_arguments)]
pub fn create_account_with_seed_signed<'a>(
    from_account: &AccountInfo<'a>,
    to_account: &AccountInfo<'a>,
    base: &AccountInfo<'a>,
    seed: &str,
    lamports: u64,
    space: u64,
    program_owner: &ProgramPubkey,
    account_owner: &AccountInfo<'a>,
    bump_seed: u8,
) -> Result<(), ProgramError> {
    let instruction = &system_instruction::create_account_with_seed(
        from_account.key,
        to_account.key,
        base.key,
        seed,
        lamports,
        space,
        &program_owner.pubkey(),
    );
    let signature = &[&account_owner.key.to_bytes()[..32], &[bump_seed]];
    solana_program::program::invoke_signed(
        instruction,
        &[from_account.clone(), to_account.clone(), base.clone()],
        &[&signature[..]],
    )?;
    Ok(())
}

/// burns account
pub fn burn_account(burned: &AccountInfo, beneficiary: &AccountInfo) {
    let mut from = burned.try_borrow_mut_lamports().unwrap();
    let mut to = beneficiary.try_borrow_mut_lamports().unwrap();
    **to += **from;
    **from = 0;
}
