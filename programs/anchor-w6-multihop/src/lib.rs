use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("258rXi3tqTFeWe7DkcheLPtFdpb2MzSDvHVBMERETFHR");

#[program]
pub mod anchor_w6_multihop {
    use super::*;

    pub fn three_hop_transfer(ctx: Context<ThreeHopTransfer>, amount: u64) -> Result<()> {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.src1.to_account_info(),
                    to: ctx.accounts.dst1.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            amount,
        )?;
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.src2.to_account_info(),
                    to: ctx.accounts.dst2.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            amount,
        )?;
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.src3.to_account_info(),
                    to: ctx.accounts.dst3.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            amount,
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ThreeHopTransfer<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub src1: Account<'info, TokenAccount>,
    #[account(mut)]
    pub dst1: Account<'info, TokenAccount>,
    #[account(mut)]
    pub src2: Account<'info, TokenAccount>,
    #[account(mut)]
    pub dst2: Account<'info, TokenAccount>,
    #[account(mut)]
    pub src3: Account<'info, TokenAccount>,
    #[account(mut)]
    pub dst3: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
