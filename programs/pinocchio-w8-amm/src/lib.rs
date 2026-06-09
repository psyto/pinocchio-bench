#![no_std]

use pinocchio::{
    error::ProgramError, no_allocator, nostd_panic_handler, program_entrypoint, AccountView,
    Address, ProgramResult,
};
use pinocchio_token::instructions::Transfer;

program_entrypoint!(process_instruction);
no_allocator!();
nostd_panic_handler!();

#[repr(C)]
pub struct Pool {
    pub reserve_in: u64,
    pub reserve_out: u64,
    pub fee_bps: u16,
    pub _pad: [u8; 6],
}

pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [authority, pool_acc, user_src, user_dst, pool_vault_in, pool_vault_out, _token_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if instruction_data.len() < 16 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let amount_in = u64::from_le_bytes(instruction_data[0..8].try_into().unwrap());
    let min_out = u64::from_le_bytes(instruction_data[8..16].try_into().unwrap());

    let amount_out: u64;
    {
        let mut pool_data = pool_acc.try_borrow_mut()?;
        if pool_data.len() < core::mem::size_of::<Pool>() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let pool = unsafe { &mut *(pool_data.as_mut_ptr() as *mut Pool) };

        let fee_bps = pool.fee_bps as u128;
        let amount_in_u128 = amount_in as u128;
        let amount_in_after_fee =
            amount_in_u128.saturating_mul(10_000u128.saturating_sub(fee_bps)) / 10_000u128;
        let reserve_in = pool.reserve_in as u128;
        let reserve_out = pool.reserve_out as u128;
        let denom = reserve_in.saturating_add(amount_in_after_fee);
        if denom == 0 {
            return Err(ProgramError::InvalidArgument);
        }
        let amount_out_u128 = reserve_out.saturating_mul(amount_in_after_fee) / denom;
        if amount_out_u128 == 0 {
            return Err(ProgramError::InvalidArgument);
        }
        amount_out = amount_out_u128 as u64;
        if amount_out < min_out {
            return Err(ProgramError::InvalidArgument);
        }

        pool.reserve_in = (reserve_in + amount_in_u128) as u64;
        pool.reserve_out = (reserve_out - amount_out_u128) as u64;
    }

    Transfer::new(user_src, pool_vault_in, authority, amount_in).invoke()?;
    Transfer::new(pool_vault_out, user_dst, authority, amount_out).invoke()?;

    Ok(())
}
