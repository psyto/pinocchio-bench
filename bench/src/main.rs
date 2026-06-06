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
const PINO_W0: &str = "4PE1tkJYXdvEXNFmqLfmu8kfLTUNVCQvMv6dGruZemfR";
const PINO_W1: &str = "2jc9CyUhCbKjqL7WTwWc3pysWzgXPN4ucbf6PUGnparY";
const PINO_W2: &str = "64QzbP8eZ47r61Hvjj9JL1yJW7uj7QLvbo8txCKh7pEK";
const PINO_W3: &str = "6QPHxpcsV7nxHnpVUJhSiS2B32RhyS65LX1a2t1pbZLY";
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
        (PINO_W0, "target/deploy/pinocchio_w0_noop.so"),
        (PINO_W1, "target/deploy/pinocchio_w1_write.so"),
        (PINO_W2, "target/deploy/pinocchio_w2_spl_cpi.so"),
        (PINO_W3, "target/deploy/pinocchio_w3_orderbook.so"),
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

    println!();
}
