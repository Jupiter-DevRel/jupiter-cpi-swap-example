mod helpers;
mod retryable_rpc;

use base64::engine::Engine;
use borsh::{BorshDeserialize, BorshSerialize};
use helpers::{get_address_lookup_table_accounts, get_discriminator};
use jup_swap::{
    quote::QuoteRequest,
    swap::SwapRequest,
    transaction_config::{DynamicSlippageSettings, TransactionConfig},
    JupiterSwapApiClient,
};
use solana_client::{rpc_client::RpcClient, rpc_config::RpcSimulateTransactionConfig};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::{v0::Message, VersionedMessage},
    pubkey,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::VersionedTransaction,
};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account_idempotent,
};
use spl_token::ID as TOKEN_PROGRAM_ID;
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio;
use tokio::sync::RwLock;
use bs58;

const INPUT_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const INPUT_AMOUNT: u64 = 2_000_000;
const OUTPUT_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

const CPI_SWAP_PROGRAM_ID: Pubkey = pubkey!("8KQG1MYXru73rqobftpFjD3hBD8Ab3jaag8wbjZG63sx");
const JUPITER_PROGRAM_ID: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
const DEFAULT_RPC_URL: &str = "https://api.mainnet-beta.solana.com";

struct LatestBlockhash {
    blockhash: RwLock<solana_sdk::hash::Hash>,
    slot: AtomicU64,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct SwapIxData {
    pub data: Vec<u8>,
}

#[tokio::main]
async fn main() {
    let rpc_url = env::var("RPC_URL").unwrap_or(DEFAULT_RPC_URL.to_string());

    // Support for both array format and bs58 private key
    let keypair = if let Ok(keypair_bs58) = env::var("BS58_KEYPAIR") {
        // BS58 format
        let keypair_bytes = bs58::decode(keypair_bs58)
            .into_vec()
            .expect("Failed to decode BS58 keypair");
        Keypair::from_bytes(&keypair_bytes).unwrap()
    } else {
        // Original array format
        let keypair_str = env::var("KEYPAIR").expect("Either KEYPAIR or BS58_KEYPAIR environment variable must be set");
        let keypair_bytes: Vec<u8> = keypair_str
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().parse().expect("Failed to parse u8 value"))
            .collect();
        Keypair::from_bytes(&keypair_bytes).unwrap()
    };

    let keypair_pubkey = keypair.pubkey();
    println!("Using wallet address: {}", keypair_pubkey);

    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        rpc_url.to_string(),
        CommitmentConfig::confirmed(),
    ));

    let rpc_client_clone = rpc_client.clone();
    let latest_blockhash = Arc::new(LatestBlockhash {
        blockhash: RwLock::new(solana_sdk::hash::Hash::default()),
        slot: AtomicU64::new(0),
    });

    let latest_blockhash_clone = latest_blockhash.clone();
    tokio::spawn(async move {
        loop {
            if let Ok((blockhash, slot)) =
                rpc_client_clone.get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
            {
                let mut blockhash_write = latest_blockhash_clone.blockhash.write().await;
                *blockhash_write = blockhash;
                latest_blockhash_clone.slot.store(slot, Ordering::Relaxed);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

    let api_base_url = env::var("API_BASE_URL").unwrap_or("https://quote-api.jup.ag/v6".into());

    let jupiter_swap_api_client = JupiterSwapApiClient::new(api_base_url);

    let quote_request = QuoteRequest {
        amount: INPUT_AMOUNT,
        input_mint: INPUT_MINT,
        output_mint: OUTPUT_MINT,
        ..QuoteRequest::default()
    };

    // GET /quote
    let quote_response = match jupiter_swap_api_client.quote(&quote_request).await {
        Ok(quote_response) => quote_response,
        Err(e) => {
            println!("quote failed: {e:#?}");
            return;
        }
    };

    println!("Quote received: {} USDC → {} SOL", 
        INPUT_AMOUNT as f64 / 1_000_000.0,
        quote_response.out_amount as f64 / 1_000_000_000.0);

    // Use user's wallet directly instead of vault
    let response = jupiter_swap_api_client
        .swap_instructions(&SwapRequest {
            user_public_key: keypair_pubkey,
            payer: keypair_pubkey,
            quote_response,
            config: TransactionConfig {
                skip_user_accounts_rpc_calls: false, 
                wrap_and_unwrap_sol: true, 
                dynamic_compute_unit_limit: true,
                dynamic_slippage: Some(DynamicSlippageSettings {
                    min_bps: Some(50),
                    max_bps: Some(1000),
                }),
                ..TransactionConfig::default()
            },
        })
        .await
        .unwrap();

    let address_lookup_table_accounts =
        get_address_lookup_table_accounts(&rpc_client, response.address_lookup_table_addresses)
            .await
            .unwrap();

    // Use Jupiter's swap instruction directly
    let swap_ix = response.swap_instruction;
    
    let simulate_cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
    let cup_ix = ComputeBudgetInstruction::set_compute_unit_price(200_000);
    loop {
        let slot = latest_blockhash.slot.load(Ordering::Relaxed);
        if slot != 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let recent_blockhash = latest_blockhash.blockhash.read().await;

    // Simulate the direct Jupiter swap (not using the CPI program)
    let simulate_message = Message::try_compile(
        &keypair_pubkey,
        &[
            simulate_cu_ix,
            cup_ix.clone(),
            swap_ix.clone(),
        ],
        &address_lookup_table_accounts,
        *recent_blockhash,
    )
    .unwrap();
    let simulate_tx =
        VersionedTransaction::try_new(VersionedMessage::V0(simulate_message), &[&keypair]).unwrap();
    let simulated_cu = match rpc_client.simulate_transaction_with_config(
        &simulate_tx,
        RpcSimulateTransactionConfig {
            replace_recent_blockhash: true,
            ..RpcSimulateTransactionConfig::default()
        },
    ) {
        Ok(simulate_result) => {
            if simulate_result.value.err.is_some() {
                let e = simulate_result.value.err.unwrap();
                panic!(
                    "Failed to simulate transaction due to {:?} logs:{:?}",
                    e, simulate_result.value.logs
                );
            }
            simulate_result.value.units_consumed.unwrap()
        }
        Err(e) => {
            panic!("simulate failed: {e:#?}");
        }
    };

    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit((simulated_cu + 10_000) as u32);

    let recent_blockhash = latest_blockhash.blockhash.read().await;
    println!("Latest blockhash: {}", recent_blockhash);
    
    // Build final transaction with direct Jupiter swap
    let message = Message::try_compile(
        &keypair_pubkey,
        &[cu_ix, cup_ix, swap_ix],
        &address_lookup_table_accounts,
        *recent_blockhash,
    )
    .unwrap();

    println!(
        "Base64 EncodedTransaction message: {}",
        base64::engine::general_purpose::STANDARD
            .encode(VersionedMessage::V0(message.clone()).serialize())
    );
    let tx = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&keypair]).unwrap();
    let retryable_client = retryable_rpc::RetryableRpcClient::new(&rpc_url);

    let tx_hash = tx.signatures[0];
    println!("Sending transaction...");

    if let Ok(tx_hash) = retryable_client.send_and_confirm_transaction(&tx).await {
        println!(
            "Transaction confirmed: https://explorer.solana.com/tx/{}",
            tx_hash
        );
    } else {
        println!(
            "Transaction failed: https://explorer.solana.com/tx/{}",
            tx_hash
        );
        return;
    };
}
