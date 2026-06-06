#![no_std]

use pinocchio::{
    error::ProgramError, no_allocator, nostd_panic_handler, program_entrypoint, AccountView,
    Address, ProgramResult,
};

program_entrypoint!(process_instruction);
no_allocator!();
nostd_panic_handler!();

pub const TICK_CAPACITY: usize = 64;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Tick {
    pub price: u64,
    pub qty: u64,
}

#[repr(C)]
pub struct OrderBook {
    pub count: u64,
    pub ticks: [Tick; TICK_CAPACITY],
}

pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [signer, book_acc, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !signer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if instruction_data.len() < 16 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let price = u64::from_le_bytes(instruction_data[0..8].try_into().unwrap());
    let qty = u64::from_le_bytes(instruction_data[8..16].try_into().unwrap());

    let mut data = book_acc.try_borrow_mut()?;
    if data.len() < core::mem::size_of::<OrderBook>() {
        return Err(ProgramError::AccountDataTooSmall);
    }
    let book = unsafe { &mut *(data.as_mut_ptr() as *mut OrderBook) };

    let count = book.count as usize;
    let mut lo = 0usize;
    let mut hi = count;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if book.ticks[mid].price < price {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    if lo < count && book.ticks[lo].price == price {
        book.ticks[lo].qty = book.ticks[lo].qty.saturating_add(qty);
    } else {
        if count >= TICK_CAPACITY {
            return Err(ProgramError::InvalidArgument);
        }
        let mut i = count;
        while i > lo {
            book.ticks[i] = book.ticks[i - 1];
            i -= 1;
        }
        book.ticks[lo] = Tick { price, qty };
        book.count = (count as u64) + 1;
    }
    Ok(())
}
