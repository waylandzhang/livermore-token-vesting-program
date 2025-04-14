use solana_program::{
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
    program_pack::IsInitialized,
};
use arrayref::{array_mut_ref, array_ref};
use crate::instruction::VestingType;
use crate::error::VestingError;
use crate::supply_limits::{PROJECT_YEARS, YEARLY_TOTAL_LIMIT};

/// Tracks the annual issuance amounts for each vesting type
#[derive(Clone, Debug, PartialEq)]
pub struct AnnualSupplyState {
    pub year: u32,
    pub market_issued: u64,
    pub data_purchase_issued: u64,
    pub team_issued: u64,
    pub is_initialized: bool,
}

impl IsInitialized for AnnualSupplyState {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

pub fn validate_project_year(year: u32) -> Result<(), ProgramError> {
    if year == 0 || year > PROJECT_YEARS {
        return Err(VestingError::InvalidYear.into());
    }
    Ok(())
}

impl Sealed for AnnualSupplyState {}
impl Pack for AnnualSupplyState {
    const LEN: usize = 29; // 4 + 8 + 8 + 8 + 1

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, AnnualSupplyState::LEN];
        let (year_dst, rest) = dst.split_at_mut(4);
        let (market_dst, rest) = rest.split_at_mut(8);
        let (data_purchase_dst, rest) = rest.split_at_mut(8);
        let (team_dst, is_init_dst) = rest.split_at_mut(8);

        year_dst.copy_from_slice(&self.year.to_le_bytes());
        market_dst.copy_from_slice(&self.market_issued.to_le_bytes());
        data_purchase_dst.copy_from_slice(&self.data_purchase_issued.to_le_bytes());
        team_dst.copy_from_slice(&self.team_issued.to_le_bytes());
        is_init_dst[0] = self.is_initialized as u8;
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, AnnualSupplyState::LEN];
        let (year_src, rest) = src.split_at(4);
        let (market_src, rest) = rest.split_at(8);
        let (data_purchase_src, rest) = rest.split_at(8);
        let (team_src, is_init_src) = rest.split_at(8);

        Ok(AnnualSupplyState {
            year: u32::from_le_bytes(year_src.try_into().unwrap()),
            market_issued: u64::from_le_bytes(market_src.try_into().unwrap()),
            data_purchase_issued: u64::from_le_bytes(data_purchase_src.try_into().unwrap()),
            team_issued: u64::from_le_bytes(team_src.try_into().unwrap()),
            is_initialized: is_init_src[0] != 0,
        })
    }
}

impl AnnualSupplyState {
    /// Update the issued amount for a specific vesting type
    pub fn update_issued_amount(&mut self, vesting_type: &VestingType, amount: u64) -> Result<(), ProgramError> {
        match vesting_type {
            VestingType::Market => {
                self.market_issued = self.market_issued.checked_add(amount)
                    .ok_or(VestingError::InvalidAmount)?;
            },
            VestingType::DataPurchase => {
                self.data_purchase_issued = self.data_purchase_issued.checked_add(amount)
                    .ok_or(VestingError::InvalidAmount)?;
            },
            VestingType::Team => {
                self.team_issued = self.team_issued.checked_add(amount)
                    .ok_or(VestingError::InvalidAmount)?;
            },
        }
        Ok(())
    }

    /// Validate if a new issuance is within limits
    pub fn validate_issuance(&self, vesting_type: &VestingType, amount: u64) -> Result<(), ProgramError> {
        // Check the total annual limit
        let total_issued = self.get_total_issued()?;

        let new_total = total_issued.checked_add(amount)
            .ok_or(VestingError::InvalidAmount)?;

        if new_total > YEARLY_TOTAL_LIMIT {
            return Err(VestingError::ExceedsYearlyLimit.into());
        }

        Ok(())
    }

    /// Get the total issued amount across all types
    pub fn get_total_issued(&self) -> Result<u64, ProgramError> {
        self.market_issued
            .checked_add(self.data_purchase_issued)
            .ok_or(ProgramError::from(VestingError::InvalidAmount))?
            .checked_add(self.team_issued)
            .ok_or(VestingError::InvalidAmount.into())
    }
}