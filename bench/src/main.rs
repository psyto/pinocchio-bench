use litesvm::LiteSVM;
use sha2::{Digest, Sha256};
use solana_sdk::{
    account::Account as SolAccount,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

const ANCHOR_W0: &str = "2xBkAYW7smqE3a5uxVcarGDHLeiqFgJDnp8r2ZZhPiM2";
const ANCHOR_W1: &str = "FLf2M1PEPVGXJFbwwPQg8REViTG6YpK4UoMCd22rsSey";
const ANCHOR_W2: &str = "4fGGsS5fYeQ8VJfcR7eB2KNaYiYJvVEEqVC5t4EskB73";
const ANCHOR_W3: &str = "7bTBRzPCg2tkq9vLHsKzPt5L8d3KYG7A1HuauwAKsGwV";
const ANCHOR_W4: &str = "F84VDYJd5ukacECaHVkR6QJR1rD9nGmd2AJUw3qDvMN2";
const ANCHOR_W6: &str = "258rXi3tqTFeWe7DkcheLPtFdpb2MzSDvHVBMERETFHR";
const ANCHOR_W7: &str = "3uVUZLsj8y7fPNpE6WksSjsZuLxUspL3pxKKeqbsnSQr";
const ANCHOR_W8: &str = "Hf89Tqt9FdVdAEsgt3UkmzriXRLPFYqeYE4hHJaSzTjN";
const ANCHOR_W9: &str = "AhdfeAdeXFQNoqfg6XMHU59bi5cty5CZS7b92A1ERZK9";
const ANCHOR_W10: &str = "2N5cmNMVnqrQDWaKE2oP92bVDwMvGNW69k7mpQfyyiMh";
const PINO_W0: &str = "4PE1tkJYXdvEXNFmqLfmu8kfLTUNVCQvMv6dGruZemfR";
const PINO_W1: &str = "2jc9CyUhCbKjqL7WTwWc3pysWzgXPN4ucbf6PUGnparY";
const PINO_W2: &str = "64QzbP8eZ47r61Hvjj9JL1yJW7uj7QLvbo8txCKh7pEK";
const PINO_W3: &str = "6QPHxpcsV7nxHnpVUJhSiS2B32RhyS65LX1a2t1pbZLY";
const PINO_W4: &str = "EZxAdAKQbnD6HZqchzuFdD3UZYVUeF5u7ffYj2pHPbc8";
const PINO_W6: &str = "EKvrHm487HWbrgiWzmHKJRZq34n1V56tZnrwNzmPuaUg";
const PINO_W7: &str = "DbzYtKH9eZ2ejQo2RyBxdY2PGWVhLjnhdw9ja5pFFRno";
const PINO_W8: &str = "DRJ9FZj2xNjSnydfSMiagn49JcXDmDfqV8miH4hhfZds";
const PINO_W9: &str = "AUfBb1dJr392vYKgKMqEYJoWTTjeE6GsWctcrir6mg3";
const PINO_W10: &str = "BHTGrn49Rw47mahPPhupKShja328C13ibSve4b2gAF9E";
const TOKEN_2022_PROGRAM: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

const MINT_LEN: usize = 82;
const TOKEN_ACCOUNT_LEN: usize = 165;

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

fn anchor_ix_disc(name: &str) -> [u8; 8] {
    let h = Sha256::digest(format!("global:{name}").as_bytes());
    h[..8].try_into().unwrap()
}

fn anchor_acc_disc(name: &str) -> [u8; 8] {
    let h = Sha256::digest(format!("account:{name}").as_bytes());
    h[..8].try_into().unwrap()
}

// Manual SPL-token Mint layout (82 bytes):
//   0..4   COption tag for mint_authority (1 = Some)
//   4..36  mint_authority pubkey
//   36..44 supply (u64 LE)
//   44     decimals
//   45     is_initialized (1)
//   46..50 COption tag for freeze_authority (0 = None)
//   50..82 freeze_authority pubkey (zeroed)
fn make_mint(authority: &Pubkey, supply: u64, decimals: u8) -> Vec<u8> {
    let mut buf = vec![0u8; MINT_LEN];
    buf[0..4].copy_from_slice(&1u32.to_le_bytes());
    buf[4..36].copy_from_slice(authority.as_ref());
    buf[36..44].copy_from_slice(&supply.to_le_bytes());
    buf[44] = decimals;
    buf[45] = 1;
    buf
}

// Manual SPL-token Account layout (165 bytes):
//   0..32    mint
//   32..64   owner
//   64..72   amount (u64 LE)
//   72..76   COption tag for delegate (0 = None)
//   76..108  delegate pubkey
//   108      state (1 = Initialized)
//   109..113 COption tag for is_native (0 = None)
//   113..121 is_native value
//   121..129 delegated_amount
//   129..133 COption tag for close_authority (0 = None)
//   133..165 close_authority
fn make_token_account(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut buf = vec![0u8; TOKEN_ACCOUNT_LEN];
    buf[0..32].copy_from_slice(mint.as_ref());
    buf[32..64].copy_from_slice(owner.as_ref());
    buf[64..72].copy_from_slice(&amount.to_le_bytes());
    buf[108] = 1;
    buf
}

fn load_programs(svm: &mut LiteSVM) {
    let progs = [
        (ANCHOR_W0, "target/deploy/anchor_w0_noop.so"),
        (ANCHOR_W1, "target/deploy/anchor_w1_write.so"),
        (ANCHOR_W2, "target/deploy/anchor_w2_spl_cpi.so"),
        (ANCHOR_W3, "target/deploy/anchor_w3_orderbook.so"),
        (ANCHOR_W4, "target/deploy/anchor_w4_matching.so"),
        (ANCHOR_W6, "target/deploy/anchor_w6_multihop.so"),
        (ANCHOR_W7, "target/deploy/anchor_w7_hook.so"),
        (ANCHOR_W8, "target/deploy/anchor_w8_amm.so"),
        (ANCHOR_W9, "target/deploy/anchor_w9_refresh.so"),
        (ANCHOR_W10, "target/deploy/anchor_w10_vault.so"),
        (PINO_W0, "target/deploy/pinocchio_w0_noop.so"),
        (PINO_W1, "target/deploy/pinocchio_w1_write.so"),
        (PINO_W2, "target/deploy/pinocchio_w2_spl_cpi.so"),
        (PINO_W3, "target/deploy/pinocchio_w3_orderbook.so"),
        (PINO_W4, "target/deploy/pinocchio_w4_matching.so"),
        (PINO_W6, "target/deploy/pinocchio_w6_multihop.so"),
        (PINO_W7, "target/deploy/pinocchio_w7_hook.so"),
        (PINO_W8, "target/deploy/pinocchio_w8_amm.so"),
        (PINO_W9, "target/deploy/pinocchio_w9_refresh.so"),
        (PINO_W10, "target/deploy/pinocchio_w10_vault.so"),
    ];
    for (id, path) in progs {
        svm.add_program_from_file(pk(id), path)
            .unwrap_or_else(|e| panic!("failed loading {id} from {path}: {e:?}"));
    }
}

fn run_tx(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) -> Result<u64, String> {
    let blockhash = svm.latest_blockhash();
    let payer = signers[0];
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), signers, blockhash);
    match svm.send_transaction(tx) {
        Ok(meta) => Ok(meta.compute_units_consumed),
        Err(e) => Err(format!("{e:?}")),
    }
}

// ---------- W0: no-op ----------

fn w0_anchor(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    let ix = Instruction {
        program_id: pk(ANCHOR_W0),
        accounts: vec![],
        data: anchor_ix_disc("noop").to_vec(),
    };
    run_tx(svm, ix, &[payer])
}

fn w0_pino(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    let ix = Instruction {
        program_id: pk(PINO_W0),
        accounts: vec![],
        data: vec![],
    };
    run_tx(svm, ix, &[payer])
}

// ---------- W1: signer + state write ----------

fn make_anchor_state(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let kp = Keypair::new();
    let mut data = vec![0u8; 16];
    data[..8].copy_from_slice(&anchor_acc_disc("State"));
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 1_500_000,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn make_pino_state(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let kp = Keypair::new();
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 1_000_000,
            data: vec![0u8; 8],
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn w1_anchor(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    let state = make_anchor_state(svm, pk(ANCHOR_W1));
    let mut data = anchor_ix_disc("write").to_vec();
    data.extend_from_slice(&42u64.to_le_bytes());
    let ix = Instruction {
        program_id: pk(ANCHOR_W1),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(state, false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

fn w1_pino(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    let state = make_pino_state(svm, pk(PINO_W1));
    let ix = Instruction {
        program_id: pk(PINO_W1),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(state, false),
        ],
        data: 42u64.to_le_bytes().to_vec(),
    };
    run_tx(svm, ix, &[payer])
}

// ---------- W2: SPL Token transfer CPI ----------

fn setup_tokens(svm: &mut LiteSVM, authority: Pubkey) -> (Pubkey, Pubkey) {
    let mint = Keypair::new();
    let src = Keypair::new();
    let dst = Keypair::new();
    let token_pid = pk(TOKEN_PROGRAM);

    svm.set_account(
        mint.pubkey(),
        SolAccount {
            lamports: 1_500_000,
            data: make_mint(&authority, 1_000_000, 6),
            owner: token_pid,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    svm.set_account(
        src.pubkey(),
        SolAccount {
            lamports: 2_039_280,
            data: make_token_account(&mint.pubkey(), &authority, 10_000),
            owner: token_pid,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    svm.set_account(
        dst.pubkey(),
        SolAccount {
            lamports: 2_039_280,
            data: make_token_account(&mint.pubkey(), &authority, 0),
            owner: token_pid,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    (src.pubkey(), dst.pubkey())
}

fn w2_anchor(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    let (src, dst) = setup_tokens(svm, payer.pubkey());
    let mut data = anchor_ix_disc("do_transfer").to_vec();
    data.extend_from_slice(&100u64.to_le_bytes());
    let ix = Instruction {
        program_id: pk(ANCHOR_W2),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(src, false),
            AccountMeta::new(dst, false),
            AccountMeta::new_readonly(pk(TOKEN_PROGRAM), false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

fn w2_pino(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    let (src, dst) = setup_tokens(svm, payer.pubkey());
    let ix = Instruction {
        program_id: pk(PINO_W2),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(src, false),
            AccountMeta::new(dst, false),
            AccountMeta::new_readonly(pk(TOKEN_PROGRAM), false),
        ],
        data: 100u64.to_le_bytes().to_vec(),
    };
    run_tx(svm, ix, &[payer])
}

// ---------- W3: orderbook tick insert ----------

const TICK_CAPACITY: usize = 64;
const ORDERBOOK_BODY_LEN: usize = 8 + 16 * TICK_CAPACITY; // count: u64 + ticks: [(u64, u64); 64]
const ANCHOR_BOOK_LEN: usize = 8 + ORDERBOOK_BODY_LEN;

// Build the raw 1032-byte orderbook body with the first `prefill` ticks populated.
// Tick layout: 16 bytes (price u64 LE | qty u64 LE).
fn make_book_body(prefill: usize) -> Vec<u8> {
    let mut body = vec![0u8; ORDERBOOK_BODY_LEN];
    body[..8].copy_from_slice(&(prefill as u64).to_le_bytes());
    for i in 0..prefill {
        let price = 200u64 * (i as u64 + 1); // 200, 400, ..., 6400
        let qty = 100u64;
        let off = 8 + i * 16;
        body[off..off + 8].copy_from_slice(&price.to_le_bytes());
        body[off + 8..off + 16].copy_from_slice(&qty.to_le_bytes());
    }
    body
}

fn make_anchor_book(svm: &mut LiteSVM, owner: Pubkey, prefill: usize) -> Pubkey {
    let kp = Keypair::new();
    let mut data = vec![0u8; ANCHOR_BOOK_LEN];
    data[..8].copy_from_slice(&anchor_acc_disc("OrderBook"));
    data[8..].copy_from_slice(&make_book_body(prefill));
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 10_000_000,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn make_pino_book(svm: &mut LiteSVM, owner: Pubkey, prefill: usize) -> Pubkey {
    let kp = Keypair::new();
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 10_000_000,
            data: make_book_body(prefill),
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn w3_anchor(svm: &mut LiteSVM, payer: &Keypair, prefill: usize, price: u64) -> Result<u64, String> {
    let book = make_anchor_book(svm, pk(ANCHOR_W3), prefill);
    let mut data = anchor_ix_disc("insert").to_vec();
    data.extend_from_slice(&price.to_le_bytes());
    data.extend_from_slice(&10u64.to_le_bytes());
    let ix = Instruction {
        program_id: pk(ANCHOR_W3),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(book, false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

fn w3_pino(svm: &mut LiteSVM, payer: &Keypair, prefill: usize, price: u64) -> Result<u64, String> {
    let book = make_pino_book(svm, pk(PINO_W3), prefill);
    let mut data = price.to_le_bytes().to_vec();
    data.extend_from_slice(&10u64.to_le_bytes());
    let ix = Instruction {
        program_id: pk(PINO_W3),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(book, false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

// ---------- W4 / W5: matching engine place_order ----------
//
// Layout (matches programs/{anchor,pinocchio}-w4-matching/src/lib.rs):
//   Market = { sequence: u64, side: u8, _pad: [u8; 7] }       — 16 bytes
//   Order  = { owner_pk: [u8; 32], qty: u64, sequence: u64 }  — 48 bytes
//   Tick   = { price: u64, n_orders: u32, _pad: u32,
//              orders: [Order; 4] }                            — 208 bytes
//   Book   = { count: u32, _pad: u32, ticks: [Tick; 32] }     — 6664 bytes
//
// Both sides receive accounts in the same order:
//   [signer (s, ro), market (mut), book (mut)]

const W4_TICK_DEPTH: usize = 4;
const W4_N_TICKS: usize = 32;
const W4_ORDER_SIZE: usize = 48;
const W4_TICK_SIZE: usize = 8 + 4 + 4 + W4_TICK_DEPTH * W4_ORDER_SIZE; // 208
const W4_BOOK_BODY: usize = 4 + 4 + W4_N_TICKS * W4_TICK_SIZE;         // 6664
const W4_MARKET_BODY: usize = 16;
const W4_ANCHOR_MARKET: usize = 8 + W4_MARKET_BODY;
const W4_ANCHOR_BOOK: usize = 8 + W4_BOOK_BODY;

fn make_w4_book_body(prefill_ticks: usize, depth: usize) -> Vec<u8> {
    let mut body = vec![0u8; W4_BOOK_BODY];
    body[0..4].copy_from_slice(&(prefill_ticks as u32).to_le_bytes());
    let placeholder_owner = [0xAAu8; 32];
    for t in 0..prefill_ticks {
        let price = 100u64 * (t as u64 + 1); // 100, 200, ..., 1600
        let tick_off = 8 + t * W4_TICK_SIZE;
        body[tick_off..tick_off + 8].copy_from_slice(&price.to_le_bytes());
        body[tick_off + 8..tick_off + 12].copy_from_slice(&(depth as u32).to_le_bytes());
        for d in 0..depth {
            let order_off = tick_off + 16 + d * W4_ORDER_SIZE;
            body[order_off..order_off + 32].copy_from_slice(&placeholder_owner);
            body[order_off + 32..order_off + 40].copy_from_slice(&100u64.to_le_bytes());
            body[order_off + 40..order_off + 48].copy_from_slice(&1u64.to_le_bytes());
        }
    }
    body
}

fn make_w4_anchor_market(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let kp = Keypair::new();
    let mut data = vec![0u8; W4_ANCHOR_MARKET];
    data[..8].copy_from_slice(&anchor_acc_disc("Market"));
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 1_000_000,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn make_w4_pino_market(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let kp = Keypair::new();
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 1_000_000,
            data: vec![0u8; W4_MARKET_BODY],
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn make_w4_anchor_book(svm: &mut LiteSVM, owner: Pubkey, prefill: usize, depth: usize) -> Pubkey {
    let kp = Keypair::new();
    let mut data = vec![0u8; W4_ANCHOR_BOOK];
    data[..8].copy_from_slice(&anchor_acc_disc("Book"));
    data[8..].copy_from_slice(&make_w4_book_body(prefill, depth));
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 50_000_000,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn make_w4_pino_book(svm: &mut LiteSVM, owner: Pubkey, prefill: usize, depth: usize) -> Pubkey {
    let kp = Keypair::new();
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 50_000_000,
            data: make_w4_book_body(prefill, depth),
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn w4_anchor(
    svm: &mut LiteSVM,
    payer: &Keypair,
    prefill: usize,
    depth: usize,
    price: u64,
) -> Result<u64, String> {
    let market = make_w4_anchor_market(svm, pk(ANCHOR_W4));
    let book = make_w4_anchor_book(svm, pk(ANCHOR_W4), prefill, depth);
    let mut data = anchor_ix_disc("place_order").to_vec();
    data.extend_from_slice(&price.to_le_bytes());
    data.extend_from_slice(&10u64.to_le_bytes());
    let ix = Instruction {
        program_id: pk(ANCHOR_W4),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(market, false),
            AccountMeta::new(book, false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

fn w4_pino(
    svm: &mut LiteSVM,
    payer: &Keypair,
    prefill: usize,
    depth: usize,
    price: u64,
) -> Result<u64, String> {
    let market = make_w4_pino_market(svm, pk(PINO_W4));
    let book = make_w4_pino_book(svm, pk(PINO_W4), prefill, depth);
    let mut data = price.to_le_bytes().to_vec();
    data.extend_from_slice(&10u64.to_le_bytes());
    let ix = Instruction {
        program_id: pk(PINO_W4),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(market, false),
            AccountMeta::new(book, false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

// ---------- W6: 3-hop SPL Token CPI chain ----------
//
// One mint, three (source, dest) token-account pairs, all authority = payer.
// Anchor side wraps each transfer in CpiContext::new + Account<TokenAccount>
// validation on src/dst; Pinocchio side just hands the AccountView slices to
// pinocchio_token::Transfer::new.

fn setup_3hop_tokens(svm: &mut LiteSVM, authority: Pubkey) -> [Pubkey; 6] {
    let mint = Keypair::new();
    let token_pid = pk(TOKEN_PROGRAM);

    svm.set_account(
        mint.pubkey(),
        SolAccount {
            lamports: 1_500_000,
            data: make_mint(&authority, 10_000_000, 6),
            owner: token_pid,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let make_acc = |svm: &mut LiteSVM, amount: u64| -> Pubkey {
        let kp = Keypair::new();
        svm.set_account(
            kp.pubkey(),
            SolAccount {
                lamports: 2_039_280,
                data: make_token_account(&mint.pubkey(), &authority, amount),
                owner: token_pid,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();
        kp.pubkey()
    };

    let src1 = make_acc(svm, 10_000);
    let dst1 = make_acc(svm, 0);
    let src2 = make_acc(svm, 10_000);
    let dst2 = make_acc(svm, 0);
    let src3 = make_acc(svm, 10_000);
    let dst3 = make_acc(svm, 0);

    [src1, dst1, src2, dst2, src3, dst3]
}

fn w6_anchor(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    let [src1, dst1, src2, dst2, src3, dst3] = setup_3hop_tokens(svm, payer.pubkey());
    let mut data = anchor_ix_disc("three_hop_transfer").to_vec();
    data.extend_from_slice(&100u64.to_le_bytes());
    let ix = Instruction {
        program_id: pk(ANCHOR_W6),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(src1, false),
            AccountMeta::new(dst1, false),
            AccountMeta::new(src2, false),
            AccountMeta::new(dst2, false),
            AccountMeta::new(src3, false),
            AccountMeta::new(dst3, false),
            AccountMeta::new_readonly(pk(TOKEN_PROGRAM), false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

fn w6_pino(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    let [src1, dst1, src2, dst2, src3, dst3] = setup_3hop_tokens(svm, payer.pubkey());
    let ix = Instruction {
        program_id: pk(PINO_W6),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(src1, false),
            AccountMeta::new(dst1, false),
            AccountMeta::new(src2, false),
            AccountMeta::new(dst2, false),
            AccountMeta::new(src3, false),
            AccountMeta::new(dst3, false),
            AccountMeta::new_readonly(pk(TOKEN_PROGRAM), false),
        ],
        data: 100u64.to_le_bytes().to_vec(),
    };
    run_tx(svm, ix, &[payer])
}

// ---------- W7: Token-2022 transfer with no-op transfer hook ----------
//
// A Token-2022 mint configured with the TransferHook extension causes Token-2022's
// TransferChecked to perform an internal CPI into the hook program (with the
// SPL-transfer-hook `execute` discriminator). The hook program does nothing here,
// so we isolate the per-hook framework cost.
//
// Pre-creates:
//   - Token-2022 mint with TransferHook extension pointing at the hook program
//   - Source + destination Token-2022 accounts with TransferHookAccount extension
//   - Extra account metas PDA at ["extra-account-metas", mint] under the hook program,
//     containing a TLV entry for ExecuteInstruction with 0 extra metas

const EXECUTE_DISCRIMINATOR: [u8; 8] = [105, 37, 101, 197, 75, 251, 102, 26];

const TOKEN22_ACCOUNT_TYPE_OFFSET: usize = 165;
const TOKEN22_TLV_START: usize = 166;

const EXT_TYPE_TRANSFER_HOOK: u16 = 14;
const EXT_TYPE_TRANSFER_HOOK_ACCOUNT: u16 = 15;
const ACCOUNT_TYPE_MINT: u8 = 1;
const ACCOUNT_TYPE_TOKEN: u8 = 2;
const TOKEN_2022_TRANSFER_CHECKED_TAG: u8 = 12;

fn derive_extra_metas_pda(mint: &Pubkey, hook_program: &Pubkey) -> Pubkey {
    let (pda, _) = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.as_ref()],
        hook_program,
    );
    pda
}

fn make_extra_metas_pda_data() -> Vec<u8> {
    // TLV: 8-byte type + 4-byte length + value
    // Value for `ExecuteInstruction` with 0 extras = PodSlice<ExtraAccountMeta>::pack(&[])
    // which is just a 4-byte u32 count of zero.
    let mut data = Vec::with_capacity(16);
    data.extend_from_slice(&EXECUTE_DISCRIMINATOR);
    data.extend_from_slice(&4u32.to_le_bytes()); // length of value
    data.extend_from_slice(&0u32.to_le_bytes()); // count = 0 entries
    data
}

fn make_token22_mint_with_hook(authority: &Pubkey, hook_program: &Pubkey) -> Vec<u8> {
    // 0..165: base Mint (82 useful + 83 padding to TokenAccount-size)
    // 165:    account_type marker = 1 (Mint)
    // 166..:  TLV extensions
    //   166..168 ExtensionType = 14 (TransferHook), u16 LE
    //   168..170 Length = 64, u16 LE
    //   170..202 authority pubkey (zero = None)
    //   202..234 program_id pubkey (the hook program)
    let mut buf = vec![0u8; 234];
    // Base Mint
    buf[0..4].copy_from_slice(&1u32.to_le_bytes()); // COption tag = Some (mint_authority)
    buf[4..36].copy_from_slice(authority.as_ref());
    buf[36..44].copy_from_slice(&1_000_000u64.to_le_bytes()); // supply
    buf[44] = 6;                                              // decimals
    buf[45] = 1;                                              // is_initialized
    // freeze_authority COption tag = None (already zero)
    buf[TOKEN22_ACCOUNT_TYPE_OFFSET] = ACCOUNT_TYPE_MINT;
    buf[TOKEN22_TLV_START..TOKEN22_TLV_START + 2].copy_from_slice(&EXT_TYPE_TRANSFER_HOOK.to_le_bytes());
    buf[TOKEN22_TLV_START + 2..TOKEN22_TLV_START + 4].copy_from_slice(&64u16.to_le_bytes());
    // authority field stays zero (= None)
    buf[TOKEN22_TLV_START + 4 + 32..TOKEN22_TLV_START + 4 + 64].copy_from_slice(hook_program.as_ref());
    buf
}

fn make_token22_account_with_hook(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    // 0..165: base Account (same layout as legacy SPL Token)
    // 165:    account_type marker = 2 (Account)
    // 166..:  TLV extensions
    //   166..168 ExtensionType = 15 (TransferHookAccount)
    //   168..170 Length = 1
    //   170      transferring = 0
    let mut buf = vec![0u8; 171];
    buf[0..32].copy_from_slice(mint.as_ref());
    buf[32..64].copy_from_slice(owner.as_ref());
    buf[64..72].copy_from_slice(&amount.to_le_bytes());
    buf[108] = 1; // state = Initialized
    buf[TOKEN22_ACCOUNT_TYPE_OFFSET] = ACCOUNT_TYPE_TOKEN;
    buf[TOKEN22_TLV_START..TOKEN22_TLV_START + 2]
        .copy_from_slice(&EXT_TYPE_TRANSFER_HOOK_ACCOUNT.to_le_bytes());
    buf[TOKEN22_TLV_START + 2..TOKEN22_TLV_START + 4].copy_from_slice(&1u16.to_le_bytes());
    buf
}

fn w7_run(svm: &mut LiteSVM, payer: &Keypair, hook_pid: Pubkey) -> Result<u64, String> {
    let token22 = pk(TOKEN_2022_PROGRAM);
    let mint = Keypair::new();
    let src = Keypair::new();
    let dst = Keypair::new();
    let extra_metas_pda = derive_extra_metas_pda(&mint.pubkey(), &hook_pid);

    svm.set_account(
        mint.pubkey(),
        SolAccount {
            lamports: 5_000_000,
            data: make_token22_mint_with_hook(&payer.pubkey(), &hook_pid),
            owner: token22,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    svm.set_account(
        src.pubkey(),
        SolAccount {
            lamports: 2_500_000,
            data: make_token22_account_with_hook(&mint.pubkey(), &payer.pubkey(), 10_000),
            owner: token22,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    svm.set_account(
        dst.pubkey(),
        SolAccount {
            lamports: 2_500_000,
            data: make_token22_account_with_hook(&mint.pubkey(), &payer.pubkey(), 0),
            owner: token22,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    svm.set_account(
        extra_metas_pda,
        SolAccount {
            lamports: 1_500_000,
            data: make_extra_metas_pda_data(),
            owner: hook_pid,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // Token-2022 TransferChecked instruction:
    //   tag(1) = 12, amount(8), decimals(1) = 10 bytes
    let mut data = vec![TOKEN_2022_TRANSFER_CHECKED_TAG];
    data.extend_from_slice(&100u64.to_le_bytes());
    data.push(6);

    let ix = Instruction {
        program_id: token22,
        accounts: vec![
            AccountMeta::new(src.pubkey(), false),
            AccountMeta::new_readonly(mint.pubkey(), false),
            AccountMeta::new(dst.pubkey(), false),
            AccountMeta::new_readonly(payer.pubkey(), true),
            // Hook accounts appended (Token-2022 expects them in this order):
            AccountMeta::new_readonly(hook_pid, false),
            AccountMeta::new_readonly(extra_metas_pda, false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

fn w7_anchor(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    w7_run(svm, payer, pk(ANCHOR_W7))
}

fn w7_pino(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    w7_run(svm, payer, pk(PINO_W7))
}

// ---------- W8: AMM constant-product swap ----------
//
// Single-hop Raydium/Meteora-shape swap:
//   [signer (s, ro), pool (mut zero-copy), user_src (mut), user_dst (mut),
//    pool_vault_in (mut), pool_vault_out (mut), token_program (ro)]
//
// Pool state body layout (matches both programs):
//   reserve_in:  u64  (offset 0)
//   reserve_out: u64  (offset 8)
//   fee_bps:     u16  (offset 16)
//   _pad:        [u8; 6]
// = 24 bytes body. Anchor adds 8-byte discriminator.
//
// Bench simplification: authority = payer for all four token accounts.
// Real AMMs use a PDA authority + invoke_signed for vault outflows; that is
// a separate axis (PDA derivation cost) and can be measured in a future W8b.

const W8_POOL_BODY: usize = 24;
const W8_ANCHOR_POOL: usize = 8 + W8_POOL_BODY;

fn make_w8_pool_body(reserve_in: u64, reserve_out: u64, fee_bps: u16) -> Vec<u8> {
    let mut body = vec![0u8; W8_POOL_BODY];
    body[0..8].copy_from_slice(&reserve_in.to_le_bytes());
    body[8..16].copy_from_slice(&reserve_out.to_le_bytes());
    body[16..18].copy_from_slice(&fee_bps.to_le_bytes());
    body
}

fn make_w8_anchor_pool(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let kp = Keypair::new();
    let mut data = vec![0u8; W8_ANCHOR_POOL];
    data[..8].copy_from_slice(&anchor_acc_disc("Pool"));
    data[8..].copy_from_slice(&make_w8_pool_body(1_000_000, 2_000_000, 30));
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 1_500_000,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn make_w8_pino_pool(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let kp = Keypair::new();
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 1_000_000,
            data: make_w8_pool_body(1_000_000, 2_000_000, 30),
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

// Creates two mints (A, B) and four SPL token accounts:
//   user_src (mint_a, amount=10_000),  user_dst (mint_b, amount=0),
//   pool_vault_in (mint_a, amount=0),  pool_vault_out (mint_b, amount=10_000)
// All four owned by `authority`.
fn setup_w8_tokens(svm: &mut LiteSVM, authority: Pubkey) -> [Pubkey; 4] {
    let mint_a = Keypair::new();
    let mint_b = Keypair::new();
    let token_pid = pk(TOKEN_PROGRAM);

    for (mint, supply) in [(&mint_a, 10_000_000u64), (&mint_b, 10_000_000u64)] {
        svm.set_account(
            mint.pubkey(),
            SolAccount {
                lamports: 1_500_000,
                data: make_mint(&authority, supply, 6),
                owner: token_pid,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();
    }

    let make_acc = |svm: &mut LiteSVM, mint: &Pubkey, amount: u64| -> Pubkey {
        let kp = Keypair::new();
        svm.set_account(
            kp.pubkey(),
            SolAccount {
                lamports: 2_039_280,
                data: make_token_account(mint, &authority, amount),
                owner: token_pid,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();
        kp.pubkey()
    };

    let user_src = make_acc(svm, &mint_a.pubkey(), 10_000);
    let user_dst = make_acc(svm, &mint_b.pubkey(), 0);
    let pool_vault_in = make_acc(svm, &mint_a.pubkey(), 0);
    let pool_vault_out = make_acc(svm, &mint_b.pubkey(), 10_000);

    [user_src, user_dst, pool_vault_in, pool_vault_out]
}

fn w8_anchor(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    let pool = make_w8_anchor_pool(svm, pk(ANCHOR_W8));
    let [user_src, user_dst, pool_vault_in, pool_vault_out] =
        setup_w8_tokens(svm, payer.pubkey());
    let mut data = anchor_ix_disc("swap").to_vec();
    data.extend_from_slice(&1_000u64.to_le_bytes()); // amount_in
    data.extend_from_slice(&0u64.to_le_bytes());     // min_out (no slippage check)
    let ix = Instruction {
        program_id: pk(ANCHOR_W8),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(pool, false),
            AccountMeta::new(user_src, false),
            AccountMeta::new(user_dst, false),
            AccountMeta::new(pool_vault_in, false),
            AccountMeta::new(pool_vault_out, false),
            AccountMeta::new_readonly(pk(TOKEN_PROGRAM), false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

fn w8_pino(svm: &mut LiteSVM, payer: &Keypair) -> Result<u64, String> {
    let pool = make_w8_pino_pool(svm, pk(PINO_W8));
    let [user_src, user_dst, pool_vault_in, pool_vault_out] =
        setup_w8_tokens(svm, payer.pubkey());
    let mut data = 1_000u64.to_le_bytes().to_vec(); // amount_in
    data.extend_from_slice(&0u64.to_le_bytes());    // min_out
    let ix = Instruction {
        program_id: pk(PINO_W8),
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(pool, false),
            AccountMeta::new(user_src, false),
            AccountMeta::new(user_dst, false),
            AccountMeta::new(pool_vault_in, false),
            AccountMeta::new(pool_vault_out, false),
            AccountMeta::new_readonly(pk(TOKEN_PROGRAM), false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

// ---------- W9: lending refresh (Kamino-shape) ----------
//
// 5 mutable zero-copy accounts — obligation + 2 reserves + 2 oracles — touched
// in a single refresh instruction. Extends the per-account scaling law to N=5.
//
// Accounts (both sides, same order):
//   [signer (s, ro), obligation (mut), reserve_a (mut), reserve_b (mut),
//    oracle_a (mut), oracle_b (mut)]
//
// State layouts (matches programs/{anchor,pinocchio}-w9-refresh/src/lib.rs):
//   Obligation = { deposit_amount: u64, borrow_amount: u64,
//                  last_health: u64, last_update_slot: u64 }                   — 32 bytes
//   Reserve    = { total_liquidity: u64, total_borrows: u64,
//                  cumulative_borrow_rate: u64, borrow_rate_bps: u32,
//                  _pad: u32, last_update_slot: u64 }                          — 40 bytes
//   Oracle     = { price: u64, conf: u64, last_update_slot: u64 }              — 24 bytes
//
// Real Kamino refresh_obligation has 3 mut (obligation + 2 reserves) and 2 ro
// (oracles). W9 keeps all 5 as mut to isolate the per-mut-account scaling law
// cleanly. A future W9b can split RO vs mut.

const W9_OBLIGATION_BODY: usize = 32;
const W9_RESERVE_BODY: usize = 40;
const W9_ORACLE_BODY: usize = 24;
const W9_ANCHOR_OBLIGATION: usize = 8 + W9_OBLIGATION_BODY;
const W9_ANCHOR_RESERVE: usize = 8 + W9_RESERVE_BODY;
const W9_ANCHOR_ORACLE: usize = 8 + W9_ORACLE_BODY;

fn make_w9_obligation_body(deposit: u64, borrow: u64) -> Vec<u8> {
    let mut body = vec![0u8; W9_OBLIGATION_BODY];
    body[0..8].copy_from_slice(&deposit.to_le_bytes());
    body[8..16].copy_from_slice(&borrow.to_le_bytes());
    // last_health = 0, last_update_slot = 0
    body
}

fn make_w9_reserve_body(liquidity: u64, borrows: u64, borrow_rate_bps: u32) -> Vec<u8> {
    let mut body = vec![0u8; W9_RESERVE_BODY];
    body[0..8].copy_from_slice(&liquidity.to_le_bytes());
    body[8..16].copy_from_slice(&borrows.to_le_bytes());
    // cumulative_borrow_rate = 0
    body[24..28].copy_from_slice(&borrow_rate_bps.to_le_bytes());
    // _pad = 0, last_update_slot = 0
    body
}

fn make_w9_oracle_body(price: u64, conf: u64) -> Vec<u8> {
    let mut body = vec![0u8; W9_ORACLE_BODY];
    body[0..8].copy_from_slice(&price.to_le_bytes());
    body[8..16].copy_from_slice(&conf.to_le_bytes());
    // last_update_slot = 0
    body
}

fn make_w9_anchor_account(
    svm: &mut LiteSVM,
    owner: Pubkey,
    disc_name: &str,
    body: Vec<u8>,
    total_len: usize,
) -> Pubkey {
    let kp = Keypair::new();
    let mut data = vec![0u8; total_len];
    data[..8].copy_from_slice(&anchor_acc_disc(disc_name));
    data[8..].copy_from_slice(&body);
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 5_000_000,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn make_w9_pino_account(svm: &mut LiteSVM, owner: Pubkey, body: Vec<u8>) -> Pubkey {
    let kp = Keypair::new();
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 5_000_000,
            data: body,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn w9_anchor(svm: &mut LiteSVM, payer: &Keypair, current_slot: u64) -> Result<u64, String> {
    let owner = pk(ANCHOR_W9);
    let obligation = make_w9_anchor_account(
        svm,
        owner,
        "Obligation",
        make_w9_obligation_body(1_000, 500),
        W9_ANCHOR_OBLIGATION,
    );
    let reserve_a = make_w9_anchor_account(
        svm,
        owner,
        "Reserve",
        make_w9_reserve_body(1_000_000, 500_000, 300),
        W9_ANCHOR_RESERVE,
    );
    let reserve_b = make_w9_anchor_account(
        svm,
        owner,
        "Reserve",
        make_w9_reserve_body(2_000_000, 800_000, 250),
        W9_ANCHOR_RESERVE,
    );
    let oracle_a = make_w9_anchor_account(
        svm,
        owner,
        "Oracle",
        make_w9_oracle_body(100, 1),
        W9_ANCHOR_ORACLE,
    );
    let oracle_b = make_w9_anchor_account(
        svm,
        owner,
        "Oracle",
        make_w9_oracle_body(50, 1),
        W9_ANCHOR_ORACLE,
    );
    let mut data = anchor_ix_disc("refresh").to_vec();
    data.extend_from_slice(&current_slot.to_le_bytes());
    let ix = Instruction {
        program_id: owner,
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(obligation, false),
            AccountMeta::new(reserve_a, false),
            AccountMeta::new(reserve_b, false),
            AccountMeta::new(oracle_a, false),
            AccountMeta::new(oracle_b, false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

fn w9_pino(svm: &mut LiteSVM, payer: &Keypair, current_slot: u64) -> Result<u64, String> {
    let owner = pk(PINO_W9);
    let obligation = make_w9_pino_account(svm, owner, make_w9_obligation_body(1_000, 500));
    let reserve_a = make_w9_pino_account(svm, owner, make_w9_reserve_body(1_000_000, 500_000, 300));
    let reserve_b = make_w9_pino_account(svm, owner, make_w9_reserve_body(2_000_000, 800_000, 250));
    let oracle_a = make_w9_pino_account(svm, owner, make_w9_oracle_body(100, 1));
    let oracle_b = make_w9_pino_account(svm, owner, make_w9_oracle_body(50, 1));
    let data = current_slot.to_le_bytes().to_vec();
    let ix = Instruction {
        program_id: owner,
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(obligation, false),
            AccountMeta::new(reserve_a, false),
            AccountMeta::new(reserve_b, false),
            AccountMeta::new(oracle_a, false),
            AccountMeta::new(oracle_b, false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

// ---------- W10: vault deposit + share accounting (ERC4626 / Yearn shape) ----------
//
// Single-deposit NAV computation:
//   [signer (s, ro), vault (mut zero-copy), user_position (mut zero-copy),
//    user_underlying (mut), vault_underlying (mut), token_program (ro)]
//
// State layouts (matches programs/{anchor,pinocchio}-w10-vault/src/lib.rs):
//   Vault         = { total_assets: u64, total_shares: u64 }                  — 16 bytes
//   UserPosition  = { share_amount: u64, deposit_count: u64 }                  — 16 bytes
//
// Composition exercised: 2 mutable zero-copy + 2 SPL token accounts + 1 CPI.
// Math: shares = (deposit × total_shares) / total_assets (with first-deposit
// 1:1 special case). Initial vault seeded with total_assets=1_000_000,
// total_shares=1_000_000 so the steady-state path is measured (not the
// first-deposit short-circuit).

const W10_VAULT_BODY: usize = 16;
const W10_USER_POSITION_BODY: usize = 16;
const W10_ANCHOR_VAULT: usize = 8 + W10_VAULT_BODY;
const W10_ANCHOR_USER_POSITION: usize = 8 + W10_USER_POSITION_BODY;

const W10_INITIAL_TOTAL_ASSETS: u64 = 1_000_000;
const W10_INITIAL_TOTAL_SHARES: u64 = 1_000_000;
const W10_USER_UNDERLYING_BALANCE: u64 = 10_000_000;

fn make_w10_vault_body(total_assets: u64, total_shares: u64) -> Vec<u8> {
    let mut body = vec![0u8; W10_VAULT_BODY];
    body[0..8].copy_from_slice(&total_assets.to_le_bytes());
    body[8..16].copy_from_slice(&total_shares.to_le_bytes());
    body
}

fn make_w10_user_position_body() -> Vec<u8> {
    vec![0u8; W10_USER_POSITION_BODY]
}

fn make_w10_anchor_vault(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let kp = Keypair::new();
    let mut data = vec![0u8; W10_ANCHOR_VAULT];
    data[..8].copy_from_slice(&anchor_acc_disc("Vault"));
    data[8..].copy_from_slice(&make_w10_vault_body(
        W10_INITIAL_TOTAL_ASSETS,
        W10_INITIAL_TOTAL_SHARES,
    ));
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 2_000_000,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn make_w10_pino_vault(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let kp = Keypair::new();
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 1_500_000,
            data: make_w10_vault_body(W10_INITIAL_TOTAL_ASSETS, W10_INITIAL_TOTAL_SHARES),
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn make_w10_anchor_user_position(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let kp = Keypair::new();
    let mut data = vec![0u8; W10_ANCHOR_USER_POSITION];
    data[..8].copy_from_slice(&anchor_acc_disc("UserPosition"));
    data[8..].copy_from_slice(&make_w10_user_position_body());
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 2_000_000,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn make_w10_pino_user_position(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let kp = Keypair::new();
    svm.set_account(
        kp.pubkey(),
        SolAccount {
            lamports: 1_500_000,
            data: make_w10_user_position_body(),
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
    kp.pubkey()
}

fn setup_w10_tokens(svm: &mut LiteSVM, authority: Pubkey) -> (Pubkey, Pubkey) {
    let mint = Keypair::new();
    let token_pid = pk(TOKEN_PROGRAM);

    svm.set_account(
        mint.pubkey(),
        SolAccount {
            lamports: 1_500_000,
            data: make_mint(&authority, 100_000_000, 6),
            owner: token_pid,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let make_acc = |svm: &mut LiteSVM, amount: u64| -> Pubkey {
        let kp = Keypair::new();
        svm.set_account(
            kp.pubkey(),
            SolAccount {
                lamports: 2_039_280,
                data: make_token_account(&mint.pubkey(), &authority, amount),
                owner: token_pid,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();
        kp.pubkey()
    };

    let user_underlying = make_acc(svm, W10_USER_UNDERLYING_BALANCE);
    let vault_underlying = make_acc(svm, W10_INITIAL_TOTAL_ASSETS);

    (user_underlying, vault_underlying)
}

fn w10_anchor(svm: &mut LiteSVM, payer: &Keypair, deposit_amount: u64) -> Result<u64, String> {
    let owner = pk(ANCHOR_W10);
    let vault = make_w10_anchor_vault(svm, owner);
    let user_position = make_w10_anchor_user_position(svm, owner);
    let (user_underlying, vault_underlying) = setup_w10_tokens(svm, payer.pubkey());
    let mut data = anchor_ix_disc("deposit").to_vec();
    data.extend_from_slice(&deposit_amount.to_le_bytes());
    let ix = Instruction {
        program_id: owner,
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new(user_position, false),
            AccountMeta::new(user_underlying, false),
            AccountMeta::new(vault_underlying, false),
            AccountMeta::new_readonly(pk(TOKEN_PROGRAM), false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

fn w10_pino(svm: &mut LiteSVM, payer: &Keypair, deposit_amount: u64) -> Result<u64, String> {
    let owner = pk(PINO_W10);
    let vault = make_w10_pino_vault(svm, owner);
    let user_position = make_w10_pino_user_position(svm, owner);
    let (user_underlying, vault_underlying) = setup_w10_tokens(svm, payer.pubkey());
    let data = deposit_amount.to_le_bytes().to_vec();
    let ix = Instruction {
        program_id: owner,
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new(user_position, false),
            AccountMeta::new(user_underlying, false),
            AccountMeta::new(vault_underlying, false),
            AccountMeta::new_readonly(pk(TOKEN_PROGRAM), false),
        ],
        data,
    };
    run_tx(svm, ix, &[payer])
}

// ---------- driver ----------

fn report(name: &str, a: Result<u64, String>, p: Result<u64, String>) {
    match (a, p) {
        (Ok(av), Ok(pv)) => {
            let diff = av as i64 - pv as i64;
            let pct = if av > 0 {
                100.0 * diff as f64 / av as f64
            } else {
                0.0
            };
            println!(
                "  {name:<20}  anchor={av:>7}  pinocchio={pv:>7}  Δ={diff:>6}  ({pct:>5.1}% saved)"
            );
        }
        (a, p) => {
            println!("  {name:<20}  anchor={a:?}  pinocchio={p:?}");
        }
    }
}

fn main() {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    load_programs(&mut svm);

    println!();
    println!("=== pinocchio-bench  (anchor 0.32.1, pinocchio 0.11.1, solana 3.x) ===");
    println!();

    let a = w0_anchor(&mut svm, &payer);
    let p = w0_pino(&mut svm, &payer);
    report("W0 no-op", a, p);

    let a = w1_anchor(&mut svm, &payer);
    let p = w1_pino(&mut svm, &payer);
    report("W1 signer+write", a, p);

    let a = w2_anchor(&mut svm, &payer);
    let p = w2_pino(&mut svm, &payer);
    report("W2 SPL CPI", a, p);

    // W3a: empty book — single compare, no shift
    let a = w3_anchor(&mut svm, &payer, 0, 500);
    let p = w3_pino(&mut svm, &payer, 0, 500);
    report("W3a orderbook empty", a, p);

    // W3b: half-full (32 ticks at 200,400,...,6400) — insert at price=100 → 5 compares + 32-entry shift
    let a = w3_anchor(&mut svm, &payer, 32, 100);
    let p = w3_pino(&mut svm, &payer, 32, 100);
    report("W3b orderbook +shift", a, p);

    // W4: matching-engine place_order into empty book (2 mut accounts: market + book)
    let a = w4_anchor(&mut svm, &payer, 0, 0, 500);
    let p = w4_pino(&mut svm, &payer, 0, 0, 500);
    report("W4 match empty book", a, p);

    // W5: prefilled with 16 ticks @ depth 2 — insert at existing price=800 → FIFO append
    let a = w4_anchor(&mut svm, &payer, 16, 2, 800);
    let p = w4_pino(&mut svm, &payer, 16, 2, 800);
    report("W5 match FIFO append", a, p);

    // W6: 3-hop SPL Token transfer chain (Jupiter-route-shaped)
    let a = w6_anchor(&mut svm, &payer);
    let p = w6_pino(&mut svm, &payer);
    report("W6 3-hop SPL chain", a, p);

    // W7: Token-2022 TransferChecked with no-op transfer hook
    let a = w7_anchor(&mut svm, &payer);
    let p = w7_pino(&mut svm, &payer);
    report("W7 Token-2022 + hook", a, p);

    // W8: AMM constant-product swap (Raydium/Meteora shape, single hop)
    let a = w8_anchor(&mut svm, &payer);
    let p = w8_pino(&mut svm, &payer);
    report("W8 AMM swap", a, p);

    // W9: lending refresh — 5 mut zero-copy accounts (Kamino-shape)
    let a = w9_anchor(&mut svm, &payer, 1_000);
    let p = w9_pino(&mut svm, &payer, 1_000);
    report("W9 lending refresh", a, p);

    // W10: vault deposit (NAV-weighted share computation)
    let a = w10_anchor(&mut svm, &payer, 1_000);
    let p = w10_pino(&mut svm, &payer, 1_000);
    report("W10 vault deposit", a, p);

    println!();
}
