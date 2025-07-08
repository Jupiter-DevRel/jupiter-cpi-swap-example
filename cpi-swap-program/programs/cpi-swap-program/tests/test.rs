mod swap_cpi {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use jup_ag_sdk::{
        types::{QuoteRequest, SwapRequest},
        JupiterClient,
    };
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

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

        let swap_req = SwapRequest::new("", "", quote_res);
        let swap_res = client
            .get_swap_instructions(&swap_req)
            .await
            .expect("failed to get swap instructions");

        let mut instructions = vec![];

        let mut swap_ix_data: Vec<u8> = vec![248, 198, 158, 145, 225, 117, 135, 200]; // discriminator for swap instruction
        let jup_ix_data = STANDARD.decode(swap_res.swap_instruction.data).unwrap();
        swap_ix_data.extend_from_slice(&jup_ix_data);

        let program_id: Pubkey = Pubkey::from_str("G6fbBJXU4CQbo6dVL6X1Mgn3RmbNEnbqEYcGqgxXHhu4")
            .expect("Invalid pubkey");
    }
}
