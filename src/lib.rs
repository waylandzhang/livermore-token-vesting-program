/// The Livermore Project Token Launch on Solana, visit white paper for more details:
/// https://www.investagentpro.com/solana_token_launch

pub mod state;
pub mod processor;
pub mod instruction;
pub mod transfer_hook;
pub mod error;
pub mod supply_limits;
pub mod annual_supply_state;
pub mod utils;

use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    pubkey::Pubkey
};
use crate::processor::Processor;

pub use crate::error::VestingError;
pub use crate::supply_limits::THEORETICAL_MAX_SUPPLY;

entrypoint!(process_instruction);
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    Processor::process_instruction(program_id, accounts, instruction_data)
}

#[cfg(test)]
mod tests {
    use crate::annual_supply_state::AnnualSupplyState;
    use crate::instruction::{VestingType, create_vesting_schedules};
    use crate::supply_limits::*;


    #[test]
    fn test_vesting_schedules_creation() {
        let now: u64 = 1609459200; // 2021-01-01

        // Test Market type
        let market_schedules = create_vesting_schedules(now, VestingType::Market, 1, 1000);
        assert_eq!(market_schedules.len(), 1);
        assert_eq!(market_schedules[0].amount, 1000);

        // Test DataPurchase type
        let data_schedules = create_vesting_schedules(now, VestingType::DataPurchase, 1, 1000);
        assert_eq!(data_schedules.len(), 2);
        assert_eq!(data_schedules[0].amount + data_schedules[1].amount, 1000);

        // Test Team type
        let team_schedules = create_vesting_schedules(now, VestingType::Team, 1, 1000);
        assert_eq!(team_schedules.len(), 1);
        assert_eq!(team_schedules[0].amount, 1000);

        // Test invalid input
        let invalid_schedules = create_vesting_schedules(now, VestingType::Market, 0, 1000);
        assert_eq!(invalid_schedules.len(), 0);
    }

    #[test]
    fn test_annual_supply_validation() {
        // Create a test AnnualSupplyState
        let mut supply_state = AnnualSupplyState {
            year: 1, // Use a valid project year (1-5)
            market_issued: 0,
            data_purchase_issued: 0,
            team_issued: 0,
            is_initialized: true,
        };

        // Test valid issuance
        assert!(supply_state.validate_issuance(&VestingType::Market, 1000).is_ok());

        // Test exceeding type-specific limit
        // This test is failing because we removed the call to validate_yearly_limit in AnnualSupplyState::validate_issuance
        // We should test the total annual limit instead
        
        // Allocate a small portion of the quota to each type
        supply_state.market_issued = MARKET_YEARLY_LIMIT / 2;        // Use 50%
        supply_state.data_purchase_issued = DATA_PURCHASE_YEARLY_LIMIT / 2;  // Use 50%
        supply_state.team_issued = TEAM_YEARLY_LIMIT / 2;            // Use 50%

        // This should be very close to the total annual limit now (since we've allocated 50% of each)
        
        // Try to issue more than the remaining total limit, should fail
        let total_issued = supply_state.get_total_issued().unwrap();
        let remaining = YEARLY_TOTAL_LIMIT.saturating_sub(total_issued);
        
        let result = supply_state.validate_issuance(
            &VestingType::Market,
            remaining + 1 // One more than what's available
        );
        assert!(result.is_err());

        // Try to issue exactly the remaining amount, should succeed
        let result = supply_state.validate_issuance(
            &VestingType::Market,
            remaining
        );
        assert!(result.is_ok());

        // Update issued amount
        supply_state.market_issued += remaining;

        // Try to issue one more token, should fail
        let result = supply_state.validate_issuance(
            &VestingType::Market,
            1
        );
        assert!(result.is_err());
    }
}