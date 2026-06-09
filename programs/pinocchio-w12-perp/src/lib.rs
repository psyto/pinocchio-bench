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
pub struct UserPerp {
    pub collateral: u64,
    pub position_size: u64,
    pub entry_price: u64,
    pub last_update_slot: u64,
}

#[repr(C)]
pub struct PerpMarket {
    pub open_interest: u64,
    pub mark_price: u64,
    pub max_leverage_bps: u32,
    pub fee_bps: u32,
}

#[repr(C)]
pub struct MarketOracle {
    pub mark_price: u64,
    pub last_update_slot: u64,
}

pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [authority, user_acc, market_acc, oracle_acc, user_token, fee_vault, _token_program, ..] =
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
    let position_size = u64::from_le_bytes(instruction_data[0..8].try_into().unwrap());
    let current_slot = u64::from_le_bytes(instruction_data[8..16].try_into().unwrap());

    if position_size == 0 {
        return Err(ProgramError::InvalidArgument);
    }

    let fee: u64;
    {
        let mut user_data = user_acc.try_borrow_mut()?;
        if user_data.len() < core::mem::size_of::<UserPerp>() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let user = unsafe { &mut *(user_data.as_mut_ptr() as *mut UserPerp) };

        let mut market_data = market_acc.try_borrow_mut()?;
        if market_data.len() < core::mem::size_of::<PerpMarket>() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let market = unsafe { &mut *(market_data.as_mut_ptr() as *mut PerpMarket) };

        let mut oracle_data = oracle_acc.try_borrow_mut()?;
        if oracle_data.len() < core::mem::size_of::<MarketOracle>() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let oracle = unsafe { &mut *(oracle_data.as_mut_ptr() as *mut MarketOracle) };

        if user.position_size != 0 {
            return Err(ProgramError::InvalidArgument);
        }

        market.mark_price = oracle.mark_price;
        oracle.last_update_slot = current_slot;

        let computed_fee = (position_size as u128)
            .saturating_mul(market.fee_bps as u128)
            / 10_000u128;
        let computed_fee = computed_fee as u64;
        if user.collateral < computed_fee {
            return Err(ProgramError::InvalidArgument);
        }
        let post_fee_collateral = user.collateral.saturating_sub(computed_fee);

        let max_notional = (post_fee_collateral as u128)
            .saturating_mul(market.max_leverage_bps as u128)
            / 10_000u128;
        if max_notional < position_size as u128 {
            return Err(ProgramError::InvalidArgument);
        }

        user.position_size = position_size;
        user.entry_price = oracle.mark_price;
        user.last_update_slot = current_slot;
        user.collateral = post_fee_collateral;

        market.open_interest = market.open_interest.saturating_add(position_size);

        fee = computed_fee;
    }

    Transfer::new(user_token, fee_vault, authority, fee).invoke()?;

    Ok(())
}
