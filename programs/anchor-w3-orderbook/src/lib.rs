use anchor_lang::prelude::*;

declare_id!("7bTBRzPCg2tkq9vLHsKzPt5L8d3KYG7A1HuauwAKsGwV");

pub const TICK_CAPACITY: usize = 64;

#[program]
pub mod anchor_w3_orderbook {
    use super::*;

    pub fn insert(ctx: Context<Insert>, price: u64, qty: u64) -> Result<()> {
        let mut book = ctx.accounts.book.load_mut()?;
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
            require!(count < TICK_CAPACITY, OrderBookErr::Full);
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
}

#[derive(Accounts)]
pub struct Insert<'info> {
    pub signer: Signer<'info>,
    #[account(mut)]
    pub book: AccountLoader<'info, OrderBook>,
}

#[account(zero_copy)]
#[repr(C)]
pub struct OrderBook {
    pub count: u64,
    pub ticks: [Tick; TICK_CAPACITY],
}

#[zero_copy]
#[repr(C)]
pub struct Tick {
    pub price: u64,
    pub qty: u64,
}

#[error_code]
pub enum OrderBookErr {
    #[msg("Orderbook full")]
    Full,
}
