
import { createCreateMetadataAccountV3Instruction, DataV2 } from '@metaplex-foundation/mpl-token-metadata';
import { PROGRAM_ID as METADATA_PROGRAM_ID } from '@metaplex-foundation/mpl-token-metadata';
import { findMetadataPda } from '@metaplex-foundation/js';
import * as anchor from '@coral-xyz/anchor';
import { Program } from '@coral-xyz/anchor';
import { PublicKey, Keypair, SystemProgram } from '@solana/web3.js';
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  getAccount,
  getAssociatedTokenAddress,
  getMint,
} from '@solana/spl-token';

async function main() {
  console.log('🚀 Starting deployment...');

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const wallet = provider.wallet as anchor.Wallet;
  const connection = provider.connection;
  const program = anchor.workspace.UpOnly as Program<anchor.Idl>;

  console.log('Wallet address:', wallet.publicKey.toBase58());
  console.log('Program ID:', program.programId.toBase58());

  const usdcMint = new PublicKey("2iu8hAjf4SRMeGF7qR5KuCE6hTd9zwAXLsXFgKYJXjPW");
  
  const upOnlyMint = await createMint(
    connection,
    wallet.payer,
    wallet.publicKey,
    wallet.publicKey,
    9 // UpOnly: 9 decimals
  );
  
  console.log('USDC Mint:', usdcMint.toBase58());
  console.log('UpOnly Mint:', upOnlyMint.toBase58());
  
  const upOnlyMintInfo = await getMint(connection, upOnlyMint);

  console.log('UpOnly Mint Authority:', upOnlyMintInfo.mintAuthority?.toBase58());
  

  
  const usdcTokenAccount = await getOrCreateAssociatedTokenAccount(
    connection,
    wallet.payer,
    usdcMint,
    wallet.publicKey
  );

  const upOnlyTokenAccount = await getOrCreateAssociatedTokenAccount(
    connection,
    wallet.payer,
    upOnlyMint,
    wallet.publicKey
  );
  
  // Mint 1 UpOnly token (1 * 10^9)
  await mintTo(
    connection,
    wallet.payer,
    upOnlyMint,
    upOnlyTokenAccount.address,
    wallet.payer,
    1 * 10 ** 9
  );
  
  
  const [mintAuthorityPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('mint_authority')],
    program.programId
  );

  const [metadataPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('metadata'), upOnlyMint.toBuffer()],
    program.programId
  );

  const metaplexMetadataPda = PublicKey.findProgramAddressSync(
    [
      Buffer.from('metadata'),
      METADATA_PROGRAM_ID.toBuffer(),
      upOnlyMint.toBuffer(),
    ],
    METADATA_PROGRAM_ID
  )[0];
  
  const metadataData: DataV2 = {
    name: 'UpOnly Token',
    symbol: 'UPONLY',
    uri: 'https://scarlet-certain-condor-27.mypinata.cloud/ipfs/bafkreihjcfoyfbjx257iqjwvszf4domyzj3eu3d4gxoiao46dkttvuewiu',
    sellerFeeBasisPoints: 0,
    creators: null,
    collection: null,
    uses: null,
  };
  
  const metadataIx = createCreateMetadataAccountV3Instruction(
    {
      metadata: metaplexMetadataPda,
      mint: upOnlyMint,
      mintAuthority: wallet.publicKey,
      payer: wallet.publicKey,
      updateAuthority: wallet.publicKey,
    },
    {
      createMetadataAccountArgsV3: {
        data: metadataData,
        isMutable: true,
        collectionDetails: null,
      },
    }
  );
  
  const tx = new anchor.web3.Transaction().add(metadataIx);
  await anchor.web3.sendAndConfirmTransaction(connection, tx, [wallet.payer]);

  const [programUpOnlyTokenAccountPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('token_account'), upOnlyMint.toBuffer()],
    program.programId
  );

  const [programUsdcTokenAccountPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('token_account'), usdcMint.toBuffer()],
    program.programId
  );

  const programUpOnlyAccount = await getOrCreateAssociatedTokenAccount(
    connection,
    wallet.payer,
    upOnlyMint,
    programUpOnlyTokenAccountPda,
    true
  );

  const programUsdcAccount = await getOrCreateAssociatedTokenAccount(
    connection,
    wallet.payer,
    usdcMint,
    programUsdcTokenAccountPda,
    true
  );

  console.log('\n=== Liquidity Pool Accounts ===');
  console.log('Program USDC Account (Liquidity Pool):', programUsdcAccount.address.toBase58());
  console.log('Program UpOnly Account (Liquidity Pool):', programUpOnlyAccount.address.toBase58());
  console.log('================================\n');

  console.log('Initializing program...');
  try {
    await program.methods
      .initialize()
      .accounts({
        upOnlyMint,
        metadata: metadataPda,
        userUpOnlyAccount: upOnlyTokenAccount.address,
        programUpOnlyAccount: programUpOnlyAccount.address,
        paymentTokenMint: usdcMint,
        userPaymentTokenAccount: usdcTokenAccount.address,
        programPaymentTokenAccount: programUsdcAccount.address,
        mintAuthority: mintAuthorityPda,
        currentMintAuthority: wallet.publicKey,
        authority: wallet.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([wallet.payer])
      .rpc();
  } catch (error) {
    console.error('Error during initialization:', error);
    throw error;
  }

  console.log('Initializing founders pool...');
  const [foundersPoolPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('founders_pool')],
    program.programId
  );

  const [founderAuthorityPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('founder_authority')],
    program.programId
  );

  const founderPoolTokenAccount = await getAssociatedTokenAddress(
    usdcMint,
    founderAuthorityPda,
    true
  );

  try {
    await program.methods
      .initializeFoundersPool()
      .accounts({
        foundersPool: foundersPoolPda,
        authority: wallet.publicKey,
        founderAuthority: founderAuthorityPda,
        founderPoolTokenAccount: founderPoolTokenAccount,
        usdcMint: usdcMint,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .signers([wallet.payer])
      .rpc();
  } catch (error) {
    console.error('Error during founders pool initialization:', error);
    throw error;
  }

  console.log('✅ Deployment complete!');
  console.log('USDC Mint:', usdcMint.toBase58());
  console.log('UpOnly Mint:', upOnlyMint.toBase58());
  console.log('Metadata PDA:', metadataPda.toBase58());
  console.log('Founders Pool PDA:', foundersPoolPda.toBase58());
}

main().then(
  () => process.exit(0),
  (err) => {
    console.error(err);
    process.exit(1);
  }
); 