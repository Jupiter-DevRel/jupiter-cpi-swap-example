import * as anchor from "@coral-xyz/anchor";
import {
  PublicKey,
  AddressLookupTableAccount,
  TransactionMessage,
  Keypair,
  VersionedTransaction,
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
  const payer = Keypair.generate();

  const connection = provider.connection;

  it("Is initialized!", async () => {
    const VAULT_SEED = Buffer.from("vault");

    const [vaultPDA, vaultBump] = PublicKey.findProgramAddressSync(
      [VAULT_SEED],
      program.programId
    );

    // replace with your valid input and output mint
    const inputMint = new PublicKey("");
    const outputMint = new PublicKey("");

    // amount in raw format (e.g., 1 SOL = 1000000000)
    const amount = "1000000";

    const swap = await get_swap(vaultPDA, inputMint, outputMint, amount);

    const computeBudgetIxs = swap.computeBudgetInstructions.map((ix: any) => {
      return {
        programId: new PublicKey(ix.programId),
        keys: [],
        data: Buffer.from(ix.data, "base64"),
      };
    });

    const setupInstructions = swap.setupInstructions.map((ix: any) => {
      return {
        programId: new PublicKey(ix.programId),
        keys: ix.accounts.map((acc: any) => ({
          pubkey: new PublicKey(acc.pubkey),
          isWritable: acc.isWritable,
          isSigner: acc.isSigner,
        })),
        data: Buffer.from(ix.data, "base64"),
      };
    });

    const cleanupInstructions = swap.cleanupInstruction
      ? swap.cleanupInstruction.map((ix: any) => {
          return {
            programId: new PublicKey(ix.programId),
            keys: ix.accounts.map((acc: any) => ({
              pubkey: new PublicKey(acc.pubkey),
              isWritable: acc.isWritable,
              isSigner: acc.isSigner,
            })),
            data: Buffer.from(ix.data, "base64"),
          };
        })
      : [];

    const otherInstructions = swap.otherInstructions.map((ix: any) => {
      return {
        programId: new PublicKey(ix.programId),
        keys: ix.accounts.map((acc: any) => ({
          pubkey: new PublicKey(acc.pubkey),
          isWritable: acc.isWritable,
          isSigner: acc.isSigner,
        })),
        data: Buffer.from(ix.data, "base64"),
      };
    });

    const remainingAccounts = swap.swapInstruction.accounts.map((acc: any) => ({
      pubkey: new PublicKey(acc.pubkey),
      isWritable: acc.isWritable,
      isSigner: false,
    }));

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
        if (!alt.value) throw new Error("ALT not found: ${address}");
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

    console.log(tx.message.compiledInstructions);

    // const sig = await connection.sendTransaction(tx);
    //
    // console.log("✅ Signature:", sig);
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
  console.log(swap.data);
  return swap.data;
}
