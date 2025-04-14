use solana_program::{instruction::{AccountMeta, Instruction}, msg, program_error::ProgramError, pubkey::Pubkey, sysvar};
use solana_program::sysvar::Sysvar;
use spl_associated_token_account::{get_associated_token_address, get_associated_token_address_with_program_id};
use crate::error::VestingError;
use crate::supply_limits::PROJECT_YEARS;
use crate::utils::{calculate_project_year};

pub const MONTH_IN_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const YEAR_IN_SECONDS: u64 = 12 * MONTH_IN_SECONDS;
pub const THREE_YEARS_IN_SECONDS: u64 = 3 * YEAR_IN_SECONDS;

#[derive(Clone, Debug, PartialEq)]
pub struct Schedule {
    pub release_time: u64,
    pub amount: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VestingType {
    Market,
    DataPurchase,
    Team,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum VestingInstruction {
    Create {
        mint_address: Pubkey,
        destination_owner: Pubkey,
        vesting_type: VestingType,
        year: u32,
        start_time: u64,
        amount: u64,
        client_timestamp: u64, // Added client timestamp parameter
    },
    Unlock {
        vesting_account: Pubkey,
        destination_token_address: Pubkey,
    },
    TransferHook,
}

impl VestingInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(ProgramError::InvalidInstructionData)?;
        Ok(match tag {
            0 => {
                if rest.len() < 93 { // Updated to include 8 more bytes for timestamp
                    return Err(ProgramError::InvalidInstructionData);
                }
                let mint_address = Pubkey::new_from_array(rest[..32].try_into().unwrap());
                let destination_owner = Pubkey::new_from_array(rest[32..64].try_into().unwrap());
                let vesting_type = match rest[64] {
                    0 => VestingType::Market,
                    1 => VestingType::DataPurchase,
                    2 => VestingType::Team,
                    _ => return Err(ProgramError::InvalidInstructionData),
                };
                let year = u32::from_le_bytes(rest[65..69].try_into().unwrap());
                if year == 0 {
                    return Err(VestingError::InvalidYear.into());
                }
                let start_time = u64::from_le_bytes(rest[69..77].try_into().unwrap());
                let amount = u64::from_le_bytes(rest[77..85].try_into().unwrap());
                if amount == 0 {
                    return Err(VestingError::InvalidAmount.into());
                }
                let client_timestamp = u64::from_le_bytes(rest[85..93].try_into().unwrap());

                Self::Create {
                    mint_address,
                    destination_owner,
                    vesting_type,
                    year,
                    start_time,
                    amount,
                    client_timestamp,
                }
            }
            1 => {
                if rest.len() < 64 {
                    return Err(ProgramError::InvalidInstructionData);
                }
                let vesting_account = Pubkey::new_from_array(rest[..32].try_into().unwrap());
                let destination_token_address = Pubkey::new_from_array(rest[32..64].try_into().unwrap());
                Self::Unlock {
                    vesting_account,
                    destination_token_address,
                }
            }
            2 => Self::TransferHook,
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            Self::Create {
                mint_address,
                destination_owner,
                vesting_type,
                year,
                start_time,
                amount,
                client_timestamp,
            } => {
                buf.push(0);
                buf.extend_from_slice(&mint_address.to_bytes());
                buf.extend_from_slice(&destination_owner.to_bytes());
                buf.push(match vesting_type {
                    VestingType::Market => 0,
                    VestingType::DataPurchase => 1,
                    VestingType::Team => 2,
                });
                buf.extend_from_slice(&year.to_le_bytes());
                buf.extend_from_slice(&start_time.to_le_bytes());
                buf.extend_from_slice(&amount.to_le_bytes());
                buf.extend_from_slice(&client_timestamp.to_le_bytes()); // Add client timestamp
            }
            Self::Unlock {
                vesting_account,
                destination_token_address,
            } => {
                buf.push(1);
                buf.extend_from_slice(&vesting_account.to_bytes());
                buf.extend_from_slice(&destination_token_address.to_bytes());
            }
            Self::TransferHook => {
                buf.push(2);
            }
        }
        buf
    }
}

pub fn create(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    source_token_account_owner: &Pubkey,
    source_token_account: &Pubkey,
    destination_owner: &Pubkey,
    mint_address: &Pubkey,
    vesting_type: VestingType,
    year: u32,
    start_time: u64,
    amount: u64,
    client_timestamp: Option<u64>, // Make it optional for backward compatibility
) -> Result<Instruction, ProgramError> {
    if year == 0 {
        return Err(VestingError::InvalidYear.into());
    }

    if amount == 0 {
        return Err(VestingError::InvalidAmount.into());
    }

    // Validate that the year parameter is a valid project year
    if year == 0 || year > PROJECT_YEARS {
        return Err(VestingError::InvalidYear.into());
    }

    // Validate that token_program_id is the SPL Token 2022 ID
    if token_program_id != &spl_token_2022::id() {
        return Err(VestingError::InvalidInstruction.into());
    }

    // Validate that the source account is a valid ATA
    let expected_source_token_account = get_associated_token_address_with_program_id(
        source_token_account_owner,
        mint_address,
        token_program_id
    );
    if *source_token_account != expected_source_token_account {
        return Err(VestingError::InvalidInstruction.into());
    }

    // Get current timestamp or use client-provided timestamp
    let timestamp = match client_timestamp {
        Some(ts) => ts,
        None => solana_program::clock::Clock::get()?.unix_timestamp as u64,
    };

    // Calculate current project year
    let current_project_year = calculate_project_year(timestamp)?;

    // Validate that the provided year matches the current project year
    if year != current_project_year {
        return Err(VestingError::InvalidYear.into());
    }

    let (vesting_account_key, _bump) = Pubkey::find_program_address(
        &[b"vesting", destination_owner.as_ref(), mint_address.as_ref(), &timestamp.to_le_bytes()],
        program_id,
    );

    let vesting_token_account_key = get_associated_token_address_with_program_id(
        &vesting_account_key,
        mint_address,
        token_program_id
    );

    // Create the instruction
    let data = VestingInstruction::Create {
        mint_address: *mint_address,
        destination_owner: *destination_owner,
        vesting_type,
        year,
        start_time,
        amount,
        client_timestamp: timestamp, // Use the determined timestamp
    }.pack();

    // Calculate annual supply state account PDA
    let (annual_supply_account_key, _) = Pubkey::find_program_address(
        &[b"annual_supply", &current_project_year.to_le_bytes()],
        program_id,
    );

    let accounts = vec![
        AccountMeta::new_readonly(*token_program_id, false),         // 0: Token Program
        AccountMeta::new(vesting_account_key, false),                // 1: Vesting Account (PDA)
        AccountMeta::new(vesting_token_account_key, false),          // 2: Vesting Token Account (ATA)
        AccountMeta::new(*source_token_account_owner, true),         // 3: Source Token Account Owner (signer)
        AccountMeta::new(*source_token_account, false),              // 4: Source Token Account
        AccountMeta::new_readonly(*destination_owner, false),        // 5: Destination Owner
        AccountMeta::new_readonly(*mint_address, false),             // 6: Mint Address
        AccountMeta::new_readonly(sysvar::clock::id(), false),       // 7: Clock Sysvar
        AccountMeta::new_readonly(solana_program::system_program::id(), false), // 8: System Program
        AccountMeta::new_readonly(spl_associated_token_account::id(), false),      // 9: Associated Token Program
        AccountMeta::new(annual_supply_account_key, false),          // 10: Annual Supply Account (PDA)
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

pub fn unlock(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    clock_sysvar_id: &Pubkey,
    vesting_account_key: &Pubkey,
    vesting_token_account_key: &Pubkey,
    destination_token_account_key: &Pubkey,
    signer: &Pubkey,
    mint_address: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = VestingInstruction::Unlock {
        vesting_account: *vesting_account_key,
        destination_token_address: *destination_token_account_key,
    }
        .pack();

    let accounts = vec![
        AccountMeta::new_readonly(*token_program_id, false),           // SPL Token 2022
        AccountMeta::new_readonly(*clock_sysvar_id, false),            // Clock sysvar
        AccountMeta::new(*vesting_account_key, false),                 // Vesting Account
        AccountMeta::new(*vesting_token_account_key, false),           // Vesting Token Account
        AccountMeta::new(*destination_token_account_key, false),       // Destination Token Account
        AccountMeta::new_readonly(*signer, true),                      // Signer
        AccountMeta::new_readonly(*mint_address, false),               // Mint Address
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

pub fn transfer_hook(
    program_id: &Pubkey,
    source_account: &Pubkey,
    mint_account: &Pubkey,
    destination_account: &Pubkey,
    authority: &Pubkey,
    vesting_account: &Pubkey,
) -> Instruction {
    let data = VestingInstruction::TransferHook.pack();

    let accounts = vec![
        AccountMeta::new_readonly(*source_account, false),             // Source Token Account
        AccountMeta::new_readonly(*mint_account, false),               // Mint Address
        AccountMeta::new_readonly(*destination_account, false),        // Destination Token Account
        AccountMeta::new_readonly(*authority, false),                  // Authority
        AccountMeta::new_readonly(*vesting_account, false),            // Vesting Account
    ];

    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

pub fn create_vesting_schedules(start_time: u64, vesting_type: VestingType, year: u32, amount: u64) -> Vec<Schedule> {
    if year == 0 || amount == 0 {
        return Vec::new();
    }

    let mut schedules = Vec::new();
    match vesting_type {
        VestingType::Market => {
            // Market: Lock 1 month
            schedules.push(Schedule {
                release_time: start_time + MONTH_IN_SECONDS,
                amount,
            });
        },
        VestingType::DataPurchase => {
            // Data Purchase: Lock 50% 1 year, 50% 6 months
            let half_amount = amount / 2;
            let remaining = amount - half_amount; // handle odd amount

            // unlock 50% after 6 months
            schedules.push(Schedule {
                release_time: start_time + 6 * MONTH_IN_SECONDS,
                amount: half_amount,
            });

            // unlock remaining after 1 year
            schedules.push(Schedule {
                release_time: start_time + YEAR_IN_SECONDS,
                amount: remaining,
            });
        },
        VestingType::Team => {
            // Team: Lock 3 years
            schedules.push(Schedule {
                release_time: start_time + 3 * YEAR_IN_SECONDS,
                amount,
            });
        },
    }

    schedules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instruction_packing() {
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let instruction = VestingInstruction::Create {
            mint_address: mint,
            destination_owner: owner,
            vesting_type: VestingType::Market,
            year: 1,
            start_time: 1709856000,
            amount: 1000,
            client_timestamp: 1709856000, // Add timestamp for test
        };
        let packed = instruction.pack();
        let unpacked = VestingInstruction::unpack(&packed).unwrap();
        assert_eq!(instruction, unpacked);
    }
}