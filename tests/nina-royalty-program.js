const anchor = require('@project-serum/anchor');
const assert = require("assert");

const {
  TOKEN_PROGRAM_ID,
  sleep,
  getTokenAccount,
  createMint,
  createMintInstructions,
  createTokenAccount,
  mintToAccount,
  findOrCreateAssociatedTokenAccount,
  bnToDecimal,
} = require("./utils");

const provider = anchor.Provider.env()
anchor.setProvider(provider);

const program = anchor.workspace.Royalty;

let usdcMint = null;

let userUsdcTokenAccount = null;
let userOutgoingUsdcAccount = null;
let secondRoyaltyUsdcTokenAccount = null;

const primaryMarketPrice = new anchor.BN(2000000); //$2.00
const secondaryMarketPrice = new anchor.BN(600000); // $0.60

const royaltyPercentageSecondaryMarket = new anchor.BN(200000); //20%

const royalty = new anchor.web3.Account();
let royaltySigner = null;
let royaltyNonce = null;
let royaltyUsdcTokenAccount = null;

describe('Royalty', async () => {
  it('Initializes the stuff that exists before the Royalty Account', async () => {
    usdcMint = await createMint(provider, provider.wallet.publicKey, 6);
    userOutgoingUsdcAccount = await createTokenAccount(provider, usdcMint, provider.wallet.publicKey);

    await mintToAccount(
      provider,
      usdcMint,
      userOutgoingUsdcAccount,
      new anchor.BN(10000000),
      provider.wallet.publicKey,
    );

    [userUsdcTokenAccount, _] = await findOrCreateAssociatedTokenAccount(
      provider,
      provider.wallet.publicKey,
      anchor.web3.SystemProgram.programId,
      anchor.web3.SYSVAR_RENT_PUBKEY,
      usdcMint,
      true,
      true,
    );

    secondRoyaltyUsdcTokenAccount = await createTokenAccount(provider, usdcMint, provider.wallet.publicKey);
  })

  it('Initializes Royalty Account', async () => {

    [royaltySigner, royaltyNonce] = await anchor.web3.PublicKey.findProgramAddress(
      [royalty.publicKey.toBuffer()],
      program.programId
    );

    let [_royaltyUsdcTokenAccount, royaltyUsdcTokenAccountIx] = await findOrCreateAssociatedTokenAccount(
      provider,
      royaltySigner,
      anchor.web3.SystemProgram.programId,
      anchor.web3.SYSVAR_RENT_PUBKEY,
      usdcMint,
    );
    royaltyUsdcTokenAccount = _royaltyUsdcTokenAccount

    const royaltyAccountIX = await program.account.royalty.createInstruction(royalty)
    await program.rpc.initializeRoyalty(
      royaltyPercentageSecondaryMarket,
      royaltyNonce, {
        accounts: {
          authority: provider.wallet.publicKey,
          authorityUsdcTokenAccount: userUsdcTokenAccount,
          royalty: royalty.publicKey,
          royaltyUsdcTokenAccount,
          royaltySigner,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        }, 
        signers: [royalty],
        instructions: [
          royaltyAccountIX,
          royaltyUsdcTokenAccountIx,
        ],
      }
    )

    const _royalty = await program.account.royalty(royalty.publicKey);
    assert.equal(bnToDecimal(_royalty.resalePercentage.toNumber()), .2)
    assert.equal(bnToDecimal(_royalty.royaltyRecipients[0].percentShare.toNumber()), 1)
  })

  it('Updates Royalties After A Primary Sale', async () => {
    await program.rpc.processRoyaltyDeposit(
      true,
      primaryMarketPrice, {
      accounts: {
        purchaser: provider.wallet.publicKey,
        purchaserUsdcTokenAccount: userOutgoingUsdcAccount,
        royalty: royalty.publicKey,
        royaltyUsdcTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      }
    })

    const _royalty = await program.account.royalty(royalty.publicKey);
    assert.equal(_royalty.primarySaleCounter.toNumber(), 1)
    assert.equal(_royalty.totalCollected.toNumber(), primaryMarketPrice)
    const _royaltyUsdcTokenAccount = await getTokenAccount(
      provider,
      _royalty.royaltyUsdcTokenAccount,
    );
    assert.equal(_royaltyUsdcTokenAccount.amount.toNumber(), primaryMarketPrice)
  })

  it('Updates Royalties After A Secondary Sale', async () => {
    await program.rpc.processRoyaltyDeposit(
      false,
      secondaryMarketPrice, {
      accounts: {
        purchaser: provider.wallet.publicKey,
        purchaserUsdcTokenAccount: userOutgoingUsdcAccount,
        royalty: royalty.publicKey,
        royaltyUsdcTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      }
    })

    const _royalty = await program.account.royalty(royalty.publicKey);
    assert.equal(_royalty.secondarySaleCounter.toNumber(), 1)
    assert.equal(_royalty.totalCollected.toNumber(), primaryMarketPrice.toNumber() + secondaryMarketPrice.toNumber())
    const _royaltyUsdcTokenAccount = await getTokenAccount(
      provider,
      _royalty.royaltyUsdcTokenAccount,
    );
    assert.equal(_royaltyUsdcTokenAccount.amount.toNumber(), primaryMarketPrice.toNumber() + secondaryMarketPrice.toNumber())
  })

  it('Collects Royalties when user has 100% of royalty share', async () => {
    await program.rpc.collectRoyalty(
      royaltyNonce, {
      accounts: {
        authority: provider.wallet.publicKey,
        authorityUsdcTokenAccount: userUsdcTokenAccount,
        royalty: royalty.publicKey,
        royaltyUsdcTokenAccount,
        royaltySigner,
        tokenProgram: TOKEN_PROGRAM_ID,
      }
    })

    const _royalty = await program.account.royalty(royalty.publicKey);
    assert.equal(_royalty.secondarySaleCounter.toNumber(), 1)
    assert.equal(_royalty.totalCollected.toNumber(), primaryMarketPrice.toNumber() + secondaryMarketPrice.toNumber())
    const _royaltyUsdcTokenAccount = await getTokenAccount(
      provider,
      _royalty.royaltyUsdcTokenAccount,
    );
    assert.equal(_royaltyUsdcTokenAccount.amount.toNumber(), 0)
    assert.equal(_royalty.royaltyRecipients[0].owed.toNumber(), 0)
    _userUsdcTokenAccount = await getTokenAccount(
      provider,
      userUsdcTokenAccount,
    );
    assert.equal(_userUsdcTokenAccount.amount.toNumber(), primaryMarketPrice.toNumber() + secondaryMarketPrice.toNumber())
  })

  it('Collects Royalties then Transfer Royalty Share to another user', async () => {
    const royaltyPercentToTransfer = new anchor.BN(250000); //25%

    await program.rpc.addRoyaltyRecipient(
      royaltyNonce,
      royaltyPercentToTransfer, {
      accounts: {
        authority: provider.wallet.publicKey,
        authorityUsdcTokenAccount: userUsdcTokenAccount,
        royalty: royalty.publicKey,
        royaltyUsdcTokenAccount,
        royaltySigner,
        newRoyaltyRecipient: provider.wallet.publicKey,
        newRoyaltyRecipientUsdcTokenAccount: secondRoyaltyUsdcTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
    })

    const _royalty = await program.account.royalty(royalty.publicKey);
    assert.equal(_royalty.secondarySaleCounter.toNumber(), 1)
    assert.equal(_royalty.totalCollected.toNumber(), primaryMarketPrice.toNumber() + secondaryMarketPrice.toNumber())
    const _royaltyUsdcTokenAccount = await getTokenAccount(
      provider,
      _royalty.royaltyUsdcTokenAccount,
    );
    assert.equal(_royaltyUsdcTokenAccount.amount.toNumber(), 0)
    assert.equal(_royalty.royaltyRecipients[0].owed.toNumber(), 0)
    assert.equal(_royalty.royaltyRecipients[0].percentShare.toNumber(), 750000)
    assert.equal(_royalty.royaltyRecipients[1].percentShare.toNumber(), 250000)
  })

})