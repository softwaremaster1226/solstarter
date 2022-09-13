//! Error types

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use solana_program::{
    decode_error::DecodeError,
    msg,
    program_error::{PrintProgramError, ProgramError},
};

/// Errors that may be returned by the SolStarter program.
#[derive(Clone, Debug, Eq, thiserror::Error, FromPrimitive, PartialEq)]
pub enum Error {
    /// Wrong market owner account
    #[error("Wrong owner account")]
    WrongOwner,

    /// Pool must be related to market
    #[error("Pool must be related to market")]
    PoolMustBeRelatedToMarket,

    /// Lock must be related to pool
    #[error("Lock must be related to pool")]
    LockMustBeRelatedToPool,

    /// Pool must be active for some time
    #[error("Pool must be active for some time")]
    PoolMustBeActiveForSomeTime,

    /// Cannot unlock when pool is active
    #[error("Cannot unlock when pool is active")]
    CannotUnlockWhenPoolIsActive,

    /// Cannot lock when pool is active
    #[error("Cannot lock when pool is active")]
    CannotLockWhenPoolIsActive,

    /// Invalid authority
    #[error("Invalid authority")]
    InvalidAuthority,

    /// One of the accounts does not correspond to the rest of the data
    #[error("Wrong account specified")]
    WrongAccountSpecified,

    /// Overflow
    #[error("Overflow")]
    Overflow,

    /// Underflow
    #[error("Underflow")]
    Underflow,

    /// Cannot transit anything now
    #[error("Cannot transit anything now")]
    CannotTransitAnythingNow,

    /// Derived account key is not equal to calculated
    #[error("Derived account key is not equal to calculated")]
    DerivedAccountKeyIsNotEqualToCalculated,

    /// Derived pool lock account key is not equal to calculated
    #[error("Derived pool lock account key is not equal to calculated")]
    DerivedPoolLockAccountKeyIsNotEqualToCalculated,

    /// Pool transit wrong direction
    #[error("Pool transit wrong direction")]
    PoolTransitWrongDirection,

    /// Pool transit must be of provided pool
    #[error("Pool transit must be of provided pool")]
    PoolTransitMustBeOfProvidedPool,
}

impl From<Error> for ProgramError {
    fn from(e: Error) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for Error {
    fn type_of() -> &'static str {
        "SolStarterStakingError"
    }
}

impl PrintProgramError for Error {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        msg!(&self.to_string())
    }
}
