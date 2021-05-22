use anchor_lang::prelude::*;
use anchor_spl::token::{self, TokenAccount, Transfer};

#[program]
mod royalty {
    use super::*;
    pub fn initialize_royalty(
        ctx: Context<InitializeRoyalty>,
        resale_percentage: u64,
        nonce: u8,
    ) -> ProgramResult {
        // Initialize the royalty account with:
        // 
        // Accounts:
        // - an authority
        // - a usdc token account to hold payments from purchasers and disperse royalties to recipients
        // - pda signer (and nonce)
        // 
        // Resale Percentage:
        // - resale percentage for secondary market sales
        // 
        // Counters:
        // - total amount of usdc that has entered the royalty account
        // - total number of primary market sales
        // - total amount of usdc from primary market sales
        // - total number of secondary market sales
        // - total amount of usdc from secondary market sales
        // 
        let mut royalty = ctx.accounts.royalty.load_init()?;
        royalty.authority = *ctx.accounts.authority.to_account_info().key;
        royalty.royalty_usdc_token_account = *ctx.accounts.royalty_usdc_token_account.to_account_info().key;
        royalty.royalty_signer = *ctx.accounts.royalty_signer.to_account_info().key;
        royalty.resale_percentage = resale_percentage;
        royalty.total_collected = 0 as u64;
        royalty.primary_sale_counter = 0 as u64;
        royalty.primary_sale_total = 0 as u64;
        royalty.secondary_sale_counter = 0 as u64;
        royalty.secondary_sale_total = 0 as u64;
        royalty.nonce = nonce;

        // Add the royalty account initializer as the only royalty recipient with 100% share in royalty
        royalty.append({
            RoyaltyRecipient {
                authority: *ctx.accounts.authority.to_account_info().key,
                royalty: *ctx.accounts.royalty.to_account_info().key,
                royalty_recipient_usdc_token_account: *ctx.accounts.authority_usdc_token_account.to_account_info().key,
                percent_share: 1000000 as u64,
                owed: 0 as u64,
                collected: 0 as u64,
            }
        });

        Ok(())
    }

    pub fn process_royalty_deposit(
        ctx: Context<ProcessRoyaltyDeposit>,
        is_primary: bool,
        amount: u64,
    ) -> ProgramResult {
        let mut royalty = ctx.accounts.royalty.load_mut()?;

        // Transfer USDC from Purchaser to Royalty USDC Account
        let cpi_accounts = Transfer {
            from: ctx.accounts.purchaser_usdc_token_account.to_account_info(),
            to: ctx.accounts.royalty_usdc_token_account.to_account_info(),
            authority: ctx.accounts.purchaser.clone(),
        };
        let cpi_program = ctx.accounts.token_program.clone();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        // Update Royalty Counters
        royalty.total_collected += amount;
        if is_primary {
            royalty.primary_sale_counter += 1;
            royalty.primary_sale_total += amount;
        } else {
            royalty.secondary_sale_counter += 1;
            royalty.secondary_sale_total += amount;
        }

        //Update royalty_recipient.owed counter proportionally for all Royalty Recipents
        royalty.update_royalty_recipient_owed(amount);        

        Ok(())
    }

    pub fn collect_royalty(
        ctx: Context<CollectRoyalty>,
        nonce: u8,
    ) -> ProgramResult {
        let mut royalty = ctx.accounts.royalty.load_mut()?;

        let mut royalty_recipient = royalty.find_royalty_recipient(*ctx.accounts.authority.to_account_info().key).unwrap();

        // Transfer Royalties from the royalty account to the royalty recipient trigger collect_royalty action
        let cpi_accounts = Transfer {
            from: ctx.accounts.royalty_usdc_token_account.to_account_info(),
            to: ctx.accounts.authority_usdc_token_account.to_account_info(),
            authority: ctx.accounts.royalty_signer.clone(),
        };
        let cpi_program = ctx.accounts.token_program.clone();
        let seeds = &[
            ctx.accounts.royalty.to_account_info().key.as_ref(),
            &[nonce],
        ];
        let signer = &[&seeds[..]];
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, royalty_recipient.owed as u64)?;

        // Update royalty recipient counters to take account for royalty collection action
        royalty_recipient.collected += royalty_recipient.owed;
        royalty_recipient.owed = 0;

        Ok(())
    }

    pub fn add_royalty_recipient(
        ctx: Context<AddRoyaltyRecipient>,
        nonce:u8,
        percent_share_to_transfer: u64,
    ) -> ProgramResult {
        // Collect Royalty so transferring user has no pending royalties
        let mut royalty = ctx.accounts.royalty.load_mut()?;

        let mut royalty_recipient = royalty.find_royalty_recipient(*ctx.accounts.authority.to_account_info().key).unwrap();

        // Transfer Royalties from the royalty account to the user collecting
        let cpi_accounts = Transfer {
            from: ctx.accounts.royalty_usdc_token_account.to_account_info(),
            to: ctx.accounts.authority_usdc_token_account.to_account_info(),
            authority: ctx.accounts.royalty_signer.clone(),
        };
        let cpi_program = ctx.accounts.token_program.clone();
        let seeds = &[
            ctx.accounts.royalty.to_account_info().key.as_ref(),
            &[nonce],
        ];
        let signer = &[&seeds[..]];
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, royalty_recipient.owed as u64)?;

        // Update royalty recipient counters to take account for royalty collection action
        royalty_recipient.collected += royalty_recipient.owed;
        royalty_recipient.owed = 0;

        // Add New Royalty Recipient
        if percent_share_to_transfer > royalty_recipient.percent_share {
            return Err(ErrorCode::RoyaltyTransferTooLarge.into())
        };

        // Take share from current user
        royalty_recipient.percent_share -= percent_share_to_transfer;

        // And give to new royalty recipient
        royalty.append({
            RoyaltyRecipient {
                authority: *ctx.accounts.new_royalty_recipient.to_account_info().key,
                royalty: *ctx.accounts.royalty.to_account_info().key,
                royalty_recipient_usdc_token_account: *ctx.accounts.new_royalty_recipient_usdc_token_account.to_account_info().key,
                percent_share: percent_share_to_transfer,
                owed: 0 as u64,
                collected: 0 as u64,
            }
        });

        // Make sure royalty shares of all recipients equals 1000000
        if royalty.royalty_equals_1000000() {
            Ok(())
        } else {
            return Err(ErrorCode::RoyaltyExceeds100Percent.into())
        }
    }
}

#[derive(Accounts)]
pub struct InitializeRoyalty<'info>  {
    pub authority: AccountInfo<'info>,
    pub authority_usdc_token_account: AccountInfo<'info>,
    #[account(init)]
    pub royalty: Loader<'info, Royalty>,
    pub royalty_usdc_token_account: AccountInfo<'info>,
    pub royalty_signer: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct ProcessRoyaltyDeposit<'info> {
    #[account(signer)]
    pub purchaser: AccountInfo<'info>,
    #[account(mut)]
    pub purchaser_usdc_token_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub royalty: Loader<'info, Royalty>,
    #[account(mut)]
    pub royalty_usdc_token_account: CpiAccount<'info, TokenAccount>,
    pub token_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CollectRoyalty<'info> {
    pub authority: AccountInfo<'info>,
    #[account(mut)]
    pub royalty: Loader<'info, Royalty>,
    #[account(mut)]
    pub authority_usdc_token_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub royalty_usdc_token_account: CpiAccount<'info, TokenAccount>,
    pub royalty_signer: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AddRoyaltyRecipient<'info> {
    pub authority: AccountInfo<'info>,
    #[account(mut)]
    pub authority_usdc_token_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub royalty_usdc_token_account: CpiAccount<'info, TokenAccount>,
    pub royalty_signer: AccountInfo<'info>,
    #[account(mut)]
    pub royalty: Loader<'info, Royalty>,
    pub new_royalty_recipient: AccountInfo<'info>,
    pub new_royalty_recipient_usdc_token_account: CpiAccount<'info, TokenAccount>,
    pub token_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
}

#[account(zero_copy)]
pub struct Royalty {
    pub authority: Pubkey,
    pub royalty_usdc_token_account: Pubkey,
    pub royalty_signer: Pubkey,
    pub resale_percentage: u64,
    pub total_collected: u64,
    pub primary_sale_counter: u64,
    pub primary_sale_total: u64,
    pub secondary_sale_counter: u64,
    pub secondary_sale_total: u64,
    pub nonce: u8,
    pub head: u64,
    pub tail: u64,
    pub royalty_recipients: [RoyaltyRecipient; 10],
}

impl Royalty {
    fn append(&mut self, royalty_recipient: RoyaltyRecipient) {
        self.royalty_recipients[Royalty::index_of(self.head)] = royalty_recipient;
        if Royalty::index_of(self.head + 1) == Royalty::index_of(self.tail) {
            self.tail += 1;
        }
        self.head += 1;
    }

    fn update_royalty_recipient_owed(&mut self, amount: u64) {
        for royalty_recipient in self.royalty_recipients.iter_mut() {
            if royalty_recipient.percent_share > 0 {
                royalty_recipient.owed += amount * (royalty_recipient.percent_share / 1000000);
            }
        }
    }

    fn find_royalty_recipient(&mut self, pubkey: Pubkey) -> Option<&mut RoyaltyRecipient> {
        for royalty_recipient in self.royalty_recipients.iter_mut() {
            if royalty_recipient.authority == pubkey {
                return Some(royalty_recipient);
            };
        }
        return None
    }

    fn royalty_equals_1000000(&mut self) -> bool {
        let mut royalty_counter = 0;
        for royalty_recipient in self.royalty_recipients.iter_mut() {
            royalty_counter += royalty_recipient.percent_share;
        }

        if royalty_counter == 1000000 {
            return true
        } else {
            return false
        };

    }

    fn index_of(counter: u64) -> usize {
        std::convert::TryInto::try_into(counter % 10).unwrap()
    }
}

#[zero_copy]
pub struct RoyaltyRecipient {
    pub authority: Pubkey,
    pub royalty: Pubkey,
    pub royalty_recipient_usdc_token_account: Pubkey,
    pub percent_share: u64,
    pub owed: u64,
    pub collected: u64,
}

#[error]
pub enum ErrorCode {
    #[msg("Provided Public Key Is Not A Royalty Recipient On This Royalty Account")]
    InvalidRoyaltyRecipient,
    #[msg("Cannot transfer royalty share larger than current share")]
    RoyaltyTransferTooLarge,
    #[msg("Royalty exceeds 100%")]
    RoyaltyExceeds100Percent,
}