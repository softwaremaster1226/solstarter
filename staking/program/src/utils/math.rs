//! Some math.

use solana_program::clock::UnixTimestamp;
use solana_program::program_error::ProgramError;
use std::convert::TryFrom;

use crate::error::Error;

/// checked add into error
pub trait ErrorAdd<T> {
    /// errored
    fn error_increment(self) -> Result<T, ProgramError>;
    /// errored
    fn error_add(self, rhs: T) -> Result<T, ProgramError>;
    /// errored
    fn error_sub(self, rhs: T) -> Result<T, ProgramError>;
    /// errored
    fn error_decrement(self) -> Result<T, ProgramError>;
}

impl ErrorAdd<u64> for u64 {
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

impl ErrorAdd<UnixTimestamp> for UnixTimestamp {
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

impl ErrorAdd<u32> for u32 {
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

/// calculates transfer amount of tokens proportional to time passed
pub fn finish(
    transit_from: UnixTimestamp,
    now: UnixTimestamp,
    transit_until: UnixTimestamp,
    amount_claimed: u64,
    remaining_amount: u64,
) -> Option<u64> {
    // should use 256 bit?
    let amount_claimed = amount_claimed as u128;
    let remaining_amount = remaining_amount as u128;

    let transit_interval = i64::max(0, transit_until.checked_sub(transit_from)?) as u128;

    let time_passed = i64::max(0, now.checked_sub(transit_from)?) as u128;
    let time_passed = u128::min(transit_interval, time_passed);

    let total = amount_claimed.checked_add(remaining_amount)?;

    let possible_to_claim = total
        .checked_mul(time_passed)?
        .checked_div(transit_interval)?;
    let amount_to_claim = possible_to_claim.checked_sub(amount_claimed)?;
    if amount_to_claim == 0 {
        None
    } else {
        u64::try_from(amount_to_claim).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn calculate() {
        let result = finish(0, 10, 10, 0, 100);
        assert_eq!(result, Some(100));

        let result = finish(0, 0, 10, 0, 100);
        assert_eq!(result, None);

        let result = finish(0, 1, 10, 0, 100);
        assert_eq!(result, Some(10));

        let result = finish(0, 5, 10, 0, 100);
        assert_eq!(result, Some(50));

        let result = finish(0, 100, 99, 0, 100);
        assert_eq!(result, Some(100));

        let result = finish(0, 98, 99, 0, 100);
        assert_eq!(result, Some(98));

        let result = finish(0, 98, 99, 0, 1);
        assert_eq!(result, None);

        let result = finish(0, 99, 99, 0, 1);
        assert_eq!(result, Some(1));

        let result = finish(0, 99, 99, 5, 5);
        assert_eq!(result, Some(5));

        let result = finish(0, 99, 99, 4, 6);
        assert_eq!(result, Some(6));

        let result = finish(10, 20, 30, 0, 10);
        assert_eq!(result, Some(5));

        let result = finish(10, 5, 30, 0, 10);
        assert_eq!(result, None);

        let result = finish(
            1_000_000_000_000,
            1_000_000_000_000 + 1_000_000,
            1_000_000_000_000 + 2_000_000,
            0,
            1_000_000_000,
        );
        assert_eq!(result, Some(500_000_000));

        let result = finish(0, 99, 99, 0, 0);
        assert_eq!(result, None);

        let result = finish(0, 75, 100, 50, 50);
        assert_eq!(result, Some(25));

        let result = finish(0, 33, 99, 10, 0);
        assert_eq!(result, None);

        let result = finish(0, 33, 10_000_000_000, 1, 1);
        assert_eq!(result, None);

        let result = finish(0, 10_000_000_000 - 1, 10_000_000_000, 1, 1);
        assert_eq!(result, None);

        let result = finish(0, 10_000_000_000 - 1, 10_000_000_000, 0, 2);
        assert_eq!(result, Some(1));

        let result = finish(0, 1_000_000_000 / 2, 1_000_000_000, 0, 100_000_000_000);
        assert_eq!(result, Some(100_000_000_000 / 2));

        let result = finish(
            0,
            1_000_000_000_000,
            1_000_000_000_000,
            0,
            1_000_000_000_000,
        );
        assert_eq!(result, Some(1_000_000_000_000));

        let result = finish(
            0,
            1_000_000_000_000,
            1_000_000_000_000,
            1_000_000_000_000,
            0,
        );
        assert_eq!(result, None);

        let result = finish(
            0,
            1_000_000_000_000 / 2,
            1_000_000_000_000,
            1_000_000_000_000 / 4,
            1_000_000_000_000 - 1_000_000_000_000 / 4,
        );
        assert_eq!(result, Some(1_000_000_000_000 / 4));

        let mut remaining_amount = 100u64;
        let mut amount_claimed = 0u64;
        let mut time = 0;
        loop {
            let amount_to_claim = finish(0, time, 100, amount_claimed, remaining_amount);
            if let Some(amount_to_claim) = amount_to_claim {
                remaining_amount -= amount_to_claim;
                amount_claimed += amount_to_claim;
            }

            if amount_claimed == 100 && remaining_amount == 0 && time == 100 {
                break;
            }
            time += 1;
        }
    }
}
