use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("AUe7aMBwQieB84WLK2CpbySsiQdjU5E3D3xmtY4s1vNd");

#[program]
pub mod anchor_w12_perp {
    use super::*;

    pub fn open_position(
        ctx: Context<OpenPosition>,
        position_size: u64,
        current_slot: u64,
    ) -> Result<()> {
        let fee = {
            let mut user = ctx.accounts.user.load_mut()?;
            let mut market = ctx.accounts.perp_market.load_mut()?;
            let mut oracle = ctx.accounts.oracle.load_mut()?;

            require!(user.position_size == 0, PerpErr::PositionAlreadyOpen);
            require!(position_size > 0, PerpErr::ZeroSize);

            // Refresh perp market's view of mark price from the oracle.
            market.mark_price = oracle.mark_price;
            oracle.last_update_slot = current_slot;

            // Compute and debit the fee FIRST, then check margin against
            // the post-fee collateral. This matches the Drift design where
            // margin solvency is verified against the actual reserve a
            // user has after costs — checking pre-fee leaves a window
            // where the margin invariant would be violated immediately
            // after the instruction.
            let computed_fee = (position_size as u128)
                .saturating_mul(market.fee_bps as u128)
                / 10_000u128;
            let computed_fee = computed_fee as u64;
            require!(user.collateral >= computed_fee, PerpErr::InsufficientCollateral);
            let post_fee_collateral = user.collateral.saturating_sub(computed_fee);

            let max_notional = (post_fee_collateral as u128)
                .saturating_mul(market.max_leverage_bps as u128)
                / 10_000u128;
            require!(
                max_notional >= position_size as u128,
                PerpErr::InsufficientMargin,
            );

            user.position_size = position_size;
            user.entry_price = oracle.mark_price;
            user.last_update_slot = current_slot;
            user.collateral = post_fee_collateral;

            market.open_interest = market.open_interest.saturating_add(position_size);

            computed_fee
        };

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token.to_account_info(),
                    to: ctx.accounts.fee_vault.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            fee,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct OpenPosition<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub user: AccountLoader<'info, UserPerp>,
    #[account(mut)]
    pub perp_market: AccountLoader<'info, PerpMarket>,
    #[account(mut)]
    pub oracle: AccountLoader<'info, MarketOracle>,
    #[account(mut)]
    pub user_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub fee_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[account(zero_copy)]
#[repr(C)]
pub struct UserPerp {
    pub collateral: u64,
    pub position_size: u64,
    pub entry_price: u64,
    pub last_update_slot: u64,
}

#[account(zero_copy)]
#[repr(C)]
pub struct PerpMarket {
    pub open_interest: u64,
    pub mark_price: u64,
    pub max_leverage_bps: u32,
    pub fee_bps: u32,
}

#[account(zero_copy)]
#[repr(C)]
pub struct MarketOracle {
    pub mark_price: u64,
    pub last_update_slot: u64,
}

#[error_code]
pub enum PerpErr {
    #[msg("Position already open")]
    PositionAlreadyOpen,
    #[msg("Zero position size")]
    ZeroSize,
    #[msg("Insufficient margin for position size")]
    InsufficientMargin,
    #[msg("Insufficient collateral for fee")]
    InsufficientCollateral,
}
