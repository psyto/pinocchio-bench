use anchor_lang::prelude::*;

declare_id!("F84VDYJd5ukacECaHVkR6QJR1rD9nGmd2AJUw3qDvMN2");

pub const N_TICKS: usize = 32;
pub const TICK_DEPTH: usize = 4;

#[program]
pub mod anchor_w4_matching {
    use super::*;

    pub fn place_order(ctx: Context<PlaceOrder>, price: u64, qty: u64) -> Result<()> {
        let mut market = ctx.accounts.market.load_mut()?;
        let mut book = ctx.accounts.book.load_mut()?;
        let signer_pk = ctx.accounts.signer.key().to_bytes();

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
            require!((tick.n_orders as usize) < TICK_DEPTH, MatchErr::TickFull);
            let idx = tick.n_orders as usize;
            tick.orders[idx] = Order {
                owner_pk: signer_pk,
                qty,
                sequence: seq,
            };
            tick.n_orders += 1;
        } else {
            require!(count < N_TICKS, MatchErr::BookFull);
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
}

#[derive(Accounts)]
pub struct PlaceOrder<'info> {
    pub signer: Signer<'info>,
    #[account(mut)]
    pub market: AccountLoader<'info, Market>,
    #[account(mut)]
    pub book: AccountLoader<'info, Book>,
}

#[account(zero_copy)]
#[repr(C)]
pub struct Market {
    pub sequence: u64,
    pub side: u8,
    pub _pad: [u8; 7],
}

#[account(zero_copy)]
#[repr(C)]
pub struct Book {
    pub count: u32,
    pub _pad: u32,
    pub ticks: [Tick; N_TICKS],
}

#[zero_copy]
#[repr(C)]
pub struct Tick {
    pub price: u64,
    pub n_orders: u32,
    pub _pad: u32,
    pub orders: [Order; TICK_DEPTH],
}

#[zero_copy]
#[repr(C)]
pub struct Order {
    pub owner_pk: [u8; 32],
    pub qty: u64,
    pub sequence: u64,
}

#[error_code]
pub enum MatchErr {
    #[msg("Tick full")]
    TickFull,
    #[msg("Book full")]
    BookFull,
}
