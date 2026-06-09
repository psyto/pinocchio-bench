#![no_std]

use pinocchio::{
    error::ProgramError, no_allocator, nostd_panic_handler, program_entrypoint, AccountView,
    Address, ProgramResult,
};

program_entrypoint!(process_instruction);
no_allocator!();
nostd_panic_handler!();

#[repr(C)]
pub struct PriceFeed {
    pub price: u64,
    pub conf: u64,
    pub ema_price: u64,
    pub last_slot: u64,
    pub publish_count: u64,
}

pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [publisher, price_feed_acc, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !publisher.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if instruction_data.len() < 24 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let new_price = u64::from_le_bytes(instruction_data[0..8].try_into().unwrap());
    let new_conf = u64::from_le_bytes(instruction_data[8..16].try_into().unwrap());
    let new_slot = u64::from_le_bytes(instruction_data[16..24].try_into().unwrap());

    let mut feed_data = price_feed_acc.try_borrow_mut()?;
    if feed_data.len() < core::mem::size_of::<PriceFeed>() {
        return Err(ProgramError::AccountDataTooSmall);
    }
    let feed = unsafe { &mut *(feed_data.as_mut_ptr() as *mut PriceFeed) };

    if new_slot <= feed.last_slot {
        return Err(ProgramError::InvalidArgument);
    }

    if feed.publish_count == 0 {
        feed.ema_price = new_price;
    } else {
        let ema_u128 = feed.ema_price as u128;
        let new_price_u128 = new_price as u128;
        let new_ema = ema_u128
            .saturating_mul(7)
            .saturating_add(new_price_u128)
            / 8;
        feed.ema_price = new_ema as u64;
    }

    feed.price = new_price;
    feed.conf = new_conf;
    feed.last_slot = new_slot;
    feed.publish_count = feed.publish_count.saturating_add(1);

    Ok(())
}
