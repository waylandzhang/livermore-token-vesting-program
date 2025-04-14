/// This file (contains transfer hook functionality) is not used in the project.

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
};

pub fn process_transfer_hook(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    msg!("Transfer hook called - but hooks are disabled, allowing transfer");
    Ok(())
}



