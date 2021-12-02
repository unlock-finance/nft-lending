import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { NftLending } from '../target/types/nft_lending';

describe('nft-lending', () => {

  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.NftLending as Program<NftLending>;

  it('Is initialized!', async () => {
    // Add your test here.
    const tx = await program.rpc.initialize({});
    console.log("Your transaction signature", tx);
  });
});
