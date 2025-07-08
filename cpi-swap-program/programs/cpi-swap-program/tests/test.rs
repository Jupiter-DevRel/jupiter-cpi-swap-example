mod swap_cpi {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use dotenv::dotenv;
    use jup_ag_sdk::{
        types::{Instruction as JupInstruction, QuoteRequest, SwapRequest},
        JupiterClient,
    };
    use solana_sdk::{
        address_lookup_table::state::AddressLookupTable,
        commitment_config::CommitmentConfig,
        instruction::{AccountMeta, Instruction},
        message::{
            v0::{self},
            AddressLookupTableAccount, VersionedMessage,
        },
        pubkey::Pubkey,
        signature::{Keypair, Signer},
        transaction::VersionedTransaction,
    };
    use std::str::FromStr;
    use std::{env, error::Error};

    #[test]
    fn add() {
        assert_eq!(2 + 2, 4);
    }

    #[tokio::test]
    async fn test_swap() {
        let client = JupiterClient::new("https://lite-api.jup.ag");

        let quote_req = QuoteRequest::new(
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
            1_000_000,
        );

        let quote_res = client
            .get_quote(&quote_req)
            .await
            .expect("failed to get quote");

        let program_id: Pubkey = Pubkey::from_str("4GRV9oi2S8QYXexvr5rsnWosaQpsuuWFBSay8vMjEuXE")
            .expect("Invalid pubkey");

        let (vault, _) = Pubkey::find_program_address(&[b"vault"], &program_id);

        let swap_req = SwapRequest::new(
            vault.to_string(),
            "37STxhFXU5tGYV9JVsMujUGKAEQMVwZFhSbG9sBq7zQ2",
            quote_res,
        );

        let swap_res = client
            .get_swap_instructions(&swap_req)
            .await
            .expect("failed to get swap instructions");

        let mut instructions = vec![];

        let mut swap_ix_data: Vec<u8> = vec![248, 198, 158, 145, 225, 117, 135, 200]; // discriminator for swap instruction
        let jup_ix_data = STANDARD.decode(swap_res.swap_instruction.data).unwrap();

        swap_ix_data.extend_from_slice(&jup_ix_data);

        let jupiter_program =
            Pubkey::from_str("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4").unwrap();

        let swap_cpi_accounts = swap_res
            .swap_instruction
            .accounts
            .iter()
            .map(|a| {
                let pubkey = Pubkey::from_str(&a.pubkey).expect("Invalid account pubkey");
                AccountMeta {
                    pubkey,
                    is_signer: false,
                    is_writable: a.is_writable,
                }
            })
            .collect::<Vec<AccountMeta>>();

        let mut accounts = vec![
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(jupiter_program, false),
        ];

        accounts.extend(swap_cpi_accounts);

        let swap_ix = Instruction {
            program_id,
            accounts,
            data: swap_ix_data,
        };

        instructions.push(swap_ix);

        if let Some(compute_instructions) = swap_res.compute_budget_instructions {
            for instr in compute_instructions {
                instructions.push(parse_instruction(&instr).unwrap());
            }
        }

        for instr in swap_res.setup_instructions {
            instructions.push(parse_instruction(&instr).unwrap());
        }

        if let Some(cleanup_instr) = swap_res.cleanup_instruction {
            instructions.push(parse_instruction(&cleanup_instr).unwrap());
        }

        if let Some(other_instructions) = swap_res.other_instructions {
            for instr in other_instructions {
                instructions.push(parse_instruction(&instr).unwrap());
            }
        }

        let rpc_url =
            "https://mainnet.helius-rpc.com/?api-key=c991f045-ba1f-4d71-b872-0ef87e7f039d";
        let rpc_client = solana_client::nonblocking::rpc_client::RpcClient::new_with_commitment(
            rpc_url.to_string(),
            CommitmentConfig::confirmed(),
        );

        let mut address_table_lookups = vec![];
        for alt_address in swap_res.address_lookup_table_addresses {
            let alt_pubkey = alt_address.parse::<Pubkey>().unwrap();
            let alt_account = rpc_client.get_account(&alt_pubkey).await.unwrap();
            let alt_state = AddressLookupTable::deserialize(&alt_account.data).unwrap();

            let address_table_account = AddressLookupTableAccount {
                key: alt_pubkey,
                addresses: alt_state.addresses.into_owned(),
            };

            address_table_lookups.push(address_table_account);
        }

        let key_bytes = [];
        let keypair = Keypair::from_bytes(&key_bytes).expect("Failed to create Keypair");

        let recent_blockhash = rpc_client
            .get_latest_blockhash()
            .await
            .expect("Failed to get blockhash");

        let message = v0::Message::try_compile(
            &keypair.pubkey(),
            &instructions,
            &address_table_lookups,
            recent_blockhash,
        )
        .unwrap();

        let versioned_message = VersionedMessage::V0(message);

        let tx = VersionedTransaction::try_new(versioned_message, &[&keypair]).unwrap();

        let signature = rpc_client.send_and_confirm_transaction(&tx).await.unwrap();
        println!("Tx sent with signature: {}", signature);
    }

    fn parse_instruction(
        instr: &JupInstruction,
    ) -> Result<Instruction, Box<dyn std::error::Error>> {
        let program_id = instr
            .program_id
            .parse::<Pubkey>()
            .map_err(|e| format!("Invalid program_id pubkey: {}", e))?;

        let accounts: Vec<AccountMeta> = instr
            .accounts
            .iter()
            .map(|a| {
                let pubkey = a
                    .pubkey
                    .parse::<Pubkey>()
                    .map_err(|e| format!("Invalid account pubkey: {}", e))?;
                Ok(AccountMeta {
                    pubkey,
                    is_signer: a.is_signer,
                    is_writable: a.is_writable,
                })
            })
            .collect::<Result<Vec<_>, String>>()?; // Explicit `Result` type;

        let data = STANDARD
            .decode(&instr.data)
            .map_err(|e| format!("Base64 decoding error in instruction data: {}", e))?;

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}
