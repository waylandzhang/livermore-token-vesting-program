use solana_program::program_error::ProgramError;
use crate::supply_limits::{PROJECT_START_TIMESTAMP, PROJECT_YEARS};
use crate::VestingError;
use crate::instruction::YEAR_IN_SECONDS;

/// Calculate project year (1-5)
pub fn calculate_project_year(current_timestamp: u64) -> Result<u32, ProgramError> {

    if current_timestamp < PROJECT_START_TIMESTAMP {
        return Err(VestingError::InvalidTime.into());
    }

    let elapsed_seconds = current_timestamp - PROJECT_START_TIMESTAMP;
    let years_passed = (elapsed_seconds / YEAR_IN_SECONDS) as u32;

    // Project year starts from 1, maximum is 5
    let project_year = years_passed + 1;
    if project_year > PROJECT_YEARS {
        return Err(VestingError::InvalidYear.into());
    }

    Ok(project_year)
}