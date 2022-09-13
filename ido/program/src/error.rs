//! Error types

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;
use solana_program::{
    decode_error::DecodeError,
    msg,
    program_error::{PrintProgramError, ProgramError},
};

/// Errors that may be returned by the SolStarter program.
#[derive(Clone, Debug, Eq, thiserror::Error, FromPrimitive, PartialEq, ToPrimitive)]
pub enum Error {
    /// Wrong market owner
    #[error("Wrong market owner")]
    WrongMarketOwner,

    /// Market or pool owner required
    #[error("Market or pool owner required")]
    MarketOrPoolOwnerRequired,

    /// Wrong program address
    #[error("Wrong program address")]
    WrongProgramAddress,

    /// Wrong token mint account
    #[error("Wrong token mint account")]
    WrongTokenMint,

    /// Wrong pool token mint account
    #[error("Wrong pool token mint account")]
    WrongPoolTokenMint,

    /// Wrong market address for current pool
    #[error("Wrong market address for current pool")]
    WrongMarketAddressForCurrentPool,

    /// Pool authority must be aligned to pool
    #[error("Pool authority must be aligned to pool")]
    PoolAuthorityMustBeAlignedToPool,

    /// Invalid goal numbers
    #[error("Invalid goal numbers")]
    InvalidGoalNumbers,

    /// Invalid investment size numbers
    #[error("Invalid investment size numbers")]
    InvalidInvestmentSizeNumbers,

    /// Invalid pool time frame
    #[error("Invalid pool time frame")]
    InvalidPoolTimeFrame,

    /// Market authority must be derived from market
    #[error("Market authority must be derived from market")]
    MarketAuthorityMustBeDerivedFromMarket,

    /// Invalid time table
    #[error("Invalid time table")]
    InvalidTimeTable,

    /// Wrong account to collect tokens
    #[error("Wrong account to collect tokens")]
    WrongCollectAccount,

    /// Wrong kyc account
    #[error("Wrong kyc account")]
    WrongKycAccount,

    /// Unable to deposit at current time
    #[error("Unable to deposit at current time")]
    CantDepositAtCurrentTime,

    /// Incorrect amount to deposit
    #[error("Incorrect amount to deposit")]
    IncorrectDepositAmount,

    /// Pool already full
    #[error("Pool already full")]
    PoolAlreadyFull,

    /// Can't claim tokens till pool is active
    #[error("Can't claim tokens till pool is active")]
    CantClaimFromActivePool,

    /// Wrong pool account to send tokens from
    #[error("Wrong pool account to send tokens from")]
    WrongPoolAccountToSendTokensFrom,

    /// Pool doesn't have mint whitelist account
    #[error("Pool doesn't have mint whitelist account")]
    WhitelistMintNotSet,

    /// Can't withdraw from active pool
    #[error("Can't withdraw from active pool")]
    CantWithdrawFromActivePool,

    /// Overflow
    #[error("Overflow")]
    Overflow,

    /// Division By Zero
    #[error("Division By Zero")]
    DivisionByZero,

    /// Underflow
    #[error("Underflow")]
    Underflow,

    /// Whitelist mint account missing
    #[error("Whitelist mint account missing")]
    WhitelistMintMissing,

    /// Whitelist mint account invalid
    #[error("Whitelist mint account invalid")]
    WhitelistMintInvalid,

    /// Wrong KYC credentials
    #[error("Wrong KYC credentials")]
    WrongKycCredentials,

    /// Wrong user pool stage
    #[error("Wrong user pool stage")]
    WrongUserPoolStage,

    /// Wrong KYC owner
    #[error("Wrong KYC owner")]
    WrongKycOwner,

    /// Lock owner must be user wallet
    #[error("Lock owner must be user wallet")]
    LockOwnerMustBeUserWallet,

    /// Pool lock token must be attached to pool lock
    #[error("Pool lock token must be attached to pool lock")]
    PoolLockTokenMustBeAttachedToPoolLock,

    /// Input time must be in future
    #[error("Input time must be in future")]
    InputTimeMustBeInFuture,

    /// Stake pool must belong to market
    #[error("Stake pool must belong to market")]
    StakePoolMustBelongToMarket,

    /// Account on this tier cannot participate on current stage
    #[error("Account on this tier cannot participate on current stage")]
    AccountOnThisTierCannotParticipateOnCurrentStage,

    /// Account already participated on this stage
    #[error("Account already participated on this stage")]
    AccountAlreadyParticipatedOnThisStage,

    /// Can participate only in started pool
    #[error("Can participate only in started pool")]
    CanParticipateOnlyInStartedPool,
}
impl From<Error> for ProgramError {
    fn from(e: Error) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for Error {
    fn type_of() -> &'static str {
        "SolStarterError"
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
