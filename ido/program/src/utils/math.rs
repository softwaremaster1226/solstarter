//! Some math.

use solana_program::clock::UnixTimestamp;
use solana_program::program_error::ProgramError;

use crate::error::Error;

/// checked add into error
pub trait ErrorAddSub<T> {
    /// errored
    fn error_increment(self) -> Result<T, ProgramError>;
    /// errored
    fn error_add(self, rhs: T) -> Result<T, ProgramError>;
    /// errored
    fn error_sub(self, rhs: T) -> Result<T, ProgramError>;
    /// errored
    fn error_decrement(self) -> Result<T, ProgramError>;
}

impl ErrorAddSub<u64> for u64 {
    fn error_increment(self) -> Result<u64, ProgramError> {
        self.checked_add(1).ok_or_else(|| Error::Overflow.into())
    }

    fn error_decrement(self) -> Result<u64, ProgramError> {
        self.checked_sub(1).ok_or_else(|| Error::Underflow.into())
    }

    fn error_add(self, rhs: u64) -> Result<u64, ProgramError> {
        self.checked_add(rhs).ok_or_else(|| Error::Overflow.into())
    }

    fn error_sub(self, rhs: u64) -> Result<u64, ProgramError> {
        self.checked_sub(rhs).ok_or_else(|| Error::Underflow.into())
    }
}

impl ErrorAddSub<u128> for u128 {
    fn error_increment(self) -> Result<u128, ProgramError> {
        self.checked_add(1).ok_or_else(|| Error::Overflow.into())
    }

    fn error_decrement(self) -> Result<u128, ProgramError> {
        self.checked_sub(1).ok_or_else(|| Error::Underflow.into())
    }

    fn error_add(self, rhs: u128) -> Result<u128, ProgramError> {
        self.checked_add(rhs).ok_or_else(|| Error::Overflow.into())
    }

    fn error_sub(self, rhs: u128) -> Result<u128, ProgramError> {
        self.checked_sub(rhs).ok_or_else(|| Error::Underflow.into())
    }
}

impl ErrorAddSub<UnixTimestamp> for UnixTimestamp {
    fn error_increment(self) -> Result<UnixTimestamp, ProgramError> {
        self.checked_add(1).ok_or_else(|| Error::Overflow.into())
    }

    fn error_decrement(self) -> Result<UnixTimestamp, ProgramError> {
        self.checked_sub(1).ok_or_else(|| Error::Underflow.into())
    }

    fn error_add(self, rhs: UnixTimestamp) -> Result<UnixTimestamp, ProgramError> {
        self.checked_add(rhs).ok_or_else(|| Error::Overflow.into())
    }

    fn error_sub(self, rhs: UnixTimestamp) -> Result<UnixTimestamp, ProgramError> {
        self.checked_sub(rhs).ok_or_else(|| Error::Underflow.into())
    }
}

impl ErrorAddSub<u32> for u32 {
    fn error_increment(self) -> Result<u32, ProgramError> {
        self.checked_add(1).ok_or_else(|| Error::Overflow.into())
    }

    fn error_decrement(self) -> Result<u32, ProgramError> {
        self.checked_sub(1).ok_or_else(|| Error::Underflow.into())
    }

    fn error_add(self, rhs: u32) -> Result<u32, ProgramError> {
        self.checked_add(rhs).ok_or_else(|| Error::Overflow.into())
    }

    fn error_sub(self, rhs: u32) -> Result<u32, ProgramError> {
        self.checked_sub(rhs).ok_or_else(|| Error::Underflow.into())
    }
}

/// checked mul and div into error
pub trait ErrorMulDiv<T> {
    /// errored
    fn error_mul(self, rhs: T) -> Result<T, ProgramError>;

    /// errored
    fn error_div(self, rhs: T) -> Result<T, ProgramError>;
}

impl ErrorMulDiv<u128> for u128 {
    fn error_mul(self, rhs: u128) -> Result<u128, ProgramError> {
        self.checked_mul(rhs).ok_or_else(|| Error::Overflow.into())
    }

    fn error_div(self, rhs: u128) -> Result<u128, ProgramError> {
        self.checked_div(rhs)
            .ok_or_else(|| Error::DivisionByZero.into())
    }
}
