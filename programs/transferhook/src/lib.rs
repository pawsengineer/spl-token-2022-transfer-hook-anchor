use anchor_lang::prelude::*;
use {
    anchor_spl::
        token_2022::spl_token_2022::{
                extension::{
                    transfer_hook::TransferHookAccount,
                    BaseStateWithExtensions, StateWithExtensions,
                },
                state::Account as Token2022Account,
            },
    spl_transfer_hook_interface::error::TransferHookError,
};
use spl_transfer_hook_interface::instruction::TransferHookInstruction;

declare_id!("7aeu4HRHR4UwQndRDyh5f7nMwgxgH3rrtLgRntxdivZw");

fn check_token_account_is_transferring(account_data: &[u8]) -> Result<()> {
	let token_account = StateWithExtensions::<Token2022Account>::unpack(account_data)?;
	let extension = token_account.get_extension::<TransferHookAccount>()?;
	if bool::from(extension.transferring) {
		Ok(())
	} else {
		Err(Into::<ProgramError>::into(
			TransferHookError::ProgramCalledOutsideOfTransfer,
		))?
	}
}

#[program]
pub mod transferhook {
    use solana_program::program::invoke_signed;
    use solana_program::system_instruction;
    use spl_transfer_hook_interface::collect_extra_account_metas_signer_seeds;
    use spl_transfer_hook_interface::instruction::ExecuteInstruction;
    use spl_tlv_account_resolution::state::ExtraAccountMetaList;
    use spl_tlv_account_resolution::account::ExtraAccountMeta;
    use spl_pod::primitives::PodBool;

    use super::*;

    /// Initialize the counter account.
    /// This function creates a new counter account and sets the owner to the authority.
    /// The counter account is used to count the number of times the transfer hook has been invoked.
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.owner = *ctx.accounts.authority.key;
        counter.count = 0;
        Ok(())
    }

    pub fn transfer_hook<'a>(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        // Count the number of times the transfer hook has been invoked.
        let counter = &mut ctx.accounts.counter;
        counter.count += 1;

        let source_account = &ctx.accounts.source;
    	let destination_account = &ctx.accounts.destination;

        check_token_account_is_transferring(&source_account.to_account_info().try_borrow_data()?)?;
    	check_token_account_is_transferring(&destination_account.to_account_info().try_borrow_data()?)?;

        msg!("Transfer hook invoked");
        msg!("Transfer amount: {}", amount);
        msg!("Transfer with extra account PDA: {}", ctx.accounts.extra_account.key());
        msg!("Transfer with counter.count: {}", counter.count);
        Ok(())
    }

    /// Initialize the extra account meta list.
    /// This function creates a new extra account meta list and allocates the extra account PDA.
    /// The extra account PDA is used to store the extra account meta list.
    pub fn initialize_extra_account_meta_list(ctx: Context<InitializeExtraAccountMetaList>, bump_seed: u8) -> Result<()> {
        // Create the extra account meta list.
        let account_metas = vec![
            ExtraAccountMeta {
                discriminator: 0,
                address_config: ctx.accounts.counter.key().to_bytes(),
                is_signer: PodBool::from(false),
                is_writable: PodBool::from(true),
            }];

        // Allocate extra account PDA account.
        let bump_seed = [bump_seed];
        let signer_seeds = collect_extra_account_metas_signer_seeds(ctx.accounts.mint.key, &bump_seed);
        let account_size = ExtraAccountMetaList::size_of(account_metas.len())?;
        invoke_signed(
            &system_instruction::allocate(ctx.accounts.extra_account.key, account_size as u64),
            &[ctx.accounts.extra_account.clone()],
            &[&signer_seeds],
        )?;
        invoke_signed(
            &system_instruction::assign(ctx.accounts.extra_account.key, ctx.program_id),
            &[ctx.accounts.extra_account.clone()],
            &[&signer_seeds],
        )?;

        // Write the extra account meta list to the extra account PDA.
        let mut data = ctx.accounts.extra_account.try_borrow_mut_data()?;
        ExtraAccountMetaList::init::<ExecuteInstruction>(&mut data, &account_metas)?;

        msg!("Extra account meta list initialized on {}", ctx.accounts.extra_account.key());
        Ok(())
    }

    /// Fallback function to handle the transfer hook instruction.
    pub fn fallback<'a>(program_id: &Pubkey, accounts: &'a[AccountInfo<'a>], data: &[u8]) -> Result<()> {
        let instruction = TransferHookInstruction::unpack(data)?;
        match instruction {
            TransferHookInstruction::Execute { amount } => {
                let amount = amount.to_le_bytes();
                __private::__global::transfer_hook(program_id, accounts, &amount)
            }
            _ => Err(ProgramError::InvalidInstructionData.into()),
        }
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, 
        seeds = [authority.key().as_ref()], 
        bump, 
        payer = authority, 
        space = 8 + 128)
    ]
    pub counter: Account<'info, Counter>,
    pub system_program: Program<'info, System>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct TransferHook<'info> {
    /// CHECK:
    pub source: AccountInfo<'info>,
    /// CHECK:
    pub mint: AccountInfo<'info>,
    /// CHECK:
    pub destination: AccountInfo<'info>,
    /// CHECK:
    pub authority: AccountInfo<'info>,
    /// CHECK: must be the extra account PDA
    #[account(
        seeds = [b"extra-account-metas", mint.key().as_ref()], 
        bump)
    ]
    pub extra_account: AccountInfo<'info>,
    /// CHECK:
    pub counter: Account<'info, Counter>,
}

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    /// CHECK: must be the extra account PDA
    #[account(mut,
        seeds = [b"extra-account-metas", mint.key().as_ref()], 
        bump)
    ]
    pub extra_account: AccountInfo<'info>,
    #[account(mut)]
    pub counter: Account<'info, Counter>,
    /// CHECK:
    pub mint: AccountInfo<'info>,
    /// CHECK:
    pub authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Counter {
    pub owner: Pubkey,
    pub count: u64,
}
