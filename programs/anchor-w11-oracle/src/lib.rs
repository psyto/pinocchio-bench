use anchor_lang::prelude::*;

declare_id!("1PA7z3xmC4WdLzc5frbUSuDynRNPPvPPyJNFPdFSmu5");

#[program]
pub mod anchor_w11_oracle {
    use super::*;

    pub fn publish_price(
        ctx: Context<PublishPrice>,
        new_price: u64,
        new_conf: u64,
        new_slot: u64,
    ) -> Result<()> {
        let mut feed = ctx.accounts.price_feed.load_mut()?;

        require!(new_slot > feed.last_slot, OracleErr::StaleSlot);

        // EMA with α = 1/8 (smoothing factor): ema = (ema * 7 + price) / 8
        // First publish bootstraps EMA to the new price.
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
}

#[derive(Accounts)]
pub struct PublishPrice<'info> {
    pub publisher: Signer<'info>,
    #[account(mut)]
    pub price_feed: AccountLoader<'info, PriceFeed>,
}

#[account(zero_copy)]
#[repr(C)]
pub struct PriceFeed {
    pub price: u64,
    pub conf: u64,
    pub ema_price: u64,
    pub last_slot: u64,
    pub publish_count: u64,
}

#[error_code]
pub enum OracleErr {
    #[msg("New slot must be strictly greater than the last published slot")]
    StaleSlot,
}
