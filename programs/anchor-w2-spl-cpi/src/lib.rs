use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("4fGGsS5fYeQ8VJfcR7eB2KNaYiYJvVEEqVC5t4EskB73");

#[program]
pub mod anchor_w2_spl_cpi {
    use super::*;

    pub fn do_transfer(ctx: Context<DoTransfer>, amount: u64) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.source.to_account_info(),
                to: ctx.accounts.destination.to_account_info(),
                authority: ctx.accounts.authority.to_account_info(),
            },
        );
        token::transfer(cpi_ctx, amount)
    }
}

#[derive(Accounts)]
pub struct DoTransfer<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub source: Account<'info, TokenAccount>,
    #[account(mut)]
    pub destination: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
