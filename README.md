# Nina Royalty Program

Proof of concept for a royalty distribution program.

## Description

This repo demonstrates a example of a royalty program built in Anchor on Solana.

Royalty Accounts are initialized with a single authority holding 100% of the royalty share.  This authority can transfer their share to a theoretically unlimited amount of RoyaltyRecipients - there are checks in place to make sure that the Royalty share never goes above 100%.  Any RoyaltyRecipient can transfer some or all of their Royalty share to other users on Solana.  Any RoyaltyRecipient can collect their Royalties at any time - with the other RoyaltyRecipients shares remaining in the Royalty Accounts USDC Token Account.

## Getting Started

* [This](https://project-serum.github.io/anchor/getting-started/installation.html "Anchor Homepage")  is a helpful step-by-step guide on installing [Anchor](https://project-serum.github.io/anchor/) and it's dependencies.
* run `anchor test`

## Acknowledgments & Helpful Repos

Inspiration, code snippets, etc.
* [Anchor](https://github.com/project-serum/anchor)
* [Armani Ferrante](https://github.com/armaniferrante)
