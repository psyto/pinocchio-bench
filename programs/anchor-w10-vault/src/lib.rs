use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("2N5cmNMVnqrQDWaKE2oP92bVDwMvGNW69k7mpQfyyiMh");

#[program]
pub mod anchor_w10_vault {
    use super::*;

    pub fn deposit(ctx: Context<Deposit>, deposit_amount: u64) -> Result<()> {
        let shares_to_mint = {
            let mut vault = ctx.accounts.vault.load_mut()?;
            let mut user_position = ctx.accounts.user_position.load_mut()?;

            require!(deposit_amount > 0, VaultErr::ZeroDeposit);

            let shares = if vault.total_shares == 0 {
                deposit_amount
            } else {
                require!(vault.total_assets > 0, VaultErr::InvariantBroken);
                let deposit_u128 = deposit_amount as u128;
                let total_shares_u128 = vault.total_shares as u128;
                let total_assets_u128 = vault.total_assets as u128;
                let s = deposit_u128
                    .saturating_mul(total_shares_u128)
                    / total_assets_u128;
                require!(s > 0, VaultErr::ZeroShares);
                s as u64
            };

            vault.total_assets = vault
                .total_assets
                .saturating_add(deposit_amount);
            vault.total_shares = vault.total_shares.saturating_add(shares);
            user_position.share_amount =
                user_position.share_amount.saturating_add(shares);
            user_position.deposit_count =
                user_position.deposit_count.saturating_add(1);

            shares
        };

        // Move the underlying tokens into the vault's holding account.
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_underlying.to_account_info(),
                    to: ctx.accounts.vault_underlying.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            deposit_amount,
        )?;

        let _ = shares_to_mint; // suppress unused warning; kept for future logging
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub vault: AccountLoader<'info, Vault>,
    #[account(mut)]
    pub user_position: AccountLoader<'info, UserPosition>,
    #[account(mut)]
    pub user_underlying: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_underlying: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[account(zero_copy)]
#[repr(C)]
pub struct Vault {
    pub total_assets: u64,
    pub total_shares: u64,
}

#[account(zero_copy)]
#[repr(C)]
pub struct UserPosition {
    pub share_amount: u64,
    pub deposit_count: u64,
}

#[error_code]
pub enum VaultErr {
    #[msg("Zero deposit")]
    ZeroDeposit,
    #[msg("Zero shares")]
    ZeroShares,
    #[msg("Invariant broken: total_shares > 0 but total_assets == 0")]
    InvariantBroken,
}
