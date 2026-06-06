use anchor_lang::prelude::*;

declare_id!("3uVUZLsj8y7fPNpE6WksSjsZuLxUspL3pxKKeqbsnSQr");

#[program]
pub mod anchor_w7_hook {
    use super::*;

    /// SPL Transfer Hook `execute` instruction — no-op body.
    /// The `#[interface]` attribute rewrites the dispatch discriminator from
    /// the default `sha256("global:execute_transfer_hook")[..8]` to
    /// `sha256("spl-transfer-hook-interface:execute")[..8]`, which is what
    /// Token-2022 invokes.
    #[interface(spl_transfer_hook_interface::execute)]
    pub fn execute_transfer_hook(_ctx: Context<ExecuteHook>, _amount: u64) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ExecuteHook<'info> {
    /// CHECK: source token account (writable in Token-2022 CPI but readonly here)
    pub source: UncheckedAccount<'info>,
    /// CHECK: mint
    pub mint: UncheckedAccount<'info>,
    /// CHECK: destination token account
    pub destination: UncheckedAccount<'info>,
    /// CHECK: authority (signer or PDA)
    pub authority: UncheckedAccount<'info>,
    /// CHECK: extra account metas PDA
    pub extra_metas: UncheckedAccount<'info>,
}
