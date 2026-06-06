#![no_std]

use pinocchio::{
    error::ProgramError, no_allocator, nostd_panic_handler, program_entrypoint, AccountView,
    Address, ProgramResult,
};

program_entrypoint!(process_instruction);
no_allocator!();
nostd_panic_handler!();

// First 8 bytes of sha256("spl-transfer-hook-interface:execute")
// (precomputed; matches what Token-2022 uses to dispatch the hook).
const EXECUTE_DISCRIMINATOR: [u8; 8] = [105, 37, 101, 197, 75, 251, 102, 26];

pub fn process_instruction(
    _program_id: &Address,
    _accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    if instruction_data[..8] != EXECUTE_DISCRIMINATOR {
        return Err(ProgramError::InvalidInstructionData);
    }
    // No-op hook body.
    Ok(())
}
