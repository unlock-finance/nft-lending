use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token::{self, Mint, TokenAccount};

declare_id!("DEqb986wmZSC2muSNZwYyoRd35zHcYhXkCv5muB8rCXk");

#[program]
pub mod nft_lending {
    use super::*;
    pub fn initialize(
        ctx: Context<Initialize>,
        bump: u8,
        loan_amount: u64,
        default_at: i64,
        //lender: Option<Pubkey>,
    ) -> ProgramResult {
        if loan_amount == 0 {
            return Err(NftLendingError::LoanCannotBeZero.into());
        }
        // put nft to vault
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.borrower_token_account.to_account_info(),
                    to: ctx.accounts.nft_vault.to_account_info(),
                    authority: ctx.accounts.borrower.to_account_info(),
                },
            ),
            1 as u64, // expect error as this might be stored in lamports
        )?;

        let loan_agreement = &mut ctx.accounts.loan_agreement;
        loan_agreement.bump = bump;
        loan_agreement.loan_amount = loan_amount;
        loan_agreement.default_at = default_at;
        loan_agreement.nft_amount = 1 as u64;
        //loan_agreement.lender = lender;
        loan_agreement.borrowed = false;

        Ok(())
    }

    pub fn lender(ctx: Context<Lender>, expected_amount: u64) -> ProgramResult {
        let loan_agreement = &ctx.accounts.loan_agreement;
        if loan_agreement.loan_amount != expected_amount {
            return Err(NftLendingError::UnexpectedLoanAgreement.into());
        }

        let loan_agreement_pk = loan_agreement.key();
        let seeds = &[
            loan_agreement_pk.as_ref(),
            b"authority".as_ref(),
            &[loan_agreement.bump],
        ];
        //transfer NFT from vault to collateral
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info().clone(),
                token::Transfer {
                    from: ctx.accounts.nft_vault.to_account_info(),
                    to: ctx.accounts.collateral.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
                &[&seeds[..]],
            ),
            loan_agreement.nft_amount, //1
        )?;

        // transfer sols from lender to borrower loan token
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.lender_token_account.to_account_info(),
                    to: ctx.accounts.borrower_loan_token_account.to_account_info(),
                    authority: ctx.accounts.lender.to_account_info(),
                },
            ),
            loan_agreement.loan_amount,
        )?;

        ctx.accounts.loan_agreement.borrowed = true;

        Ok(())
    }

    pub fn repay(ctx: Context<Repay>) -> ProgramResult {
        let loan_agreement = &ctx.accounts.loan_agreement;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.borrower_loan_token_account.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.borrower.to_account_info(),
                },
            ),
            loan_agreement.loan_amount,
        )?;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info().clone(),
                token::Transfer {
                    from: ctx.accounts.collateral.to_account_info(),
                    to: ctx
                        .accounts
                        .borrower_collateral_token_account
                        .to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            loan_agreement.nft_amount,
        )?;

        ctx.accounts.loan_agreement.borrowed = false;
        Ok(())
    }

    pub fn close(ctx: Context<Close>) -> ProgramResult {
        let loan_agreement = &ctx.accounts.loan_agreement;

        if loan_agreement.borrowed && Clock::get()?.unix_timestamp < loan_agreement.default_at {
            return Err(NftLendingError::DefaultAtIsNotReached.into());
        }

        let loan_agreement_pk = loan_agreement.key();
        let seeds = &[
            loan_agreement_pk.as_ref(),
            b"authority".as_ref(),
            &[loan_agreement.bump],
        ];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info().clone(),
                token::Transfer {
                    from: ctx.accounts.collateral.to_account_info(),
                    to: ctx
                        .accounts
                        .lender_collateral_token_account
                        .to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
                &[&seeds[..]],
            ),
            loan_agreement.loan_amount, // Empty collateral
        )?;

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info().clone(),
                token::Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.lender_loan_token_account.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
                &[&seeds[..]],
            ),
            ctx.accounts.vault.amount, // Empty loan
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = borrower
    )]
    loan_agreement: Account<'info, LoanAgreement>,
    #[account(
        seeds = [loan_agreement.key().as_ref(), b"authority"],
        bump,
    )]
    authority: UncheckedAccount<'info>,
    #[account(
        init,
        seeds = [loan_agreement.key().as_ref(), b"nft_vault"],
        bump,
        token::mint = nft_vault_mint,
        token::authority = authority,
        payer = borrower
    )]
    nft_vault: Account<'info, TokenAccount>, // NFT
    nft_vault_mint: Box<Account<'info, Mint>>, // NFT mint
    #[account(
        init,
        seeds = [loan_agreement.key().as_ref(), b"vault"],
        bump,
        token::mint = vault_mint,
        token::authority = authority,
        payer = borrower
    )]
    vault: Account<'info, TokenAccount>, // Sol
    vault_mint: Box<Account<'info, Mint>>,     // Sol mint
    #[account(
        init,
        seeds = [loan_agreement.key().as_ref(), b"collateral"],
        bump,
        token::mint = collateral_mint,
        token::authority = authority,
        payer = borrower,
    )]
    collateral: Account<'info, TokenAccount>, // Nft expecting
    collateral_mint: Box<Account<'info, Mint>>, // Nft mint

    #[account(mut)]
    borrower: Signer<'info>,
    #[account(mut)]
    borrower_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    borrower_collateral_token_account: Account<'info, TokenAccount>,
    token_program: Program<'info, Token>,
    system_program: Program<'info, System>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Lender<'info> {
    #[account(mut)]
    loan_agreement: Account<'info, LoanAgreement>,
    #[account(
        seeds = [loan_agreement.key().as_ref(), b"authority"],
        bump,
    )]
    authority: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [loan_agreement.key().as_ref(), b"collateral"],
        bump,
    )]
    collateral: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [loan_agreement.key().as_ref(), b"nft_vault"],
        bump,
    )]
    nft_vault: Account<'info, TokenAccount>,

    lender: Signer<'info>,
    #[account(mut)]
    borrower_loan_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    lender_token_account: Account<'info, TokenAccount>,

    token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Repay<'info> {
    #[account(mut)]
    loan_agreement: Account<'info, LoanAgreement>,
    #[account(
        seeds = [loan_agreement.key().as_ref(), b"authority"],
        bump,
    )]
    authority: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [loan_agreement.key().as_ref(), b"collateral"],
        bump,
    )]
    collateral: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [loan_agreement.key().as_ref(), b"vault"],
        bump,
    )]
    vault: Account<'info, TokenAccount>,

    borrower: Signer<'info>,
    #[account(mut)]
    borrower_loan_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    borrower_collateral_token_account: Account<'info, TokenAccount>,

    token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut, close = lender, has_one = lender)]
    loan_agreement: Account<'info, LoanAgreement>,
    #[account(
        seeds = [loan_agreement.key().as_ref(), b"authority"],
        bump,
    )]
    authority: UncheckedAccount<'info>,
    #[account(
        mut,
        close = lender,
        seeds = [loan_agreement.key().as_ref(), b"collateral"],
        bump,
    )]
    collateral: Account<'info, TokenAccount>,
    #[account(
        mut,
        close = lender,
        seeds = [loan_agreement.key().as_ref(), b"vault"],
        bump,
    )]
    vault: Account<'info, TokenAccount>,

    #[account(mut)]
    lender: Signer<'info>,
    #[account(mut)]
    lender_loan_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    lender_collateral_token_account: Account<'info, TokenAccount>,

    token_program: Program<'info, Token>,
}

#[account]
#[derive(Default)]
pub struct LoanAgreement {
    bump: u8,
    borrower: Pubkey,
    lender: Pubkey, // Borrower can be constrained or not by the loan agreement
    loan_amount: u64,
    nft_amount: u64,
    default_at: i64,
    borrowed: bool,
}

#[error]
pub enum NftLendingError {
    #[msg("Loan cannot be zero")]
    LoanCannotBeZero,
    #[msg("Expected cannot be zero")]
    ExpectedCannotBeZero,
    #[msg("Collateral cannot be zero")]
    CollateralCannotBeZero,
    #[msg("Unexpected loan agreement")]
    UnexpectedLoanAgreement,
    #[msg("Default at is not reached")]
    DefaultAtIsNotReached,
    #[msg("Incorrect borrower")]
    IncorrectBorrower,
}
