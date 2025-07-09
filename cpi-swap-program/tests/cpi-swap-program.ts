import * as anchor from "@coral-xyz/anchor";
import {
  PublicKey,
  AddressLookupTableAccount,
  TransactionMessage,
  Keypair,
  VersionedTransaction,
  Signer,
} from "@solana/web3.js";
import { Program } from "@coral-xyz/anchor";
import { CpiSwapProgram } from "../target/types/cpi_swap_program";
import axios from "axios";
import { TOKEN_PROGRAM_ID } from "@coral-xyz/anchor/dist/cjs/utils/token";

describe("cpi-swap-program", () => {
  anchor.setProvider(
    anchor.AnchorProvider.local("https://api.mainnet-beta.solana.com", {
      preflightCommitment: "confirmed",
      commitment: "confirmed",
    })
  );

  const program = anchor.workspace.CpiSwapProgram as Program<CpiSwapProgram>;
  const provider = anchor.getProvider();

  // replace with your own keypair
  const payer: Signer = Keypair.generate();

  const connection = provider.connection;

  // swaping tokens in a PDA, CPI into JUPITER Aggregator v6
  it("Is initialized!", async () => {
    const VAULT_SEED = Buffer.from("vault");

    // derive the vault PDA
    const [vaultPDA, vaultBump] = PublicKey.findProgramAddressSync(
      [VAULT_SEED],
      program.programId
    );

    // replace with your valid input and output mint
    // For example, swaping 1 USDC and USDT
    const inputMint = new PublicKey(
      "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
    );
    const outputMint = new PublicKey(
      "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"
    );

    // amount in raw format (e.g., 1 SOL = 1000000000)
    const amount = "1000000";

    // get the swap instructions from Jupiter API
    const swap = await get_swap(vaultPDA, inputMint, outputMint, amount);

    // instructions
    const computeBudgetIxs = swap.computeBudgetInstructions.map(deserializeIx);
    const setupInstructions = swap.setupInstructions.map(deserializeIx);
    const cleanupInstructions = swap.cleanupInstruction
      ? swap.cleanupInstruction?.map(deserializeIx)
      : [];
    const otherInstructions = swap.otherInstructions
      ? swap.otherInstructions.map(deserializeIx)
      : [];

    // get the accounts needed to cpi into the Jupiter program
    const remainingAccounts = swap.swapInstruction.accounts.map((acc: any) => ({
      pubkey: new PublicKey(acc.pubkey),
      isWritable: acc.isWritable,
      isSigner: false,
    }));

    // decode the instruction data
    const data = Buffer.from(swap.swapInstruction.data, "base64");

    const ix = await program.methods
      .swap(data)
      .accounts({
        inputMint,
        inputMintProgram: TOKEN_PROGRAM_ID,
        outputMint,
        outputMintProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts(remainingAccounts)
      .instruction();

    const latestBlockhash = await connection.getLatestBlockhash();

    const altAddresses = swap.addressLookupTableAddresses || [];

    // resolve Address Lookup Tables (ALT)
    const altLookups = await Promise.all(
      altAddresses.map(async (address: any) => {
        const alt = await connection.getAddressLookupTable(
          new PublicKey(address)
        );
        if (!alt.value) throw new Error(`ALT not found: ${address}`);
        return new AddressLookupTableAccount({
          key: new PublicKey(address),
          state: alt.value.state,
        });
      })
    );

    // Build Transaction Message with ALT
    const messageV0 = new TransactionMessage({
      payerKey: payer.publicKey,
      recentBlockhash: latestBlockhash.blockhash,
      instructions: [
        ...computeBudgetIxs,
        ...setupInstructions,
        ix,
        ...cleanupInstructions,
        ...otherInstructions,
      ],
    }).compileToV0Message(altLookups);

    const tx = new VersionedTransaction(messageV0);
    tx.sign([payer]);

    const sig = await connection.sendTransaction(tx);

    console.log("✅ Signature:", sig);
  });
});

async function get_swap(
  address: PublicKey,
  input_mint: PublicKey,
  output_mint: PublicKey,
  amount: string
) {
  // reference - https://dev.jup.ag/docs/api/swap-api/quote
  const quote_url = `https://lite-api.jup.ag/swap/v1/quote?inputMint=${input_mint.toString()}&outputMint=${output_mint.toString()}&amount=${amount}`;
  const quote = await axios.get(quote_url);

  // reference - https://dev.jup.ag/docs/api/swap-api/swap-instructions
  let config = {
    method: "post",
    maxBodyLength: Infinity,
    url: "https://lite-api.jup.ag/swap/v1/swap-instructions",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    data: JSON.stringify({
      userPublicKey: address.toString(),
      quoteResponse: quote.data,
    }),
  };

  const swap = await axios.request(config);
  return swap.data;
}

function deserializeIx(ix: any) {
  return {
    programId: new PublicKey(ix.programId),
    keys: ix.accounts.map((acc: any) => ({
      pubkey: new PublicKey(acc.pubkey),
      isWritable: acc.isWritable,
      isSigner: acc.isSigner,
    })),
    data: Buffer.from(ix.data, "base64"),
  };
}
