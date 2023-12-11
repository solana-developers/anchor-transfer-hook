use anchor_lang::{prelude::*, system_program::{Transfer, transfer}};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface, TransferChecked, transfer_checked};
use solana_program::{program::invoke_signed, system_instruction};
use spl_tlv_account_resolution::{
    account::ExtraAccountMeta, 
    state::ExtraAccountMetaList,
    seeds::Seed,
};
use spl_transfer_hook_interface::{
    collect_extra_account_metas_signer_seeds, 
    instruction::{ExecuteInstruction, TransferHookInstruction}
};

declare_id!("DrWbQtYJGtsoRwzKqAbHKHKsCJJfpysudF39GBVFSxub");

#[program]
pub mod transfer_hook {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.bump = ctx.bumps.counter; // store bump seed in `Counter` account
        msg!("Counter account created! Current count: {}", counter.count);
        msg!("Counter bump: {}", counter.bump);
        Ok(())
    }

    pub fn initialize_extra_account_meta_list(
        ctx: Context<InitializeExtraAccountMetaList>,
    ) -> Result<()> {
        let account_metas = [
            ExtraAccountMeta::new_with_pubkey(&ctx.accounts.counter.key(), false, true).unwrap(),
            ExtraAccountMeta::new_with_pubkey(&ctx.accounts.token_program.key(), false, false).unwrap(),
            ExtraAccountMeta::new_with_seeds(
                &[
                    Seed::Literal {
                        bytes: b"delegate".to_vec(),
                    },
                    Seed::AccountKey { index: 0 },
                ],
                false,
                false,
            ).unwrap(),
        ];

        let extra_account = &ctx.accounts.extra_account;
        let program_id = ctx.program_id;

        let account_size = ExtraAccountMetaList::size_of(account_metas.len())? as u64;
        msg!("ExtraAccountMetaList Size: {}", account_size);

        let lamports =  Rent::get()?.minimum_balance(account_size as usize);
        transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.extra_account.to_account_info(),
                },
            ),
            lamports,
        )?;

        let bump_seed = [ctx.bumps.extra_account];
        let mint = ctx.accounts.mint.key();
        let signer_seeds = collect_extra_account_metas_signer_seeds(&mint, &bump_seed);

        let allocate = system_instruction::allocate(extra_account.key, account_size);
        let assign = system_instruction::assign(extra_account.key, program_id);
        invoke_signed(&allocate, &[extra_account.clone()], &[&signer_seeds])?;
        invoke_signed(&assign, &[extra_account.clone()], &[&signer_seeds])?;

        let mut data = extra_account.try_borrow_mut_data()?;
        ExtraAccountMetaList::init::<ExecuteInstruction>(&mut data, &account_metas)?;

        msg!("ExtraAccountMetaList Account: {}", extra_account.key);

        Ok(())
    }

    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        msg!("Transfer Hook Invoked");
        msg!("Transfer Amount: {}", amount);

        let counter = &mut ctx.accounts.counter;
        msg!("Previous counter: {}", counter.count);
        counter.count = counter.count.checked_add(1).unwrap();
        msg!("Counter incremented! Current count: {}", counter.count);

        msg!("Token Program: {}", ctx.accounts.token_program.key());


        let source_token = ctx.accounts.source_token.key();
        let signer_seeds: &[&[&[u8]]] = &[&[b"delegate", source_token.as_ref(), &[ctx.bumps.delegate]]];
        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.source_token.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.destination_token.to_account_info(),
                    authority: ctx.accounts.delegate.to_account_info(),
                },
            ).with_signer(signer_seeds),
            ctx.accounts.source_token.delegated_amount,
            ctx.accounts.mint.decimals,
        )?;
        
        msg!("Source: {}", ctx.accounts.source_token.key());
        msg!("Mint: {}", ctx.accounts.mint.key());
        msg!("Destination: {}", ctx.accounts.destination_token.key());
        msg!("Owner: {}", ctx.accounts.owner.key());
        msg!("ExtraAccountMetaList: {}", ctx.accounts.extra_account.key());
        msg!("Counter: {}", ctx.accounts.counter.key());

        msg!("Delegate: {}", ctx.accounts.delegate.key());
        msg!("Source Delegate: {:?}", ctx.accounts.source_token.delegate);
        Ok(())
    }

    pub fn fallback<'info>(
        program_id: &Pubkey,
        accounts: &'info [AccountInfo<'info>],
        data: &[u8],
    ) -> Result<()> {
        let instruction = TransferHookInstruction::unpack(data)?;
        match instruction {
            TransferHookInstruction::Execute { amount } => {
                msg!("Instruction: Execute");
                let amount_bytes = amount.to_le_bytes();
                __private::__global::transfer_hook(program_id, accounts, &amount_bytes)
            }
            _ => return Err(ProgramError::InvalidInstructionData.into()),
        }
        // pub const TRANSFER_HOOK_DISCRIMINATOR: [u8; 8] = [105, 37, 101, 197, 75, 251, 102, 26];
        // let (discriminator, remaining_ix_data) = data.split_at(8);

        // if discriminator == &TRANSFER_HOOK_DISCRIMINATOR {
        //     __private::__global::transfer_hook(program_id, accounts, remaining_ix_data)
        // } else {
        //     Err(ProgramError::InvalidInstructionData.into())
        // }
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init,
        seeds = [b"counter"], // optional seeds for pda
        bump,                 // bump seed for pda
        payer = user,
        space = 8 + Counter::INIT_SPACE
    )]
    pub counter: Account<'info, Counter>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(mut)]
    payer: Signer<'info>,

    /// CHECK: ExtraAccountMetaList Account, must use these seeds
    #[account(
        mut,
        seeds = [b"extra-account-metas", mint.key().as_ref()], 
        bump)
    ]
    pub extra_account: AccountInfo<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        seeds = [b"counter"],
        bump = counter.bump,               
    )]
    pub counter: Account<'info, Counter>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

// Note: Order of accounts matters for this struct.
// TODO: Add constraints
#[derive(Accounts)]
pub struct TransferHook<'info> {
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: source token account owner, can be SystemAccount or PDA owned by another program
    pub owner: UncheckedAccount<'info>,

    /// CHECK: ExtraAccountMetaList Account, must use these seeds
    #[account(
        seeds = [b"extra-account-metas", mint.key().as_ref()], 
        bump)
    ]
    pub extra_account: UncheckedAccount<'info>,
    #[account(
        seeds = [b"counter"],
        bump = counter.bump,               
    )]
    pub counter: Account<'info, Counter>,
    pub token_program: Interface<'info, TokenInterface>,
    #[account(
        seeds = [b"delegate", source_token.key().as_ref()],
        bump,               
    )]
    pub delegate: SystemAccount<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct Counter {
    pub count: u64, // 8 bytes
    pub bump: u8,   // 1 byte
}
