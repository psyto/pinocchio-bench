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
pub struct Vault {
    pub total_assets: u64,
    pub total_shares: u64,
}

#[repr(C)]
pub struct UserPosition {
    pub share_amount: u64,
    pub deposit_count: u64,
}

pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [authority, vault_acc, user_position_acc, user_underlying, vault_underlying, _token_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if instruction_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let deposit_amount = u64::from_le_bytes(instruction_data[0..8].try_into().unwrap());
    if deposit_amount == 0 {
        return Err(ProgramError::InvalidArgument);
    }

    {
        let mut vault_data = vault_acc.try_borrow_mut()?;
        if vault_data.len() < core::mem::size_of::<Vault>() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let vault = unsafe { &mut *(vault_data.as_mut_ptr() as *mut Vault) };

        let mut user_position_data = user_position_acc.try_borrow_mut()?;
        if user_position_data.len() < core::mem::size_of::<UserPosition>() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let user_position =
            unsafe { &mut *(user_position_data.as_mut_ptr() as *mut UserPosition) };

        let shares = if vault.total_shares == 0 {
            deposit_amount
        } else {
            if vault.total_assets == 0 {
                return Err(ProgramError::InvalidArgument);
            }
            let deposit_u128 = deposit_amount as u128;
            let total_shares_u128 = vault.total_shares as u128;
            let total_assets_u128 = vault.total_assets as u128;
            let s = deposit_u128.saturating_mul(total_shares_u128) / total_assets_u128;
            if s == 0 {
                return Err(ProgramError::InvalidArgument);
            }
            s as u64
        };

        vault.total_assets = vault.total_assets.saturating_add(deposit_amount);
        vault.total_shares = vault.total_shares.saturating_add(shares);
        user_position.share_amount = user_position.share_amount.saturating_add(shares);
        user_position.deposit_count = user_position.deposit_count.saturating_add(1);
    }

    Transfer::new(user_underlying, vault_underlying, authority, deposit_amount).invoke()?;

    Ok(())
}
