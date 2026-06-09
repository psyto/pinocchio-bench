use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("Hf89Tqt9FdVdAEsgt3UkmzriXRLPFYqeYE4hHJaSzTjN");

#[program]
pub mod anchor_w8_amm {
    use super::*;

    pub fn swap(ctx: Context<Swap>, amount_in: u64, min_out: u64) -> Result<()> {
        let amount_out_u128 = {
            let mut pool = ctx.accounts.pool.load_mut()?;

            let fee_bps = pool.fee_bps as u128;
            let amount_in_u128 = amount_in as u128;
            let amount_in_after_fee = amount_in_u128
                .saturating_mul(10_000u128.saturating_sub(fee_bps))
                / 10_000u128;
            let reserve_in = pool.reserve_in as u128;
            let reserve_out = pool.reserve_out as u128;
            let denom = reserve_in.saturating_add(amount_in_after_fee);
            require!(denom > 0, AmmErr::ZeroDenominator);
            let amount_out = reserve_out
                .saturating_mul(amount_in_after_fee)
                / denom;

            require!(amount_out > 0, AmmErr::ZeroOutput);
            require!((amount_out as u64) >= min_out, AmmErr::SlippageExceeded);

            pool.reserve_in = (reserve_in + amount_in_u128) as u64;
            pool.reserve_out = (reserve_out - amount_out) as u64;

            amount_out
        };
        let amount_out = amount_out_u128 as u64;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_src.to_account_info(),
                    to: ctx.accounts.pool_vault_in.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            amount_in,
        )?;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.pool_vault_out.to_account_info(),
                    to: ctx.accounts.user_dst.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            amount_out,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Swap<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub pool: AccountLoader<'info, Pool>,
    #[account(mut)]
    pub user_src: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_dst: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_vault_in: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_vault_out: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[account(zero_copy)]
#[repr(C)]
pub struct Pool {
    pub reserve_in: u64,
    pub reserve_out: u64,
    pub fee_bps: u16,
    pub _pad: [u8; 6],
}

#[error_code]
pub enum AmmErr {
    #[msg("Slippage exceeded")]
    SlippageExceeded,
    #[msg("Zero output")]
    ZeroOutput,
    #[msg("Zero denominator")]
    ZeroDenominator,
}
