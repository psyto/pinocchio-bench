use anchor_lang::prelude::*;

declare_id!("FLf2M1PEPVGXJFbwwPQg8REViTG6YpK4UoMCd22rsSey");

#[program]
pub mod anchor_w1_write {
    use super::*;

    pub fn write(ctx: Context<DoWrite>, value: u64) -> Result<()> {
        ctx.accounts.state.value = value;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct DoWrite<'info> {
    pub signer: Signer<'info>,
    #[account(mut)]
    pub state: Account<'info, State>,
}

#[account]
pub struct State {
    pub value: u64,
}
