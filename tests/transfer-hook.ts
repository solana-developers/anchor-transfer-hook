import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { TransferHook } from "../target/types/transfer_hook";
import {
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  Keypair,
} from "@solana/web3.js";
import {
  ExtensionType,
  TOKEN_2022_PROGRAM_ID,
  getMintLen,
  createInitializeMintInstruction,
  createInitializeTransferHookInstruction,
  addExtraAccountsToInstruction,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
  createTransferCheckedInstruction,
  getAssociatedTokenAddressSync,
  getMint,
  getTransferHook,
  getExtraAccountMetaAddress,
  getExtraAccountMetas,
  createApproveInstruction,
} from "@solana/spl-token";

describe("transfer-hook", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.TransferHook as Program<TransferHook>;
  const wallet = provider.wallet as anchor.Wallet;
  const connection = provider.connection;

  const [counterPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("counter")],
    program.programId
  );

  const mint = new Keypair();
  const decimals = 2;

  const sourceTokenAccount = getAssociatedTokenAddressSync(
    mint.publicKey,
    wallet.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  const recipient = Keypair.generate();
  const destinationTokenAccount = getAssociatedTokenAddressSync(
    mint.publicKey,
    recipient.publicKey,
    false,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  it("Create Counter Account", async () => {
    try {
      const txSig = await program.methods
        .initialize()
        .accounts({
          counter: counterPDA,
        })
        .rpc();

      // Fetch the counter account data
      const accountData = await program.account.counter.fetch(counterPDA);

      console.log(`Transaction Signature: ${txSig}`);
      console.log(`Count: ${accountData.count}`);
    } catch (error) {
      // If PDA Account already created, then we expect an error
      console.log(error);
    }
  });

  it("Create Mint Account with Transfer Hook Extension", async () => {
    const extensions = [ExtensionType.TransferHook];
    const mintLen = getMintLen(extensions);
    const lamports =
      await provider.connection.getMinimumBalanceForRentExemption(mintLen);

    const transaction = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: wallet.publicKey,
        newAccountPubkey: mint.publicKey,
        space: mintLen,
        lamports: lamports,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeTransferHookInstruction(
        mint.publicKey,
        wallet.publicKey,
        program.programId,
        TOKEN_2022_PROGRAM_ID
      ),
      createInitializeMintInstruction(
        mint.publicKey,
        decimals,
        wallet.publicKey,
        null,
        TOKEN_2022_PROGRAM_ID
      )
    );

    const txSig = await sendAndConfirmTransaction(
      provider.connection,
      transaction,
      [wallet.payer, mint]
    );
    console.log(`Transaction Signature: ${txSig}`);
  });

  it("Create ExtraAccountMetaList Account", async () => {
    const [extraAccountMetaListPDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("extra-account-metas"), mint.publicKey.toBuffer()],
      program.programId
    );

    const initializeExtraAccountMetaListInstruction = await program.methods
      .initializeExtraAccountMetaList()
      .accounts({
        payer: wallet.publicKey,
        extraAccount: extraAccountMetaListPDA,
        counter: counterPDA,
        mint: mint.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();

    const transaction = new Transaction().add(
      initializeExtraAccountMetaListInstruction
    );

    const txSig = await sendAndConfirmTransaction(
      provider.connection,
      transaction,
      [wallet.payer]
    );
    console.log("Transaction Signature:", txSig);
  });

  it("Create Token Accounts and Mint Tokens", async () => {
    // 100 tokens
    const amount = 100 * 10 ** decimals;

    const transaction = new Transaction().add(
      createAssociatedTokenAccountInstruction(
        wallet.publicKey,
        sourceTokenAccount,
        wallet.publicKey,
        mint.publicKey,
        TOKEN_2022_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID
      ),
      createAssociatedTokenAccountInstruction(
        wallet.publicKey,
        destinationTokenAccount,
        recipient.publicKey,
        mint.publicKey,
        TOKEN_2022_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID
      ),
      createMintToInstruction(
        mint.publicKey,
        sourceTokenAccount,
        wallet.publicKey,
        amount,
        [],
        TOKEN_2022_PROGRAM_ID
      )
    );

    const txSig = await sendAndConfirmTransaction(connection, transaction, [
      wallet.payer,
    ]);

    console.log(`Transaction Signature: ${txSig}`);
  });

  it("Transfer Hook with Extra Account Meta", async () => {
    const [delegatePDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("delegate"), sourceTokenAccount.toBuffer()],
      program.programId
    );

    console.log("Delegate PDA:", delegatePDA.toBase58());

    // 1 token
    const amount = 10 * 10 ** decimals;

    // Approve delegate to transfer tokens
    const approveInstruction = createApproveInstruction(
      sourceTokenAccount,
      delegatePDA,
      wallet.publicKey,
      amount,
      [],
      TOKEN_2022_PROGRAM_ID
    );

    const transferInstruction = createTransferCheckedInstruction(
      sourceTokenAccount,
      mint.publicKey,
      destinationTokenAccount,
      wallet.publicKey,
      amount,
      decimals,
      [],
      TOKEN_2022_PROGRAM_ID
    );

    const instructionWithExtraAccounts = await addExtraAccountsToInstruction(
      connection,
      transferInstruction,
      mint.publicKey,
      "confirmed",
      TOKEN_2022_PROGRAM_ID
    );

    const transaction = new Transaction().add(
      approveInstruction,
      instructionWithExtraAccounts
    );
    const txSig = await sendAndConfirmTransaction(
      connection,
      transaction,
      [wallet.payer],
      { skipPreflight: true }
    );
    console.log("Transfer Signature:", txSig);
  });

  // it("Test", async () => {
  //   // 1 token
  //   const amount = 1 * 10 ** decimals;
  //   const transferInstruction = createTransferCheckedInstruction(
  //     sourceTokenAccount,
  //     mint.publicKey,
  //     destinationTokenAccount,
  //     wallet.publicKey,
  //     amount,
  //     decimals,
  //     [],
  //     TOKEN_2022_PROGRAM_ID
  //   );

  //   const mintInfo = await getMint(
  //     connection,
  //     mint.publicKey,
  //     "confirmed",
  //     TOKEN_2022_PROGRAM_ID
  //   );

  //   const transferHook = getTransferHook(mintInfo);

  //   const extraAccountsAccount = getExtraAccountMetaAddress(
  //     mint.publicKey,
  //     transferHook.programId
  //   );

  //   const extraAccountsInfo = await connection.getAccountInfo(
  //     extraAccountsAccount
  //   );

  //   const extraAccountMetas = getExtraAccountMetas(extraAccountsInfo);
  //   console.log("Extra Account Metas:", extraAccountMetas);

  //   const accountMetas = transferInstruction.keys;

  //   accountMetas.push({
  //     pubkey: transferHook.programId,
  //     isSigner: false,
  //     isWritable: false,
  //   });
  //   accountMetas.push({
  //     pubkey: extraAccountsAccount,
  //     isSigner: false,
  //     isWritable: false,
  //   });
  //   accountMetas.push({
  //     pubkey: counterPDA,
  //     isSigner: false,
  //     isWritable: true,
  //   });

  //   // console.log("Account Metas:", accountMetas);

  //   const instruction = new TransactionInstruction({
  //     keys: accountMetas,
  //     programId: TOKEN_2022_PROGRAM_ID,
  //     data: transferInstruction.data,
  //   });

  //   console.log("Instruction:", instruction);

  //   const transaction = new Transaction().add(instruction);
  //   const txSig = await sendAndConfirmTransaction(
  //     connection,
  //     transaction,
  //     [wallet.payer],
  //     { skipPreflight: true }
  //   );
  //   console.log("Transfer Signature:", txSig);
  // });
});
