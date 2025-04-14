use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar, rent::Rent},
};
use solana_program::instruction::AccountMeta;
use spl_token_2022::{
    instruction::transfer_checked,
    state::Mint,
};
use spl_associated_token_account::{get_associated_token_address, get_associated_token_address_with_program_id};
use solana_program::program_pack::Pack;
use crate::{
    instruction::{VestingInstruction, VestingType, create_vesting_schedules},
    state::{VestingSchedule, VestingScheduleHeader, pack_schedules_into_slice, unpack_schedules},
    transfer_hook::process_transfer_hook,
    error::VestingError,
};
use solana_program::program_pack::IsInitialized;
use crate::supply_limits::{validate_yearly_limit, THEORETICAL_MAX_SUPPLY, TOKEN_DECIMALS};
use crate::annual_supply_state::AnnualSupplyState;
use crate::utils::{calculate_project_year};

impl IsInitialized for VestingScheduleHeader {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

pub struct Processor;

impl Processor {
    pub fn process_create(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        mint_address: &Pubkey,
        destination_owner: &Pubkey,
        vesting_type: VestingType,
        year: u32,
        start_time: u64,
        amount: u64,
        client_timestamp: u64, // Add client timestamp parameter
    ) -> ProgramResult {
        // msg!("===== DEBUG START =====");
        // msg!("Processing create instruction for vesting_type: {:?}, year: {}, amount: {}", vesting_type, year, amount);
        // msg!("Using client-provided timestamp: {}", client_timestamp);

        // Check account count
        if accounts.len() != 11 {
            // msg!("Error: Expected 11 accounts but got {}", accounts.len());
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        // Validate input parameters
        if year == 0 {
            return Err(VestingError::InvalidYear.into());
        }
        if amount == 0 {
            return Err(VestingError::InvalidAmount.into());
        }

        // Validate annual issuance limit
        validate_yearly_limit(&vesting_type, year, amount)?;

        let accounts_iter = &mut accounts.iter();
        let token_program = next_account_info(accounts_iter)?;
        let vesting_account = next_account_info(accounts_iter)?;
        let vesting_token_account = next_account_info(accounts_iter)?;
        let source_token_account_owner = next_account_info(accounts_iter)?;
        let source_token_account = next_account_info(accounts_iter)?;
        let destination_owner_info = next_account_info(accounts_iter)?;
        let mint_account = next_account_info(accounts_iter)?;
        let clock_sysvar = next_account_info(accounts_iter)?;
        let system_program = next_account_info(accounts_iter)?;
        let associated_token_program = next_account_info(accounts_iter)?;
        let annual_supply_account = next_account_info(accounts_iter)?;

        // msg!("Vesting token account owner: {}", vesting_token_account.owner);
        // msg!("Source token account owner: {}", source_token_account.owner);

        // Verify if source_token_account is a valid ATA
        let expected_source_token_account = get_associated_token_address_with_program_id(
            source_token_account_owner.key,
            mint_address,
            token_program.key,  // Explicitly pass the Token Program 2022 ID
        );
        if *source_token_account.key != expected_source_token_account {
            // msg!("Source token account is not a valid ATA for the owner");
            return Err(VestingError::InvalidInstruction.into());
        }

        // Verify destination owner
        if destination_owner_info.key != destination_owner {
            return Err(VestingError::InvalidInstruction.into());
        }
        if *token_program.key != spl_token_2022::id() {
            return Err(VestingError::InvalidInstruction.into());
        }

        // Verify the source token account owner is a signer
        if !source_token_account_owner.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // Get current time
        let clock = Clock::from_account_info(clock_sysvar)?;
        let current_timestamp = clock.unix_timestamp as u64;
        // msg!("Current chain timestamp: {}", current_timestamp);

        // Calculate project year - use current timestamp (doesn't affect PDA calculation)
        let project_year = calculate_project_year(current_timestamp)?;

        // Verify system program and associated token program
        if *system_program.key != solana_program::system_program::id() {
            return Err(VestingError::InvalidInstruction.into());
        }
        if *associated_token_program.key != spl_associated_token_account::id() {
            return Err(VestingError::InvalidInstruction.into());
        }

        // Check vesting account PDA, using client-provided timestamp
        let (vesting_account_key, bump) = Pubkey::find_program_address(
            &[b"vesting", destination_owner.as_ref(), mint_address.as_ref(), &client_timestamp.to_le_bytes()],
            program_id,
        );

        // msg!("Calculated vesting PDA: {}", vesting_account_key);
        // msg!("Provided vesting account: {}", vesting_account.key);

        if *vesting_account.key != vesting_account_key {
            // msg!("Error: Vesting account mismatch. Expected: {}, Got: {}",
            //     vesting_account_key, vesting_account.key);
            return Err(VestingError::InvalidInstruction.into());
        }

        // Check if the vesting account already exists and is initialized
        if !vesting_account.data_is_empty() {
            // If the account already exists, return an error to prevent double funding
            return Err(VestingError::InvalidInstruction.into());
        }

        // First, check if the vesting account PDA needs to be created
        if vesting_account.data_is_empty() {
            // msg!("Creating vesting account PDA - account is empty");

            // Calculate required space
            let schedules = create_vesting_schedules(start_time, vesting_type.clone(), year, amount);
            if schedules.is_empty() {
                // msg!("Error: No schedules created");
                return Err(VestingError::InvalidInstruction.into());
            }
            let required_size = VestingScheduleHeader::LEN + schedules.len() * VestingSchedule::LEN;
            // msg!("Required size for vesting account: {}", required_size);

            // Calculate rent exempt balance
            let rent = Rent::get()?;
            let lamports = rent.minimum_balance(required_size);
            // msg!("Rent-exempt lamports: {}", lamports);

            // TEST IF VESTING ACCOUNT CREATION IS FAILING
            // msg!("About to create vesting account PDA");
            match invoke_signed(
                &solana_program::system_instruction::create_account(
                    source_token_account_owner.key,
                    vesting_account.key,
                    lamports,
                    required_size as u64,
                    program_id,
                ),
                &[
                    source_token_account_owner.clone(),
                    vesting_account.clone(),
                    system_program.clone(),
                ],
                &[&[
                    b"vesting",
                    destination_owner.as_ref(),
                    mint_address.as_ref(),
                    &client_timestamp.to_le_bytes(), // Use client timestamp for PDA derivation
                    &[bump],
                ]],
            ) {
                Ok(_) => msg!("Vesting account created successfully!"),
                Err(e) => {
                    msg!("Error creating vesting account: {:?}", e);
                    return Err(e);
                }
            }

            // Initialize the vesting account data
            // msg!("Initializing vesting account data");
            let mut data = vesting_account.data.borrow_mut();
            let header = VestingScheduleHeader {
                destination_address: get_associated_token_address_with_program_id(
                    destination_owner,
                    mint_address,
                    token_program.key
                ),
                mint_address: *mint_address,
                creator_pubkey: *source_token_account_owner.key,
                destination_owner: *destination_owner,
                is_initialized: true,
                creation_timestamp: client_timestamp, // Store client timestamp in header
            };

            // msg!("Packing header into vesting account data");
            header.pack_into_slice(&mut data[..VestingScheduleHeader::LEN]);

            // Store the schedules
            // msg!("Packing {} schedules into vesting account", schedules.len());
            let vesting_schedules: Vec<VestingSchedule> = schedules.into_iter().map(|s| VestingSchedule {
                release_time: s.release_time,
                amount: s.amount,
            }).collect();

            match pack_schedules_into_slice(vesting_schedules, &mut data[VestingScheduleHeader::LEN..]) {
                Ok(_) => msg!("Schedules packed successfully"),
                Err(e) => {
                    msg!("Error packing schedules: {:?}", e);
                    return Err(e);
                }
            }

            // msg!("Vesting account initialization complete");
        }

        // Check if source account has enough tokens
        // msg!("Checking source token account balance");
        {
            let source_token_data = source_token_account.data.borrow();

            // msg!("Source token account size: {}", source_token_data.len());
            // msg!("Expected Account::LEN: {}", spl_token_2022::state::Account::LEN);

            let source_token = match spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Account>::unpack(&source_token_data) {
                Ok(extension_account) => extension_account.base,
                Err(err) => {
                    // msg!("Failed to unpack source token account with extensions: {:?}", err);
                    return Err(err.into());
                }
            };

            // msg!("Source token account balance: {}", source_token.amount);
            if source_token.amount < amount {
                // msg!("Insufficient token balance: {} < {}", source_token.amount, amount);
                return Err(VestingError::InvalidAmount.into());
            }
        }


        // Now handle the vesting token account
        if vesting_token_account.data_is_empty() {
            // msg!("Vesting token account is empty - needs to be created first by client");
            return Err(VestingError::InvalidInstruction.into());
        }
        // Just log the token account with no validation
        // msg!("Vesting token account exists, proceeding with transfer");
        // msg!("Vesting token account address: {}", vesting_token_account.key);
        // msg!("Expected ATA address: {}", get_associated_token_address_with_program_id(
        //     vesting_account.key,
        //     mint_address,
        //     token_program.key,
        // ));

        // Verify Mint address
        if mint_account.key != mint_address {
            return Err(VestingError::InvalidInstruction.into());
        }

        if start_time < clock.unix_timestamp as u64 {
            return Err(VestingError::InvalidTime.into());
        }

        // msg!("Unpacking mint data");
        let mint_data = mint_account.data.borrow();
        let mint = spl_token_2022::extension::StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
        let decimals = mint.base.decimals;
        
        // Validate that the mint decimals match the expected TOKEN_DECIMALS
        if decimals != TOKEN_DECIMALS {
            msg!("Invalid mint decimals: expected {}, got {}", TOKEN_DECIMALS, decimals);
            return Err(VestingError::InvalidInstruction.into());
        }
        
        // msg!("Mint decimals: {}", decimals);
        msg!("Preparing to transfer tokens");

        let destination_token_address = get_associated_token_address_with_program_id(
            destination_owner,
            mint_address,
            token_program.key
        );

        let schedules = create_vesting_schedules(start_time, vesting_type.clone(), year, amount);
        let total_amount: u64 = schedules.iter().map(|s| s.amount).sum();

        if total_amount != amount {
            return Err(VestingError::InvalidInstruction.into());
        }

        // BEFORE THE TRANSFER
        // msg!("About to transfer {} tokens from {} to {}", amount, source_token_account.key, vesting_token_account.key);

        let extra_accounts = &[];

        let transfer_accounts = &[
            source_token_account.clone(),
            mint_account.clone(),
            vesting_token_account.clone(),
            source_token_account_owner.clone(),
            token_program.clone(),
        ];

        match invoke(
            &transfer_checked(
                token_program.key,
                source_token_account.key,
                mint_address,
                vesting_token_account.key,
                source_token_account_owner.key,
                extra_accounts,
                amount,
                decimals,
            )?,
            transfer_accounts,
        ) {
            Ok(_) => msg!("Token transfer successful!"),
            Err(err) => {
                msg!("Token transfer failed: {:?}", err);
                return Err(err);
            }
        };


        // Verify and update annual supply status
        let (supply_account_key, supply_bump) = Pubkey::find_program_address(
            &[b"annual_supply", &project_year.to_le_bytes()],
            program_id,
        );

        if *annual_supply_account.key != supply_account_key {
            // msg!("Invalid annual supply account: expected {}, got {}",
            //      supply_account_key, annual_supply_account.key);
            return Err(VestingError::InvalidInstruction.into());
        }

        // If the annual supply account doesn't exist, create it
        if annual_supply_account.data_is_empty() {
            // msg!("Creating annual supply account for year {}", project_year);

            let rent = Rent::get()?;
            let lamports = rent.minimum_balance(AnnualSupplyState::LEN);

            // Seed array structure for invoke_signed
            invoke_signed(
                &solana_program::system_instruction::create_account(
                    source_token_account_owner.key,
                    annual_supply_account.key,
                    lamports,
                    AnnualSupplyState::LEN as u64,
                    program_id,
                ),
                &[
                    source_token_account_owner.clone(),
                    annual_supply_account.clone(),
                    system_program.clone(),
                ],
                &[&[
                    b"annual_supply",
                    &project_year.to_le_bytes(),
                    &[supply_bump],
                ]],
            )?;

            // msg!("Annual supply account created successfully");

            // Initialize annual supply state
            let supply_data = AnnualSupplyState {
                year: project_year,
                market_issued: 0,
                data_purchase_issued: 0,
                team_issued: 0,
                is_initialized: true,
            };

            supply_data.pack_into_slice(&mut annual_supply_account.data.borrow_mut());
        }

        // Read and verify annual supply state
        let mut supply_data = AnnualSupplyState::unpack(&annual_supply_account.data.borrow())?;

        // If it's a new year, reset the annual supply state
        if supply_data.year != project_year {
            supply_data = AnnualSupplyState {
                year: project_year,
                market_issued: 0,
                data_purchase_issued: 0,
                team_issued: 0,
                is_initialized: true,
            };

            // Save the reset state first
            supply_data.pack_into_slice(&mut annual_supply_account.data.borrow_mut());
        }

        // Verify the new issuance doesn't exceed annual quota
        supply_data.validate_issuance(&vesting_type, amount)?;

        // Update annual issued amount
        supply_data.update_issued_amount(&vesting_type, amount)?;

        // Save the updated annual supply state
        supply_data.pack_into_slice(&mut annual_supply_account.data.borrow_mut());

        // msg!("Created vesting for {}", destination_token_address);

        // Log the created issuance
        // msg!("Created vesting of type {:?} for year {}: {} tokens", vesting_type, year, amount);
        // msg!("Theoretical max supply: {}", THEORETICAL_MAX_SUPPLY);

        Ok(())
    }

    pub fn process_unlock(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        vesting_account_key: &Pubkey,
        destination_token_address: &Pubkey,
    ) -> ProgramResult {
        // Check account count
        if accounts.len() != 7 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let accounts_iter = &mut accounts.iter();
        let token_program = next_account_info(accounts_iter)?;
        let clock_sysvar = next_account_info(accounts_iter)?;
        let vesting_account = next_account_info(accounts_iter)?;
        let vesting_token_account = next_account_info(accounts_iter)?;
        let destination_token_account = next_account_info(accounts_iter)?;
        let signer = next_account_info(accounts_iter)?; // Signer (can be creator or recipient)
        let mint_account = next_account_info(accounts_iter)?;

        // Verify account consistency
        if vesting_account.key != vesting_account_key {
            return Err(VestingError::InvalidInstruction.into());
        }

        // Verify destination token account
        if destination_token_account.key != destination_token_address {
            return Err(VestingError::InvalidInstruction.into());
        }

        // Verify header information
        let header = VestingScheduleHeader::unpack(&vesting_account.data.borrow()[..VestingScheduleHeader::LEN])?;
        let timestamp = header.creation_timestamp;

        // Ensure signer is a valid unlocker
        if !signer.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // Verify signer is creator or recipient
        if *signer.key != header.creator_pubkey && *signer.key != header.destination_owner {
            return Err(VestingError::InvalidInstruction.into());
        }

        // Verify destination token account address
        if header.destination_address != *destination_token_address {
            return Err(VestingError::InvalidInstruction.into());
        }

        // Get PDA signing seeds
        let (vesting_account_key, bump_seed) = Pubkey::find_program_address(
            &[b"vesting", &header.destination_owner.to_bytes(), &header.mint_address.to_bytes(), &timestamp.to_le_bytes()],
            program_id,
        );

        if *vesting_account.key != vesting_account_key {
            return Err(VestingError::InvalidInstruction.into());
        }

        // Use seeds to sign for PDA
        let seeds = &[
            b"vesting".as_ref(),
            &header.destination_owner.to_bytes(),
            &header.mint_address.to_bytes(),
            &timestamp.to_le_bytes(),
            &[bump_seed],
        ];

        // Verify vesting_token_account is the ATA of vesting_account
        let expected_vesting_token_account = get_associated_token_address_with_program_id(
            vesting_account.key,
            &header.mint_address,
            token_program.key
        );

        if *vesting_token_account.key != expected_vesting_token_account {
            return Err(VestingError::InvalidInstruction.into());
        }

        // Verify Mint address
        if *mint_account.key != header.mint_address {
            return Err(VestingError::InvalidInstruction.into());
        }

        // Just extract the decimals field which is at a fixed position in both SPL Token and Token-2022
        let decimals = match mint_account.data.borrow().get(44) {
            Some(&decimals) => decimals,
            None => return Err(VestingError::InvalidInstruction.into()),
        };

        let clock = Clock::from_account_info(clock_sysvar)?;
        let mut schedules = unpack_schedules(&vesting_account.data.borrow()[VestingScheduleHeader::LEN..])?;
        let mut total_amount_to_transfer: u64 = 0;

        for s in schedules.iter_mut() {
            if clock.unix_timestamp as u64 >= s.release_time && s.amount > 0 {
                // Use checked_add to prevent overflow
                match total_amount_to_transfer.checked_add(s.amount) {
                    Some(new_total) => {
                        total_amount_to_transfer = new_total;
                        s.amount = 0;
                    },
                    None => {
                        return Err(VestingError::InvalidAmount.into());
                    }
                }
            }
        }

        if total_amount_to_transfer > 0 {
            // Use PDA signing to execute transfer
            invoke_signed(
                &transfer_checked(
                    token_program.key,
                    vesting_token_account.key,
                    &header.mint_address,
                    destination_token_account.key,
                    vesting_account.key,
                    &[], // Empty extra accounts for now
                    total_amount_to_transfer,
                    decimals,
                )?,
                &[
                    vesting_token_account.clone(),
                    mint_account.clone(),
                    destination_token_account.clone(),
                    vesting_account.clone(),
                    token_program.clone(),
                ],
                &[seeds],
            )?;

            // Update unlock status
            pack_schedules_into_slice(schedules, &mut vesting_account.data.borrow_mut()[VestingScheduleHeader::LEN..])?;
            msg!("Unlocked {} tokens to {}", total_amount_to_transfer, destination_token_address);
        } else {
            msg!("No tokens available to unlock at this time");
        }

        Ok(())
    }

    pub fn process_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = VestingInstruction::unpack(instruction_data)?;
        match instruction {
            VestingInstruction::Create {
                mint_address,
                destination_owner,
                vesting_type,
                year,
                start_time,
                amount,
                client_timestamp, // Add client timestamp parameter
            } => Self::process_create(
                program_id,
                accounts,
                &mint_address,
                &destination_owner,
                vesting_type,
                year,
                start_time,
                amount,
                client_timestamp, // Pass the client timestamp
            ),
            VestingInstruction::Unlock {
                vesting_account,
                destination_token_address,
            } => Self::process_unlock(program_id, accounts, &vesting_account, &destination_token_address),
            VestingInstruction::TransferHook => process_transfer_hook(program_id, accounts), // transfer hook implementation is currently a placeholder
        }
    }
}