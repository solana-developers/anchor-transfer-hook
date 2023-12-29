use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};
use solana_program::{program::invoke_signed, system_instruction};
use spl_tlv_account_resolution::{
    account::ExtraAccountMeta, seeds::Seed, state::ExtraAccountMetaList,
};
use spl_transfer_hook_interface::{
    collect_extra_account_metas_signer_seeds,
    instruction::{ExecuteInstruction, TransferHookInstruction},
};

declare_id!("DrWbQtYJGtsoRwzKqAbHKHKsCJJfpysudF39GBVFSxub");

#[program]
pub mod transfer_hook {
    use super::*;

    pub fn initialize_extra_account_meta_list(
        ctx: Context<InitializeExtraAccountMetaList>,
    ) -> Result<()> {
        let account_metas = [
            // index: 5
            ExtraAccountMeta::new_with_pubkey(&ctx.accounts.wsol_mint.key(), false, false).unwrap(),
            // index: 6
            ExtraAccountMeta::new_with_pubkey(&ctx.accounts.token_program.key(), false, false)
                .unwrap(),
            // index: 7
            ExtraAccountMeta::new_with_pubkey(
                &ctx.accounts.associated_token_program.key(),
                false,
                false,
            )
            .unwrap(),
            // When resolving ExtraAccountMetaList accounts in the Token Extensions program,
            // "index: 4" is address of ExtraAccountMetaList account
            // The `addExtraAccountsToInstruction` JS helper function resolving incorrectly
            ExtraAccountMeta::new_external_pda_with_seeds(
                7, // associated token program index
                &[
                    Seed::AccountKey { index: 3 }, // owner index
                    Seed::AccountKey { index: 6 }, // token program index
                    Seed::AccountKey { index: 5 }, // wsol mint index
                ],
                false,
                true,
            )
            .unwrap(),
            ExtraAccountMeta::new_with_seeds(
                &[Seed::Literal {
                    bytes: "delegate".as_bytes().to_vec(),
                }],
                false,
                true,
            )
            .unwrap(),
            ExtraAccountMeta::new_with_pubkey(&ctx.accounts.delegate_wsol.key(), false, true)
                .unwrap(),
        ];

        let extra_account = &ctx.accounts.extra_account;
        let program_id = ctx.program_id;

        let account_size = ExtraAccountMetaList::size_of(account_metas.len())? as u64;
        msg!("ExtraAccountMetaList Size: {}", account_size);

        let lamports = Rent::get()?.minimum_balance(account_size as usize);
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

    // Require SOL fee on transfer
    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        msg!("Transfer Hook Invoked");

        let signer_seeds: &[&[&[u8]]] = &[&[b"delegate", &[ctx.bumps.delegate]]];
        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.sender_wsol_token_account.to_account_info(),
                    mint: ctx.accounts.wsol_mint.to_account_info(),
                    to: ctx.accounts.delegate_wsol_token_account.to_account_info(),
                    authority: ctx.accounts.delegate.to_account_info(),
                },
            )
            .with_signer(signer_seeds),
            amount,
            ctx.accounts.wsol_mint.decimals,
        )?;
        Ok(())
    }

    pub fn fallback<'info>(
        program_id: &Pubkey,
        accounts: &'info [AccountInfo<'info>],
        data: &[u8],
    ) -> Result<()> {
        // // Iterating through accounts and printing each with its index
        // for (index, account) in accounts.iter().enumerate() {
        //     msg!("Account Index: {}, Account: {:?}", index, account.key);
        // }

        let instruction = TransferHookInstruction::unpack(data)?;
        match instruction {
            TransferHookInstruction::Execute { amount } => {
                msg!("Instruction: Execute");

                let amount_bytes = amount.to_le_bytes();
                __private::__global::transfer_hook(program_id, accounts, &amount_bytes)
            }
            _ => return Err(ProgramError::InvalidInstructionData.into()),
        }
    }
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
    pub wsol_mint: InterfaceAccount<'info, Mint>,
    pub delegate_wsol: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

// Note: Order of accounts matters for this struct.
// TODO: Add constraints
#[derive(Accounts)]
pub struct TransferHook<'info> {
    pub source_token: InterfaceAccount<'info, TokenAccount>, // 0
    pub mint: InterfaceAccount<'info, Mint>,                 // 1
    pub destination_token: InterfaceAccount<'info, TokenAccount>, // 2
    /// CHECK: source token account owner, can be SystemAccount or PDA owned by another program
    pub owner: UncheckedAccount<'info>,
    /// CHECK: ExtraAccountMetaList Account,
    #[account(
        seeds = [b"extra-account-metas", mint.key().as_ref()], 
        bump)
    ]
    pub extra_account: UncheckedAccount<'info>,
    pub wsol_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    #[account(mut)]
    pub sender_wsol_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"delegate"], 
        bump)
    ]
    pub delegate: SystemAccount<'info>,
    #[account(mut)]
    pub delegate_wsol_token_account: InterfaceAccount<'info, TokenAccount>,
}
