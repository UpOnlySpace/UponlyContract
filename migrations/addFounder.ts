import * as anchor from '@coral-xyz/anchor';
import { PublicKey } from '@solana/web3.js';

const FOUNDER_ADDRESSES = [
  'GNYVxhkUqFWKN52sHwChi3yX5ouRCnJ7US1La85fXcbt'
];

const main = async () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const wallet = provider.wallet as anchor.Wallet;
  const program = anchor.workspace.UpOnly as anchor.Program;
  const uponlyMint = new PublicKey("AD9fW6TDroxh8NXVZccyhBZBHacmnQYB3xiz4aRj7m4c");

  const [metadata] = PublicKey.findProgramAddressSync(
    [Buffer.from("metadata"), uponlyMint.toBuffer()],
    program.programId
  );

  const [foundersPool] = PublicKey.findProgramAddressSync(
    [Buffer.from('founders_pool')],
    program.programId
  );

  const metadataAccount = await program.account.tokenMetadata.fetch(metadata) as any;
  console.log("Expected deployer from metadata:", metadataAccount.deployer.toBase58());
  console.log("Your wallet:", wallet.publicKey.toBase58());

  const pool = await program.account.foundersPool.fetch(foundersPool) as any;

  for (const address of FOUNDER_ADDRESSES) {
    let pubkey: PublicKey;
    try {
      pubkey = new PublicKey(address);
    } catch (e) {
      console.log(`Skipping invalid address: ${address}`);
      continue;
    }

    const alreadyExists = pool.founders
      .slice(0, pool.founderCount)
      .some((f: PublicKey) => f.toBase58() === pubkey.toBase58());

    if (alreadyExists) {
      console.log(`⚠️ Already in pool: ${address}`);
      continue;
    }

    const tx = await program.methods
      .addFounder(pubkey)
      .accounts({
        metadata,
        foundersPool,
        deployer: wallet.publicKey,
      })
      .rpc();

    console.log(`✅ Added founder: ${address}`);
    console.log(`Tx Signature: ${tx}`);
  }
};

main().catch((err) => {
  console.error('Error:', err);
});
