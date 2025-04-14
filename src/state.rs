use solana_program::{
    program_error::ProgramError,
    program_pack::{Pack, Sealed},
    pubkey::Pubkey,
};
use arrayref::{array_mut_ref, array_ref};

#[derive(Clone, Debug, PartialEq)]
pub struct VestingSchedule {
    pub release_time: u64,
    pub amount: u64,
}

impl Sealed for VestingSchedule {}
impl Pack for VestingSchedule {
    const LEN: usize = 16; // 8 bytes for release_time + 8 bytes for amount

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, VestingSchedule::LEN];
        let (release_time_dst, amount_dst) = dst.split_at_mut(8);
        release_time_dst.copy_from_slice(&self.release_time.to_le_bytes());
        amount_dst.copy_from_slice(&self.amount.to_le_bytes());
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, VestingSchedule::LEN];
        let (release_time_src, amount_src) = src.split_at(8);
        Ok(VestingSchedule {
            release_time: u64::from_le_bytes(release_time_src.try_into().unwrap()),
            amount: u64::from_le_bytes(amount_src.try_into().unwrap()),
        })
    }
}

/// Represents a single vesting schedule entry.
/// - `release_time`: Unix timestamp when tokens are available for release.
/// - `amount`: Amount of tokens to release.
#[derive(Clone, Debug, PartialEq)]
pub struct VestingScheduleHeader {
    pub destination_address: Pubkey,    // Token destination address
    pub mint_address: Pubkey,           // Token mint address
    pub creator_pubkey: Pubkey,         // Creator public key
    pub destination_owner: Pubkey,      // Token destination owner
    pub is_initialized: bool,           // Whether the schedule is initialized
    pub creation_timestamp: u64,        // Timestamp of schedule creation
}

impl Sealed for VestingScheduleHeader {}
impl Pack for VestingScheduleHeader {
    /*
    32 bytes for destination_address (Pubkey)
    32 bytes for mint_address (Pubkey)
    32 bytes for creator_pubkey (Pubkey)
    32 bytes for destination_owner (Pubkey)
    1 byte for is_initialized (boolean)
    8 bytes for creation_timestamp (u64)
    = 105 bytes
     */
    const LEN: usize = 137;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, VestingScheduleHeader::LEN];
        let (dest_addr_dst, rest) = dst.split_at_mut(32);
        let (mint_addr_dst, rest) = rest.split_at_mut(32);
        let (creator_pubkey_dst, rest) = rest.split_at_mut(32);
        let (dest_owner_dst, rest) = rest.split_at_mut(32);
        let (is_init_dst, timestamp_dst) = rest.split_at_mut(1);

        dest_addr_dst.copy_from_slice(&self.destination_address.to_bytes());
        mint_addr_dst.copy_from_slice(&self.mint_address.to_bytes());
        creator_pubkey_dst.copy_from_slice(&self.creator_pubkey.to_bytes());
        dest_owner_dst.copy_from_slice(&self.destination_owner.to_bytes());
        is_init_dst[0] = self.is_initialized as u8;
        timestamp_dst.copy_from_slice(&self.creation_timestamp.to_le_bytes());
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, VestingScheduleHeader::LEN];
        let (dest_addr_src, rest) = src.split_at(32);
        let (mint_addr_src, rest) = rest.split_at(32);
        let (creator_pubkey_src, rest) = rest.split_at(32);
        let (dest_owner_src, rest) = rest.split_at(32);
        let (is_init_src, timestamp_src) = rest.split_at(1);

        Ok(VestingScheduleHeader {
            destination_address: Pubkey::new_from_array(dest_addr_src.try_into().unwrap()),
            mint_address: Pubkey::new_from_array(mint_addr_src.try_into().unwrap()),
            creator_pubkey: Pubkey::new_from_array(creator_pubkey_src.try_into().unwrap()),
            destination_owner: Pubkey::new_from_array(dest_owner_src.try_into().unwrap()),
            is_initialized: is_init_src[0] != 0,
            creation_timestamp: u64::from_le_bytes(timestamp_src.try_into().unwrap()),
        })
    }
}

pub fn pack_schedules_into_slice(schedules: Vec<VestingSchedule>, dst: &mut [u8]) -> Result<(), ProgramError> {
    if dst.len() < schedules.len() * VestingSchedule::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    for (i, schedule) in schedules.iter().enumerate() {
        let start = i * VestingSchedule::LEN;
        let end = start + VestingSchedule::LEN;
        schedule.pack_into_slice(&mut dst[start..end]);
    }
    Ok(())
}

pub fn unpack_schedules(src: &[u8]) -> Result<Vec<VestingSchedule>, ProgramError> {
    if src.len() % VestingSchedule::LEN != 0 {
        return Err(ProgramError::InvalidAccountData);
    }

    let count = src.len() / VestingSchedule::LEN;
    let mut schedules = Vec::with_capacity(count);
    for i in 0..count {
        let start = i * VestingSchedule::LEN;
        let end = start + VestingSchedule::LEN;
        schedules.push(VestingSchedule::unpack_from_slice(&src[start..end])?);
    }
    Ok(schedules)
}