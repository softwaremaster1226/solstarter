//! In program helpers

use std::mem;

use borsh::BorshSerialize;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
};

use crate::error::Error;

/// some well know often users patters for program derived keys
pub trait PubkeyPatterns {
    /// Find authority address and bump seed based on 1 pubkey
    fn find_key_program_address(
        owner: &Pubkey,
        program_id: &ProgramPubkey,
    ) -> (ProgramDerivedPubkey, u8);

    /// Find authority address and bump seed based on 2 pubkeys
    fn find_2key_program_address(
        key1: &Pubkey,
        key2: &Pubkey,
        program_id: &ProgramPubkey,
    ) -> (Pubkey, u8);

    /// pubkey
    fn pubkey(&self) -> Pubkey;
}

impl PubkeyPatterns for Pubkey {
    fn find_key_program_address(key: &Pubkey, program_id: &ProgramPubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[&key.to_bytes()[..32]], &program_id.pubkey())
    }

    fn find_2key_program_address(
        key1: &Pubkey,
        key2: &Pubkey,
        program_id: &ProgramPubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[&key1.to_bytes()[..32], &key2.to_bytes()[..32]],
            &program_id.pubkey(),
        )
    }

    fn pubkey(&self) -> Pubkey {
        *self
    }
}

/// swaps two accounts data
/// panics if accounts are borrowedy
pub fn swap_accounts<'a, T: Default + BorshSerialize>(
    current: &AccountInfo<'a>,
    last: &AccountInfo<'a>,
) -> Result<(), ProgramError> {
    let mut last_data = last.data.try_borrow_mut().unwrap();
    if current.key != last.key {
        let mut current_data = current.data.try_borrow_mut().unwrap();
        mem::swap(&mut *current_data, &mut *last_data);
    }
    T::default().serialize(&mut *last_data)?;
    Ok(())
}
/// some reusable methods around accounts
pub trait AccountPatterns {
    /// validate key is equal to other key which assumed to  be derived
    fn is_derived<'b, K: Into<&'b ProgramPubkey>>(
        &self,
        owner: &Pubkey,
        program_id: K,
    ) -> Result<u8, ProgramError>;
    /// public key
    fn pubkey(&self) -> Pubkey;

    /// checks if program_id owner of self
    fn is_owner(&self, program_id: &ProgramPubkey) -> ProgramResult;

    /// checks if account is signer
    fn is_signer(&self) -> ProgramResult;
}

impl<'a> AccountPatterns for AccountInfo<'a> {
    fn is_derived<'b, K: Into<&'b ProgramPubkey>>(
        &self,
        owner: &Pubkey,
        program_id: K,
    ) -> Result<u8, ProgramError> {
        let (expected_key, seed) = Pubkey::find_key_program_address(owner, &program_id.into());

        if *self.key == expected_key {
            Ok(seed)
        } else {
            Err(Error::DerivedAccountKeyIsNotEqualToCalculated.into())
        }
    }

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
        if !self.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(())
    }
}

/// marker for keys which are programs
pub struct ProgramPubkey(pub Pubkey);

impl ProgramPubkey {
    /// public key
    pub fn pubkey(&self) -> Pubkey {
        self.0
    }
}

#[allow(clippy::from_over_into)] // by design
impl Into<Pubkey> for ProgramPubkey {
    fn into(self) -> Pubkey {
        self.pubkey()
    }
}

/// marker for addresses which are derived from program (so these such accounts can only be created and initialized by the owner program)
pub type ProgramDerivedPubkey = Pubkey;

/// marker wrapper for program accounts
pub struct ProgramAccountInfo<'a, 'b>(pub &'b AccountInfo<'a>);

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
    signature: &[&[u8]],
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

    solana_program::program::invoke_signed(
        instruction,
        &[from_account.clone(), to_account.clone(), base.clone()],
        &[signature],
    )?;
    Ok(())
}
