use anchor_lang::prelude::*;

declare_id!("2xBkAYW7smqE3a5uxVcarGDHLeiqFgJDnp8r2ZZhPiM2");

#[program]
pub mod anchor_w0_noop {
    use super::*;

    pub fn noop(_ctx: Context<Noop>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Noop {}
