/// The Livermore Project Token Launch on Solana, visit white paper for more details:
/// https://www.investagentpro.com/solana_token_launch
use solana_program::{
    program_error::ProgramError,
};
use crate::{
    instruction::VestingType,
    error::VestingError,
};

// Define token decimals and multiplier
pub const TOKEN_DECIMALS: u8 = 6;
pub const TOKEN_MULTIPLIER: u64 = 1_000_000; // 10^6

pub const PROJECT_START_TIMESTAMP: u64 = 1741608000; // Monday, March 10, 2025 12:00:00 PM GMT

// Define annual limits for different vesting types
pub const MARKET_YEARLY_LIMIT: u64 = 12_600_000 * TOKEN_MULTIPLIER; // 12.6 million tokens/year
pub const DATA_PURCHASE_YEARLY_LIMIT: u64 = 21_000_000 * TOKEN_MULTIPLIER; // 21 million tokens/year
pub const TEAM_YEARLY_LIMIT: u64 = 8_400_000 * TOKEN_MULTIPLIER; // 8.4 million tokens/year

// Define total annual limit (42 million tokens/year)
pub const YEARLY_TOTAL_LIMIT: u64 = MARKET_YEARLY_LIMIT + DATA_PURCHASE_YEARLY_LIMIT + TEAM_YEARLY_LIMIT; // 42 million tokens/year in total

// Total years
pub const PROJECT_YEARS: u32 = 5;

// Define theoretical maximum supply
pub const THEORETICAL_MAX_SUPPLY: u64 = YEARLY_TOTAL_LIMIT * PROJECT_YEARS as u64;

/// Verify that the requested amount is within the yearly limit
pub fn validate_yearly_limit(
    vesting_type: &VestingType,
    year: u32,
    amount: u64,
) -> Result<(), ProgramError> {
    // Check if the year is valid
    if year == 0 || year > PROJECT_YEARS {
        return Err(VestingError::InvalidYear.into());
    }

    // Get the annual limit based on the vesting type
    let yearly_limit = match vesting_type {
        VestingType::Market => MARKET_YEARLY_LIMIT,
        VestingType::DataPurchase => DATA_PURCHASE_YEARLY_LIMIT,
        VestingType::Team => TEAM_YEARLY_LIMIT,
    };

    // Check if the amount exceeds the annual limit
    if amount > yearly_limit {
        return Err(VestingError::ExceedsYearlyLimit.into());
    }

    Ok(())
}
