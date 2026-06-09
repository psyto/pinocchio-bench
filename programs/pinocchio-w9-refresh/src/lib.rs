#![no_std]

use pinocchio::{
    error::ProgramError, no_allocator, nostd_panic_handler, program_entrypoint, AccountView,
    Address, ProgramResult,
};

program_entrypoint!(process_instruction);
no_allocator!();
nostd_panic_handler!();

#[repr(C)]
pub struct Obligation {
    pub deposit_amount: u64,
    pub borrow_amount: u64,
    pub last_health: u64,
    pub last_update_slot: u64,
}

#[repr(C)]
pub struct Reserve {
    pub total_liquidity: u64,
    pub total_borrows: u64,
    pub cumulative_borrow_rate: u64,
    pub borrow_rate_bps: u32,
    pub _pad: u32,
    pub last_update_slot: u64,
}

#[repr(C)]
pub struct Oracle {
    pub price: u64,
    pub conf: u64,
    pub last_update_slot: u64,
}

pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [signer, obligation_acc, reserve_a_acc, reserve_b_acc, oracle_a_acc, oracle_b_acc, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !signer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if instruction_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let current_slot = u64::from_le_bytes(instruction_data[0..8].try_into().unwrap());

    let mut obligation_data = obligation_acc.try_borrow_mut()?;
    if obligation_data.len() < core::mem::size_of::<Obligation>() {
        return Err(ProgramError::AccountDataTooSmall);
    }
    let obligation = unsafe { &mut *(obligation_data.as_mut_ptr() as *mut Obligation) };

    let mut reserve_a_data = reserve_a_acc.try_borrow_mut()?;
    if reserve_a_data.len() < core::mem::size_of::<Reserve>() {
        return Err(ProgramError::AccountDataTooSmall);
    }
    let reserve_a = unsafe { &mut *(reserve_a_data.as_mut_ptr() as *mut Reserve) };

    let mut reserve_b_data = reserve_b_acc.try_borrow_mut()?;
    if reserve_b_data.len() < core::mem::size_of::<Reserve>() {
        return Err(ProgramError::AccountDataTooSmall);
    }
    let reserve_b = unsafe { &mut *(reserve_b_data.as_mut_ptr() as *mut Reserve) };

    let mut oracle_a_data = oracle_a_acc.try_borrow_mut()?;
    if oracle_a_data.len() < core::mem::size_of::<Oracle>() {
        return Err(ProgramError::AccountDataTooSmall);
    }
    let oracle_a = unsafe { &mut *(oracle_a_data.as_mut_ptr() as *mut Oracle) };

    let mut oracle_b_data = oracle_b_acc.try_borrow_mut()?;
    if oracle_b_data.len() < core::mem::size_of::<Oracle>() {
        return Err(ProgramError::AccountDataTooSmall);
    }
    let oracle_b = unsafe { &mut *(oracle_b_data.as_mut_ptr() as *mut Oracle) };

    let delta_a = current_slot.saturating_sub(reserve_a.last_update_slot);
    reserve_a.cumulative_borrow_rate = reserve_a
        .cumulative_borrow_rate
        .saturating_add(delta_a.saturating_mul(reserve_a.borrow_rate_bps as u64));
    reserve_a.last_update_slot = current_slot;

    let delta_b = current_slot.saturating_sub(reserve_b.last_update_slot);
    reserve_b.cumulative_borrow_rate = reserve_b
        .cumulative_borrow_rate
        .saturating_add(delta_b.saturating_mul(reserve_b.borrow_rate_bps as u64));
    reserve_b.last_update_slot = current_slot;

    oracle_a.last_update_slot = current_slot;
    oracle_b.last_update_slot = current_slot;

    let collateral_value =
        (obligation.deposit_amount as u128).saturating_mul(oracle_a.price as u128);
    let debt_value = (obligation.borrow_amount as u128)
        .saturating_mul(oracle_b.price as u128)
        .max(1);
    obligation.last_health = (collateral_value.saturating_mul(10_000) / debt_value) as u64;
    obligation.last_update_slot = current_slot;

    Ok(())
}
