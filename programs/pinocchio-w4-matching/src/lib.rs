#![no_std]

use pinocchio::{
    error::ProgramError, no_allocator, nostd_panic_handler, program_entrypoint, AccountView,
    Address, ProgramResult,
};

program_entrypoint!(process_instruction);
no_allocator!();
nostd_panic_handler!();

pub const N_TICKS: usize = 32;
pub const TICK_DEPTH: usize = 4;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Order {
    pub owner_pk: [u8; 32],
    pub qty: u64,
    pub sequence: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Tick {
    pub price: u64,
    pub n_orders: u32,
    pub _pad: u32,
    pub orders: [Order; TICK_DEPTH],
}

#[repr(C)]
pub struct Market {
    pub sequence: u64,
    pub side: u8,
    pub _pad: [u8; 7],
}

#[repr(C)]
pub struct Book {
    pub count: u32,
    pub _pad: u32,
    pub ticks: [Tick; N_TICKS],
}

pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [signer, market_acc, book_acc, ..] = accounts else {
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

    let signer_pk = signer.address().to_bytes();

    let mut market_data = market_acc.try_borrow_mut()?;
    if market_data.len() < core::mem::size_of::<Market>() {
        return Err(ProgramError::AccountDataTooSmall);
    }
    let market = unsafe { &mut *(market_data.as_mut_ptr() as *mut Market) };

    let mut book_data = book_acc.try_borrow_mut()?;
    if book_data.len() < core::mem::size_of::<Book>() {
        return Err(ProgramError::AccountDataTooSmall);
    }
    let book = unsafe { &mut *(book_data.as_mut_ptr() as *mut Book) };

    market.sequence = market.sequence.saturating_add(1);
    let seq = market.sequence;

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
        let tick = &mut book.ticks[lo];
        if (tick.n_orders as usize) >= TICK_DEPTH {
            return Err(ProgramError::InvalidArgument);
        }
        let idx = tick.n_orders as usize;
        tick.orders[idx] = Order {
            owner_pk: signer_pk,
            qty,
            sequence: seq,
        };
        tick.n_orders += 1;
    } else {
        if count >= N_TICKS {
            return Err(ProgramError::InvalidArgument);
        }
        let mut i = count;
        while i > lo {
            book.ticks[i] = book.ticks[i - 1];
            i -= 1;
        }
        let first = Order {
            owner_pk: signer_pk,
            qty,
            sequence: seq,
        };
        let zero = Order {
            owner_pk: [0u8; 32],
            qty: 0,
            sequence: 0,
        };
        book.ticks[lo] = Tick {
            price,
            n_orders: 1,
            _pad: 0,
            orders: [first, zero, zero, zero],
        };
        book.count = (count as u32) + 1;
    }
    Ok(())
}
