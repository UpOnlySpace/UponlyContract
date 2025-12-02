use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::spl_token::instruction::AuthorityType;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer};

declare_id!("2XRYELdk3k9XFs7JkSw55sz5aZWqNeZhY71rZr2pYETu");


#[program]
pub mod up_only {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, team: Pubkey) -> Result<()> {
        if ctx.accounts.metadata.initialized {
            return Err(CustomError::AlreadyInitialized.into());
        }

        if ctx.accounts.global_state.initialized {
            return Err(CustomError::AlreadyInitialized.into());
        }

        let (expected_metadata_pda, _) = Pubkey::find_program_address(
            &[b"metadata", ctx.accounts.up_only_mint.key().as_ref()],
            ctx.program_id,
        );

        require!(
            ctx.accounts.metadata.key() == expected_metadata_pda,
            CustomError::InvalidTokenMint
        );

        let (pool_authority, _) = Pubkey::find_program_address(
            &[b"token_account", ctx.accounts.payment_token_mint.key().as_ref()],
            ctx.program_id,
        );
        let (up_pool_authority, _) = Pubkey::find_program_address(
            &[b"token_account", ctx.accounts.up_usdc_mint.key().as_ref()],
            ctx.program_id,
        );

        require!(
            ctx.accounts.program_payment_token_account.owner == pool_authority,
            CustomError::InvalidOwner
        );
        require!(
            ctx.accounts.program_up_only_account.owner == ctx.accounts.mint_authority.key(),
            CustomError::InvalidOwner
        );
        require!(
            ctx.accounts.program_up_usdc_account.owner == up_pool_authority,
            CustomError::InvalidOwner
        );

        let (mint_authority, _) =
            Pubkey::find_program_address(&[b"mint_authority"], ctx.program_id);
        let (up_usdc_mint_authority, _) =
            Pubkey::find_program_address(&[b"up_usdc_mint_authority"], ctx.program_id);

        let metadata = &mut ctx.accounts.metadata;
        metadata.name = "UpOnly".to_string();
        metadata.symbol = "UP".to_string();
        metadata.mint = ctx.accounts.up_only_mint.key();
        metadata.authority = mint_authority;
        metadata.payment_token = ctx.accounts.payment_token_mint.key();
        metadata.up_usdc_mint = ctx.accounts.up_usdc_mint.key();
        metadata.initialized = true;
        metadata.deployer = ctx.accounts.authority.key();
        metadata.team = team;

        let cpi_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.user_payment_token_account.to_account_info(),
                to: ctx.accounts.program_payment_token_account.to_account_info(),
                authority: ctx.accounts.authority.to_account_info(),
            },
        );
        token::transfer(cpi_context, 1_000_000)?;

        let cpi_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.user_up_usdc_account.to_account_info(),
                to: ctx.accounts.program_up_usdc_account.to_account_info(),
                authority: ctx.accounts.authority.to_account_info(),
            },
        );
        token::transfer(cpi_context, 1_000_000)?;

        let cpi_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.user_up_only_account.to_account_info(),
                to: ctx.accounts.program_up_only_account.to_account_info(),
                authority: ctx.accounts.authority.to_account_info(),
            },
        );
        token::transfer(cpi_context, 1_000_000_000)?;

        let cpi_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::SetAuthority {
                account_or_mint: ctx.accounts.up_only_mint.to_account_info(),
                current_authority: ctx.accounts.current_mint_authority.to_account_info(),
            },
        );

        token::set_authority(cpi_context, AuthorityType::MintTokens, Some(mint_authority))?;

        let cpi_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::SetAuthority {
                account_or_mint: ctx.accounts.up_only_mint.to_account_info(),
                current_authority: ctx.accounts.current_mint_authority.to_account_info(),
            },
        );
        token::set_authority(cpi_context, AuthorityType::FreezeAccount, None)?;

        let cpi_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::SetAuthority {
                account_or_mint: ctx.accounts.up_usdc_mint.to_account_info(),
                current_authority: ctx.accounts.current_up_usdc_authority.to_account_info(),
            },
        );

        token::set_authority(
            cpi_context,
            AuthorityType::MintTokens,
            Some(up_usdc_mint_authority),
        )?;

        let cpi_context = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::SetAuthority {
                account_or_mint: ctx.accounts.up_usdc_mint.to_account_info(),
                current_authority: ctx.accounts.current_up_usdc_authority.to_account_info(),
            },
        );

        token::set_authority(cpi_context, AuthorityType::FreezeAccount, None)?;

        ctx.accounts.global_state.initialized = true;
        Ok(())
    }

    pub fn initialize_founders_pool(ctx: Context<InitializeFoundersPool>) -> Result<()> {
        require!(
            ctx.accounts.authority.key() == ctx.accounts.metadata.deployer,
            CustomError::Unauthorized
        );

        require!(
            ctx.accounts.usdc_mint.key() == ctx.accounts.metadata.payment_token,
            CustomError::InvalidDeployerAccount
        );

        let expected_founder_pool_token_account =
            anchor_spl::associated_token::get_associated_token_address(
                &ctx.accounts.founder_authority.key(),
                &ctx.accounts.usdc_mint.key(),
            );
        require!(
            ctx.accounts.founder_pool_token_account.key() == expected_founder_pool_token_account,
            CustomError::InvalidDeployerAccount
        );

        let pool = &mut ctx.accounts.founders_pool;
        pool.total_collected = 0;
        pool.founder_count = 0;
        pool.founders = vec![Pubkey::default(); 60];
        pool.claim_status = vec![0u64; 60];

        if ctx.accounts.founder_pool_token_account.lamports() == 0 {
            let cpi_ctx = CpiContext::new(
                ctx.accounts.associated_token_program.to_account_info(),
                anchor_spl::associated_token::Create {
                    payer: ctx.accounts.authority.to_account_info(),
                    associated_token: ctx.accounts.founder_pool_token_account.to_account_info(),
                    authority: ctx.accounts.founder_authority.to_account_info(),
                    mint: ctx.accounts.usdc_mint.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                },
            );
            anchor_spl::associated_token::create(cpi_ctx)?;
        }

        Ok(())
    }

    pub fn initialize_user_vault(ctx: Context<InitializeUserVault>) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.associated_token_program.to_account_info(),
            anchor_spl::associated_token::Create {
                payer: ctx.accounts.user.to_account_info(),
                associated_token: ctx.accounts.vault_token_account.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
                mint: ctx.accounts.token_mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            },
        );
        anchor_spl::associated_token::create(cpi_ctx)?;
        Ok(())
    }

    pub fn initialize_leverage_user_vault(ctx: Context<InitializeLeverageUserVault>) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.associated_token_program.to_account_info(),
            anchor_spl::associated_token::Create {
                payer: ctx.accounts.user.to_account_info(),
                associated_token: ctx.accounts.vault_token_account.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
                mint: ctx.accounts.token_mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            },
        );
        anchor_spl::associated_token::create(cpi_ctx)?;
        Ok(())
    }

    pub fn buy_and_lock_token(
        ctx: Context<BuyAndLockToken>,
        amount: u64,
        lock_days: u64,
        referral: Option<Pubkey>,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let lock_state = &mut ctx.accounts.lock_state;
        require!(!lock_state.initialized, CustomError::AlreadyInitialized);
        require!(
            matches!(lock_days, 1 | 2 | 3 | 4 | 6 | 8 | 12),
            CustomError::InvalidLockPeriod
        );

        require!(ctx.accounts.metadata.initialized, CustomError::AlreadyInitialized);
        
        let payment_token_mint = ctx.accounts.metadata.payment_token;
        require!(
            ctx.accounts.vault_token_account.mint == ctx.accounts.metadata.mint,
            CustomError::InvalidTokenMint
        );
        require!(
            ctx.accounts.user_usdc_account.mint == payment_token_mint,
            CustomError::InvalidTokenMint
        );
        require!(
            ctx.accounts.deployer_usdc_account.mint == payment_token_mint,
            CustomError::InvalidTokenMint
        );
        require!(
            ctx.accounts.program_payment_token_account.mint == payment_token_mint,
            CustomError::InvalidTokenMint
        );

        let expected_deployer_usdc_account = anchor_spl::associated_token::get_associated_token_address(
            &ctx.accounts.metadata.team,
            &ctx.accounts.metadata.payment_token,
        );
        require!(
            ctx.accounts.deployer_usdc_account.key() == expected_deployer_usdc_account,
            CustomError::InvalidDeployerUsdcAccount
        );

        let (expected_pool_authority, _) = Pubkey::find_program_address(
            &[b"token_account", ctx.accounts.metadata.payment_token.as_ref()],
            ctx.program_id,
        );
        require!(
            ctx.accounts.pool_authority.key() == expected_pool_authority,
            CustomError::InvalidOwner
        );

        require!(
            ctx.accounts.user_up_usdc_account.mint == ctx.accounts.metadata.up_usdc_mint,
            CustomError::InvalidTokenMint
        );
        require!(
            ctx.accounts.program_up_usdc_account.mint == ctx.accounts.metadata.up_usdc_mint,
            CustomError::InvalidTokenMint
        );
        require!(
            ctx.accounts.up_usdc_mint.key() == ctx.accounts.metadata.up_usdc_mint,
            CustomError::InvalidTokenMint
        );

        let expected_founder_pool_token_account = anchor_spl::associated_token::get_associated_token_address(
            &ctx.accounts.founder_authority.key(),
            &ctx.accounts.metadata.payment_token,
        );
        require!(
            ctx.accounts.founder_pool_token_account.key() == expected_founder_pool_token_account,
            CustomError::InvalidFounderPoolTokenAccount
        );

        let expected_program_up_usdc_account = anchor_spl::associated_token::get_associated_token_address(
            &ctx.accounts.up_pool_authority.key(),
            &ctx.accounts.metadata.up_usdc_mint,
        );
        require!(
            ctx.accounts.program_up_usdc_account.key() == expected_program_up_usdc_account,
            CustomError::InvalidProgramUpUsdcAccount
        );

        let config = get_lock_fee_config(lock_days);
        let total_usdc = amount;
        let team_share = total_usdc * config.team_bps / 10_000;
        let founder_fee = total_usdc * config.founder_bps / 10_000;
        let locked_share = total_usdc * config.liquidity_bps / 10_000;
        let usdc_for_tokens = total_usdc - team_share - founder_fee - locked_share;

        let liquidity_balance =
            token::accessor::amount(&ctx.accounts.program_up_usdc_account.to_account_info())?;
        let token_supply = ctx.accounts.token_mint.supply;

        let mut price_start =
            (liquidity_balance as u128) * 1_000_000_000 / (token_supply.max(1) as u128);
        if price_start == 0 {
            price_start = 1;
        }
        let estimated_tokens = (usdc_for_tokens as u128) * 1_000_000_000 / price_start;
        let liquidity_growth =
            (liquidity_balance as u128) + (usdc_for_tokens as u128) + (locked_share as u128);
        let price_end =
            liquidity_growth * 1_000_000_000 / (((token_supply as u128) + estimated_tokens).max(1));
        let avg_price = (price_start + price_end) / 2;
        require!(avg_price > 0, CustomError::InsufficientAmount);
        let usdc_for_tokens_u128 = usdc_for_tokens as u128;
        let multiplier = 1_000_000_000u128;
        let numerator = usdc_for_tokens_u128 * multiplier;
        let mintable_tokens = (numerator / avg_price) as u64;
        
        require!(mintable_tokens > 0, CustomError::InsufficientAmount);
        
        if let Some(ref_pubkey) = referral {
            let referral_token_account = ctx
                .accounts
                .referral_usdc_account
                .as_ref()
                .ok_or(CustomError::MissingReferralAccount)?;
            require!(
                referral_token_account.owner == ref_pubkey,
                CustomError::InvalidReferral
            );

            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user_usdc_account.to_account_info(),
                        to: referral_token_account.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                team_share / 2,
            )?;

            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user_usdc_account.to_account_info(),
                        to: ctx.accounts.deployer_usdc_account.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                team_share / 2,
            )?;
        } else {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user_usdc_account.to_account_info(),
                        to: ctx.accounts.deployer_usdc_account.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                team_share,
            )?;
        }

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.user_usdc_account.to_account_info(),
                    to: ctx.accounts.founder_pool_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            founder_fee,
        )?;

        let pool = &mut ctx.accounts.founders_pool;
        pool.total_collected += founder_fee;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_usdc_account.to_account_info(),
                    to: ctx.accounts.program_payment_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            usdc_for_tokens + locked_share,
        )?;

        let up_usdc_mint_bump = ctx.bumps.up_usdc_mint_authority;
        let up_usdc_signer_seeds: &[&[&[u8]]] =
            &[&[b"up_usdc_mint_authority", &[up_usdc_mint_bump]]];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.up_usdc_mint.to_account_info(),
                    to: ctx.accounts.program_up_usdc_account.to_account_info(),
                    authority: ctx.accounts.up_usdc_mint_authority.to_account_info(),
                },
                up_usdc_signer_seeds,
            ),
            usdc_for_tokens + locked_share,
        )?;

        let mint_bump = ctx.bumps.mint_authority;
        let signer_seeds: &[&[&[u8]]] = &[&[b"mint_authority", &[mint_bump]]];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.mint_authority.to_account_info(),
                },
                signer_seeds,
            ),
            mintable_tokens,
        )?;

        lock_state.user = ctx.accounts.user.key();
        lock_state.amount = mintable_tokens;
        lock_state.unlock_time = clock.unix_timestamp + (lock_days as i64) * 3600;
        lock_state.referral = referral;
        lock_state.initialized = true;
        lock_state.lock_days = lock_days;

        Ok(())
    }

    pub fn claim_locked_tokens(ctx: Context<ClaimLockedTokens>) -> Result<()> {
        let clock = Clock::get()?;
        let lock_state = &mut ctx.accounts.lock_state;

        require!(lock_state.initialized, CustomError::AlreadyClaimed);
        require!(
            clock.unix_timestamp >= lock_state.unlock_time,
            CustomError::LockPeriodNotOver
        );

        require!(
            ctx.accounts.program_payment_token_account.owner == ctx.accounts.pool_authority.key(),
            CustomError::InvalidOwner
        );
        let expected_program_up_usdc_account = anchor_spl::associated_token::get_associated_token_address(
            &ctx.accounts.up_pool_authority.key(),
            &ctx.accounts.metadata.up_usdc_mint,
        );
        require!(
            ctx.accounts.program_up_usdc_account.key() == expected_program_up_usdc_account,
            CustomError::InvalidProgramUpUsdcAccount
        );

        let token_amount = lock_state.amount;
        let lock_days = lock_state.lock_days;
        let config = get_lock_fee_config(lock_days);
        let liquidity_balance_raw =
            token::accessor::amount(&ctx.accounts.program_up_usdc_account.to_account_info())?
                as f64;
        let token_supply_raw = ctx.accounts.token_mint.supply.max(1) as f64;
        let liquidity_balance = liquidity_balance_raw / 1e6;
        let token_supply = token_supply_raw / 1e9;
        let token_amount_dec = token_amount as f64 / 1e9;
        let price_per_token = liquidity_balance / token_supply;
        let total_value = token_amount_dec * price_per_token;
        let total_value_scaled = total_value * 1e6;
        let founder_fee =
            ((config.founder_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;
        let team_fee = ((config.team_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;
        let liquidity_fee =
            ((config.liquidity_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;
        let user_receives =
            total_value_scaled.round() as u64 - founder_fee - team_fee - liquidity_fee;
        let vault_bump = ctx.bumps.vault_authority;
        let vault_seeds: &[&[&[u8]]] =
            &[&[b"vault", ctx.accounts.user.key.as_ref(), &[vault_bump]]];

        token::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                vault_seeds,
            ),
            token_amount,
        )?;

        let pool_bump = ctx.bumps.pool_authority;
        let pool_seeds: &[&[&[u8]]] = &[&[
            b"token_account",
            ctx.accounts.metadata.payment_token.as_ref(),
            &[pool_bump],
        ]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_payment_token_account.to_account_info(),
                    to: ctx.accounts.founder_pool_token_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                pool_seeds,
            ),
            founder_fee,
        )?;

        let pool = &mut ctx.accounts.founders_pool;
        pool.total_collected += founder_fee;

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_payment_token_account.to_account_info(),
                    to: ctx.accounts.deployer_usdc_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                pool_seeds,
            ),
            team_fee,
        )?;

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_payment_token_account.to_account_info(),
                    to: ctx.accounts.user_usdc_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                pool_seeds,
            ),
            user_receives,
        )?;

        let up_pool_bump = ctx.bumps.up_pool_authority;
        let up_mint_key = ctx.accounts.up_usdc_mint.key();
        let up_pool_signer_seeds: &[&[&[u8]]] =
            &[&[b"token_account", up_mint_key.as_ref(), &[up_pool_bump]]];

        token::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.up_usdc_mint.to_account_info(),
                    from: ctx.accounts.program_up_usdc_account.to_account_info(),
                    authority: ctx.accounts.up_pool_authority.to_account_info(),
                },
                up_pool_signer_seeds,
            ),
            user_receives + team_fee + founder_fee,
        )?;

        lock_state.initialized = false;
        lock_state.amount = 0;

        Ok(())
    }

    pub fn early_unlock_tokens(ctx: Context<EarlyUnlockTokens>) -> Result<()> {
        let lock_state = &mut ctx.accounts.lock_state;

        require!(lock_state.initialized, CustomError::AlreadyClaimed);

        require!(
            ctx.accounts.program_payment_token_account.owner == ctx.accounts.pool_authority.key(),
            CustomError::InvalidOwner
        );

        let token_amount = lock_state.amount;
        let lock_days = lock_state.lock_days;
        let mut config = get_lock_fee_config(lock_days);
        config.team_bps += 50;

        let liquidity_balance_raw =
            token::accessor::amount(&ctx.accounts.program_up_usdc_account.to_account_info())?
                as f64;
        let token_supply_raw = ctx.accounts.token_mint.supply.max(1) as f64;

        let liquidity_balance = liquidity_balance_raw / 1e6;
        let token_supply = token_supply_raw / 1e9;
        let token_amount_dec = token_amount as f64 / 1e9;

        let price_per_token = liquidity_balance / token_supply;
        let total_value = token_amount_dec * price_per_token;
        let total_value_scaled = total_value * 1e6;

        let founder_fee =
            ((config.founder_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;
        let team_fee = ((config.team_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;
        let liquidity_fee =
            ((config.liquidity_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;
        let user_receives =
            total_value_scaled.round() as u64 - founder_fee - team_fee - liquidity_fee;

        let vault_bump = ctx.bumps.vault_authority;
        let vault_seeds: &[&[&[u8]]] =
            &[&[b"vault", ctx.accounts.user.key.as_ref(), &[vault_bump]]];

        token::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                vault_seeds,
            ),
            token_amount,
        )?;

        let pool_bump = ctx.bumps.pool_authority;
        let pool_seeds: &[&[&[u8]]] = &[&[
            b"token_account",
            ctx.accounts.metadata.payment_token.as_ref(),
            &[pool_bump],
        ]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_payment_token_account.to_account_info(),
                    to: ctx.accounts.founder_pool_token_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                pool_seeds,
            ),
            founder_fee,
        )?;

        let pool = &mut ctx.accounts.founders_pool;
        pool.total_collected += founder_fee;

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_payment_token_account.to_account_info(),
                    to: ctx.accounts.deployer_usdc_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                pool_seeds,
            ),
            team_fee,
        )?;

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_payment_token_account.to_account_info(),
                    to: ctx.accounts.user_usdc_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                pool_seeds,
            ),
            user_receives,
        )?;

        let up_pool_bump = ctx.bumps.up_pool_authority;
        let up_mint_key = ctx.accounts.up_usdc_mint.key();
        let up_pool_signer_seeds: &[&[&[u8]]] =
            &[&[b"token_account", up_mint_key.as_ref(), &[up_pool_bump]]];

        token::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.up_usdc_mint.to_account_info(),
                    from: ctx.accounts.program_up_usdc_account.to_account_info(),
                    authority: ctx.accounts.up_pool_authority.to_account_info(),
                },
                up_pool_signer_seeds,
            ),
            user_receives + team_fee + founder_fee,
        )?;

        lock_state.initialized = false;
        lock_state.amount = 0;

        Ok(())
    }

    pub fn add_founder(ctx: Context<AddFounder>, new_founder: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.deployer.key() == ctx.accounts.metadata.deployer,
            CustomError::Unauthorized
        );

        let pool = &mut ctx.accounts.founders_pool;
        require!(pool.founder_count < 60, CustomError::FounderLimitReached);

        if pool.founders[..pool.founder_count as usize].contains(&new_founder) {
            return Err(CustomError::DuplicateFounder.into());
        }

        let index = pool.founder_count as usize;
        pool.founders[index] = new_founder;
        pool.claim_status[index] = 0;
        pool.founder_count += 1;

        Ok(())
    }

    pub fn claim_founder_share(ctx: Context<ClaimFounderShare>) -> Result<()> {
        let pool = &mut ctx.accounts.founders_pool;
        let founder_key = ctx.accounts.founder.key();
        let mut index = None;

        for (i, f) in pool.founders.iter().enumerate() {
            if *f == founder_key {
                index = Some(i);
                break;
            }
        }

        let idx = index.ok_or(CustomError::NotFounder)?;
        let total_per_founder = pool.total_collected / 60;
        let already_claimed = pool.claim_status[idx];
        let claimable = total_per_founder.saturating_sub(already_claimed);

        require!(claimable > 0, CustomError::NothingToClaim);

        pool.claim_status[idx] += claimable;

        require!(
            ctx.accounts.founder_token_account.mint == ctx.accounts.founder_pool_token_account.mint,
            CustomError::InvalidTokenMint
        );

        let bump = ctx.bumps.founder_authority;
        let signer_seeds: &[&[&[u8]]] = &[&[b"founder_authority".as_ref(), &[bump]]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.founder_pool_token_account.to_account_info(),
                    to: ctx.accounts.founder_token_account.to_account_info(),
                    authority: ctx.accounts.founder_authority.to_account_info(),
                },
                signer_seeds,
            ),
            claimable,
        )?;

        Ok(())
    }

    pub fn leverage_buy(
        ctx: Context<LeverageBuy>,
        amount: u64,
        leverage_multiplier: u64,
        lock_days: u64,
        referral: Option<Pubkey>,
    ) -> Result<()> {
        require!(
            matches!(leverage_multiplier, 1..=5),
            CustomError::InvalidLeverageMultiplier
        );
        let clock = Clock::get()?;
        let leverage_position = &mut ctx.accounts.leverage_position;
        require!(
            !leverage_position.initialized,
            CustomError::AlreadyInitialized
        );

        require!(
            matches!(lock_days, 1 | 2 | 3 | 4 | 6 | 8 | 12),
            CustomError::InvalidLockPeriod
        );

        require!(ctx.accounts.metadata.initialized, CustomError::AlreadyInitialized);
        
        let payment_token_mint = ctx.accounts.metadata.payment_token;
        let up_usdc_mint = ctx.accounts.metadata.up_usdc_mint;

        let expected_deployer_usdc_account = anchor_spl::associated_token::get_associated_token_address(
            &ctx.accounts.metadata.team,
            &payment_token_mint,
        );
        require!(
            ctx.accounts.deployer_usdc_account.key() == expected_deployer_usdc_account,
            CustomError::InvalidDeployerUsdcAccount
        );

        let expected_program_payment_token_account = anchor_spl::associated_token::get_associated_token_address(
            &ctx.accounts.pool_authority.key(),
            &payment_token_mint,
        );
        require!(
            ctx.accounts.program_payment_token_account.key() == expected_program_payment_token_account,
            CustomError::InvalidProgramPaymentTokenAccount
        );

        let expected_founder_pool_token_account = anchor_spl::associated_token::get_associated_token_address(
            &ctx.accounts.founder_authority.key(),
            &payment_token_mint,
        );
        require!(
            ctx.accounts.founder_pool_token_account.key() == expected_founder_pool_token_account,
            CustomError::InvalidFounderPoolTokenAccount
        );

        let expected_program_up_usdc_account = anchor_spl::associated_token::get_associated_token_address(
            &ctx.accounts.up_pool_authority.key(),
            &up_usdc_mint,
        );
        require!(
            ctx.accounts.program_up_usdc_account.key() == expected_program_up_usdc_account,
            CustomError::InvalidProgramUpUsdcAccount
        );


        let total_usdc = amount
            .checked_mul(leverage_multiplier as u64)
            .ok_or(ProgramError::InvalidArgument)?;

        let borrow_amount = total_usdc - amount;

        let config = get_lock_fee_config(lock_days);

        let team_share = total_usdc * config.team_bps / 10_000;
        let founder_fee = total_usdc * config.founder_bps / 10_000;
        let locked_share = total_usdc * config.liquidity_bps / 10_000;
        let usdc_for_tokens = total_usdc - team_share - founder_fee - locked_share;
        let user_amount_after_fees =
            total_usdc - borrow_amount - team_share - founder_fee - locked_share;
        let liquidity_balance =
            token::accessor::amount(&ctx.accounts.program_up_usdc_account.to_account_info())?;

        let token_supply = ctx.accounts.token_mint.supply;

        let mut price_start =
            (liquidity_balance as u128) * 1_000_000_000 / (token_supply.max(1) as u128);
        if price_start == 0 {
            price_start = 1;
        }
        let estimated_tokens = (usdc_for_tokens as u128) * 1_000_000_000 / price_start;
        let liquidity_growth =
            (liquidity_balance as u128) + (usdc_for_tokens as u128) + (locked_share as u128);
        let price_end =
            liquidity_growth * 1_000_000_000 / (((token_supply as u128) + estimated_tokens).max(1));
        let avg_price = (price_start + price_end) / 2;
        let mintable_tokens = ((usdc_for_tokens as u128) * 1_000_000_000 / avg_price) as u64;

        require!(mintable_tokens > 0, CustomError::InsufficientAmount);

        if let Some(ref_pubkey) = referral {
            let referral_token_account = ctx
                .accounts
                .referral_usdc_account
                .as_ref()
                .ok_or(CustomError::MissingReferralAccount)?;
            require!(
                referral_token_account.owner == ref_pubkey,
                CustomError::InvalidReferral
            );

            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user_usdc_account.to_account_info(),
                        to: referral_token_account.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                team_share / 2,
            )?;

            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user_usdc_account.to_account_info(),
                        to: ctx.accounts.deployer_usdc_account.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                team_share / 2,
            )?;
        } else {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user_usdc_account.to_account_info(),
                        to: ctx.accounts.deployer_usdc_account.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                team_share,
            )?;
        }

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.user_usdc_account.to_account_info(),
                    to: ctx.accounts.founder_pool_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            founder_fee,
        )?;
        let pool = &mut ctx.accounts.founders_pool;
        pool.total_collected += founder_fee;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_usdc_account.to_account_info(),
                    to: ctx.accounts.program_payment_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            user_amount_after_fees + locked_share,
        )?;

        let up_usdc_mint_bump = ctx.bumps.up_usdc_mint_authority;
        let up_usdc_signer_seeds: &[&[&[u8]]] =
            &[&[b"up_usdc_mint_authority", &[up_usdc_mint_bump]]];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.up_usdc_mint.to_account_info(),
                    to: ctx.accounts.program_up_usdc_account.to_account_info(),
                    authority: ctx.accounts.up_usdc_mint_authority.to_account_info(),
                },
                up_usdc_signer_seeds,
            ),
            usdc_for_tokens + locked_share,
        )?;

        let mint_bump = ctx.bumps.mint_authority;
        let signer_seeds: &[&[&[u8]]] = &[&[b"mint_authority", &[mint_bump]]];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.mint_authority.to_account_info(),
                },
                signer_seeds,
            ),
            mintable_tokens,
        )?;

        leverage_position.user = ctx.accounts.user.key();
        leverage_position.amount_user_paid = amount;
        leverage_position.amount_borrowed = borrow_amount;
        leverage_position.unlock_time = clock.unix_timestamp + (lock_days as i64) * 3600;
        leverage_position.referral = referral;
        leverage_position.initialized = true;
        leverage_position.lock_days = lock_days;
        leverage_position.amount_minted = mintable_tokens;

        Ok(())
    }

    pub fn leverage_sell(ctx: Context<LeverageSell>) -> Result<()> {
        let clock = Clock::get()?;
        let position = &mut ctx.accounts.leverage_position;
        require!(position.initialized, CustomError::AlreadyClaimed);
        require!(
            clock.unix_timestamp >= position.unlock_time,
            CustomError::LockPeriodNotOver
        );

        require!(
            ctx.accounts.program_payment_token_account.owner == ctx.accounts.pool_authority.key(),
            CustomError::InvalidOwner
        );
       
        let amount_minted = position.amount_minted;
        let liquidity_balance_raw =
            token::accessor::amount(&ctx.accounts.program_up_usdc_account.to_account_info())?
                as f64;
        let token_supply_raw = ctx.accounts.token_mint.supply.max(1) as f64;
        let liquidity_balance = liquidity_balance_raw / 1e6;
        let token_supply = token_supply_raw / 1e9;
        let price_per_token = liquidity_balance / token_supply;
        let token_amount_dec = amount_minted as f64 / 1e9;
        let total_value = token_amount_dec * price_per_token;
        let total_value_scaled = total_value * 1e6;
        let borrowed = position.amount_borrowed;

        let lock_days = position.lock_days;
        let config = get_lock_fee_config(lock_days);

        let founder_fee =
            ((config.founder_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;
        let team_fee = ((config.team_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;
        let liquidity_fee =
            ((config.liquidity_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;
        let user_cut = total_value_scaled.round() as u64
            - borrowed as u64
            - founder_fee
            - team_fee
            - liquidity_fee;

        let vault_bump = ctx.bumps.vault_authority;
        let vault_seeds: &[&[&[u8]]] =
            &[&[b"l_vault", ctx.accounts.user.key.as_ref(), &[vault_bump]]];
        token::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                vault_seeds,
            ),
            amount_minted,
        )?;

        let up_pool_bump = ctx.bumps.up_pool_authority;
        let up_mint_key = ctx.accounts.up_usdc_mint.key();
        let up_pool_signer_seeds: &[&[&[u8]]] =
            &[&[b"token_account", up_mint_key.as_ref(), &[up_pool_bump]]];

        token::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Burn {
                    mint: ctx.accounts.up_usdc_mint.to_account_info(),
                    from: ctx.accounts.program_up_usdc_account.to_account_info(),
                    authority: ctx.accounts.up_pool_authority.to_account_info(),
                },
                up_pool_signer_seeds,
            ),
            user_cut + team_fee + founder_fee + borrowed as u64,
        )?;

        let pool_bump = ctx.bumps.pool_authority;
        let pool_seeds: &[&[&[u8]]] = &[&[
            b"token_account",
            ctx.accounts.metadata.payment_token.as_ref(),
            &[pool_bump],
        ]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_payment_token_account.to_account_info(),
                    to: ctx.accounts.founder_pool_token_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                pool_seeds,
            ),
            founder_fee,
        )?;

        let pool = &mut ctx.accounts.founders_pool;
        pool.total_collected += founder_fee;

        if let Some(ref_pubkey) = position.referral {
            let referral_token_account = ctx
                .accounts
                .referral_usdc_account
                .as_ref()
                .ok_or(CustomError::MissingReferralAccount)?;
            require!(
                referral_token_account.owner == ref_pubkey,
                CustomError::InvalidReferral
            );

            let referral_share = team_fee / 2;
            let deployer_share = team_fee - referral_share;

            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_payment_token_account.to_account_info(),
                        to: referral_token_account.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    pool_seeds,
                ),
                referral_share,
            )?;

            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_payment_token_account.to_account_info(),
                        to: ctx.accounts.deployer_usdc_account.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    pool_seeds,
                ),
                deployer_share,
            )?;
        } else {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_payment_token_account.to_account_info(),
                        to: ctx.accounts.deployer_usdc_account.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    pool_seeds,
                ),
                team_fee,
            )?;
        }

        if user_cut > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_payment_token_account.to_account_info(),
                        to: ctx.accounts.user_usdc_account.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    pool_seeds,
                ),
                user_cut,
            )?;
        }

        position.initialized = false;
        position.amount_minted = 0;
        position.amount_borrowed = 0;
        position.amount_user_paid = 0;

        Ok(())
    }

    pub fn early_close_leverage(ctx: Context<EarlyCloseLeverage>) -> Result<()> {
        let leverage_position = &mut ctx.accounts.leverage_position;
        require!(leverage_position.initialized, CustomError::AlreadyClaimed);

        require!(
            ctx.accounts.program_payment_token_account.owner == ctx.accounts.pool_authority.key(),
            CustomError::InvalidOwner
        );

        let expected_program_up_usdc_account = anchor_spl::associated_token::get_associated_token_address(
            &ctx.accounts.up_pool_authority.key(),
            &ctx.accounts.metadata.up_usdc_mint,
        );
        require!(
            ctx.accounts.program_up_usdc_account.key() == expected_program_up_usdc_account,
            CustomError::InvalidProgramUpUsdcAccount
        );

        let amount_minted = leverage_position.amount_minted;
        let lock_days = leverage_position.lock_days;
        let mut config = get_lock_fee_config(lock_days);
        config.team_bps += 50; // Add 0.5% penalty

        let borrowed = leverage_position.amount_borrowed;

        let liquidity_balance_raw =
            token::accessor::amount(&ctx.accounts.program_up_usdc_account.to_account_info())?
                as f64;
        let token_supply_raw = ctx.accounts.token_mint.supply.max(1) as f64;

        let liquidity_balance = liquidity_balance_raw / 1e6;
        let token_supply = token_supply_raw / 1e9;
        let token_amount_dec = amount_minted as f64 / 1e9;

        let price_per_token = liquidity_balance / token_supply;
        let total_value = token_amount_dec * price_per_token;
        let total_value_scaled = total_value * 1e6;

        let founder_fee =
            ((config.founder_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;
        let team_fee = ((config.team_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;
        let liquidity_fee =
            ((config.liquidity_bps as f64 / 10_000.0) * total_value_scaled).round() as u64;

        let user_cut = total_value_scaled.round() as u64
            - borrowed as u64
            - founder_fee
            - team_fee
            - liquidity_fee;

        let vault_bump = ctx.bumps.vault_authority;
        let vault_seeds: &[&[&[u8]]] =
            &[&[b"l_vault", ctx.accounts.user.key.as_ref(), &[vault_bump]]];
        token::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                vault_seeds,
            ),
            amount_minted,
        )?;

        let up_pool_bump = ctx.bumps.up_pool_authority;
        let up_mint_key = ctx.accounts.up_usdc_mint.key();
        let up_pool_signer_seeds: &[&[&[u8]]] =
            &[&[b"token_account", up_mint_key.as_ref(), &[up_pool_bump]]];

        token::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Burn {
                    mint: ctx.accounts.up_usdc_mint.to_account_info(),
                    from: ctx.accounts.program_up_usdc_account.to_account_info(),
                    authority: ctx.accounts.up_pool_authority.to_account_info(),
                },
                up_pool_signer_seeds,
            ),
            user_cut + team_fee + founder_fee + borrowed as u64,
        )?;

        let pool_bump = ctx.bumps.pool_authority;
        let pool_seeds: &[&[&[u8]]] = &[&[
            b"token_account",
            ctx.accounts.metadata.payment_token.as_ref(),
            &[pool_bump],
        ]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_payment_token_account.to_account_info(),
                    to: ctx.accounts.founder_pool_token_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                pool_seeds,
            ),
            founder_fee,
        )?;

        let pool = &mut ctx.accounts.founders_pool;
        pool.total_collected += founder_fee;

        if let Some(ref_pubkey) = leverage_position.referral {
            let referral_token_account = ctx
                .accounts
                .referral_usdc_account
                .as_ref()
                .ok_or(CustomError::MissingReferralAccount)?;
            require!(
                referral_token_account.owner == ref_pubkey,
                CustomError::InvalidReferral
            );

            let referral_share = team_fee / 2;
            let deployer_share = team_fee - referral_share;

            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_payment_token_account.to_account_info(),
                        to: referral_token_account.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    pool_seeds,
                ),
                referral_share,
            )?;

            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_payment_token_account.to_account_info(),
                        to: ctx.accounts.deployer_usdc_account.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    pool_seeds,
                ),
                deployer_share,
            )?;
        } else {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_payment_token_account.to_account_info(),
                        to: ctx.accounts.deployer_usdc_account.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    pool_seeds,
                ),
                team_fee,
            )?;
        }

        if user_cut > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_payment_token_account.to_account_info(),
                        to: ctx.accounts.user_usdc_account.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    pool_seeds,
                ),
                user_cut,
            )?;
        }

        leverage_position.initialized = false;
        leverage_position.amount_minted = 0;
        leverage_position.amount_borrowed = 0;
        leverage_position.amount_user_paid = 0;

        Ok(())
    }
}

pub fn get_lock_fee_config(lock_days: u64) -> LockFeeConfig {
    match lock_days {
        1 => LockFeeConfig {
            liquidity_bps: 150,
            team_bps: 75,
            founder_bps: 25,
        },
        2 => LockFeeConfig {
            liquidity_bps: 225,
            team_bps: 100,
            founder_bps: 25,
        },
        3 => LockFeeConfig {
            liquidity_bps: 300,
            team_bps: 125,
            founder_bps: 25,
        },
        4 => LockFeeConfig {
            liquidity_bps: 375,
            team_bps: 150,
            founder_bps: 25,
        },
        6 => LockFeeConfig {
            liquidity_bps: 450,
            team_bps: 175,
            founder_bps: 25,
        },
        8 => LockFeeConfig {
            liquidity_bps: 550,
            team_bps: 200,
            founder_bps: 25,
        },
        12 => LockFeeConfig {
            liquidity_bps: 725,
            team_bps: 250,
            founder_bps: 25,
        },
        _ => LockFeeConfig {
            liquidity_bps: 925,
            team_bps: 300,
            founder_bps: 25,
        },
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub up_only_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        space = 8 + 4 + 6 + 4 + 2 + 160 + 32 + 1,
        seeds = [b"metadata", up_only_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(
        mut,
        constraint = user_up_only_account.mint == up_only_mint.key()
    )]
    pub user_up_only_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = program_up_only_account.mint == up_only_mint.key()
    )]
    pub program_up_only_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub payment_token_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = user_payment_token_account.mint == payment_token_mint.key()
    )]
    pub user_payment_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = program_payment_token_account.mint == payment_token_mint.key()
    )]
    pub program_payment_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub up_usdc_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        constraint = user_up_usdc_account.mint == up_usdc_mint.key()
    )]
    pub user_up_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = program_up_usdc_account.mint == up_usdc_mint.key()
    )]
    pub program_up_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        seeds = [b"mint_authority"],
        bump
    )]
    /// CHECK: This PDA is derived within the program and only used as a signer; it's safe.
    pub mint_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [b"up_usdc_mint_authority"],
        bump
    )]
    /// CHECK: This PDA is derived within the program and only used as a signer; it's safe.
    pub up_usdc_mint_authority: UncheckedAccount<'info>,

    pub current_mint_authority: Signer<'info>,
    pub current_up_usdc_authority: Signer<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init_if_needed,
        payer = authority,
        space = 8 + 1,
        seeds = [b"global_state"],
        bump
    )]
    pub global_state: Account<'info, GlobalState>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct InitializeFoundersPool<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 8 + 1924 + 484 + 1,
        seeds = [b"founders_pool"],
        bump
    )]
    pub founders_pool: Account<'info, FoundersPool>,

    /// CHECK: Just a PDA, no need for data validation
    #[account(
        seeds = [b"founder_authority"],
        bump
    )]
    pub founder_authority: UncheckedAccount<'info>,

    ///CHECK: PDA that owns the token account
    #[account(mut)]
    pub founder_pool_token_account: AccountInfo<'info>,

    pub usdc_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"metadata", token_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct ClaimFounderShare<'info> {
    #[account(mut)]
    pub founder: Signer<'info>,

    #[account(mut, seeds = [b"founders_pool"], bump)]
    pub founders_pool: Account<'info, FoundersPool>,

    #[account(
        mut,
        constraint = founder_token_account.owner == founder.key(),
        constraint = founder_token_account.mint == founder_pool_token_account.mint
    )]
    pub founder_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub founder_pool_token_account: Account<'info, TokenAccount>,

    /// CHECK: signer PDA
    #[account(seeds = [b"founder_authority"], bump)]
    pub founder_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BuyAndLockToken<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(init_if_needed, payer = user, space = 8 + 32 + 8 + 8 + 33 + 1 + 8, seeds = [b"locked", user.key().as_ref()], bump)]
    pub lock_state: Account<'info, LockedTokenState>,

    #[account(mut)]
    pub user_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub deployer_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub program_payment_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"mint_authority"],
        bump
    )]
    /// CHECK: only used as signer
    pub mint_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [b"metadata", token_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = vault_authority
    )]
    /// CHECK: ATA for vault
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(seeds = [b"vault", user.key().as_ref()], bump)]
    /// CHECK: Vault PDA signer
    pub vault_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub referral_usdc_account: Option<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,

    #[account(mut, seeds = [b"founders_pool"], bump)]
    pub founders_pool: Account<'info, FoundersPool>,

    #[account(
        seeds = [b"founder_authority"],
        bump
    )]
    /// CHECK: PDA used as authority/owner of founder pool ATA
    pub founder_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub founder_pool_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: PDA that owns program_payment_token_account
    /// Seeds will be validated in function body after metadata is loaded
    pub pool_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub user_up_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = program_up_usdc_account.mint == metadata.up_usdc_mint,
        constraint = program_up_usdc_account.key() == anchor_spl::associated_token::get_associated_token_address(&up_pool_authority.key(), &metadata.up_usdc_mint)
    )]
    pub program_up_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub up_usdc_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"up_usdc_mint_authority"],
        bump
    )]
    /// CHECK: only used as signer
    pub up_usdc_mint_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [b"token_account", up_usdc_mint.key().as_ref()],
        bump
    )]
    /// CHECK: PDA that owns program_up_usdc_account
    pub up_pool_authority: UncheckedAccount<'info>,

    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct LeverageBuy<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 32 + 8 + 8 + 8 + 33 + 1 + 8 + 8,
        seeds = [b"leverage", user.key().as_ref()],
        bump
    )]
    pub leverage_position: Account<'info, LeveragePosition>,

    #[account(
        mut,
        constraint = user_usdc_account.mint == metadata.payment_token
    )]
    pub user_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = deployer_usdc_account.mint == metadata.payment_token
    )]
    pub deployer_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = program_payment_token_account.mint == metadata.payment_token
    )]
    pub program_payment_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"mint_authority"],
        bump
    )]
    /// CHECK: only used as signer
    pub mint_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [b"metadata", token_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = vault_authority,
        constraint = vault_token_account.mint == metadata.mint
    )]
    /// CHECK: ATA for vault
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(seeds = [b"l_vault", user.key().as_ref()], bump)]
    /// CHECK: Vault PDA signer
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = referral_usdc_account.key() == anchor_lang::solana_program::system_program::ID || referral_usdc_account.mint == metadata.payment_token
    )]
    pub referral_usdc_account: Option<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,

    #[account(mut, seeds = [b"founders_pool"], bump)]
    pub founders_pool: Account<'info, FoundersPool>,

    #[account(
        seeds = [b"founder_authority"],
        bump
    )]
    /// CHECK: PDA used as authority/owner of founder pool ATA
    pub founder_authority: UncheckedAccount<'info>,
    #[account(
        mut,
        constraint = founder_pool_token_account.mint == metadata.payment_token
    )]
    pub founder_pool_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        seeds = [b"token_account", metadata.payment_token.as_ref()],
        bump
    )]
    /// CHECK: PDA that owns program_payment_token_account
    pub pool_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = user_up_usdc_account.mint == metadata.up_usdc_mint
    )]
    pub user_up_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = program_up_usdc_account.mint == metadata.up_usdc_mint,
        constraint = program_up_usdc_account.key() == anchor_spl::associated_token::get_associated_token_address(&up_pool_authority.key(), &metadata.up_usdc_mint)
    )]
    pub program_up_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = up_usdc_mint.key() == metadata.up_usdc_mint
    )]
    pub up_usdc_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"up_usdc_mint_authority"],
        bump
    )]
    /// CHECK: only used as signer
    pub up_usdc_mint_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [b"token_account", up_usdc_mint.key().as_ref()],
        bump
    )]
    /// CHECK: PDA that owns program_up_usdc_account
    pub up_pool_authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct LeverageSell<'info> {
    pub cranker: Signer<'info>,

    #[account(
        seeds = [b"token_account", metadata.payment_token.as_ref()],
        bump
    )]
    /// CHECK: signer for transferring from program_payment_token_account
    pub pool_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [b"metadata", token_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(mut)]
    ///CHECK: Used to derive vault PDA
    pub user: UncheckedAccount<'info>,

    #[account(mut, seeds = [b"leverage", user.key().as_ref()], bump)]
    pub leverage_position: Account<'info, LeveragePosition>,

    #[account(
        mut,
        seeds = [b"l_vault", user.key().as_ref()],
        bump
    )]
    /// CHECK: Only used as signer
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = vault_authority,
        constraint = vault_token_account.mint == metadata.mint
    )]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_usdc_account.mint == metadata.payment_token
    )]
    pub user_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = deployer_usdc_account.mint == metadata.payment_token,
        constraint = deployer_usdc_account.key() == anchor_spl::associated_token::get_associated_token_address(&metadata.team, &metadata.payment_token)
    )]
    pub deployer_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = program_payment_token_account.mint == metadata.payment_token
    )]
    pub program_payment_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = founder_pool_token_account.mint == metadata.payment_token,
        constraint = founder_pool_token_account.key() == anchor_spl::associated_token::get_associated_token_address(&founder_authority.key(), &metadata.payment_token)
    )]
    pub founder_pool_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut, seeds = [b"founders_pool"], bump)]
    pub founders_pool: Account<'info, FoundersPool>,

    #[account(
        seeds = [b"founder_authority"],
        bump
    )]
    /// CHECK: PDA used as authority/owner of founder pool ATA
    pub founder_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    #[account(
        mut,
        constraint = user_up_usdc_account.mint == metadata.up_usdc_mint
    )]
    pub user_up_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = program_up_usdc_account.mint == metadata.up_usdc_mint,
        constraint = program_up_usdc_account.key() == anchor_spl::associated_token::get_associated_token_address(&up_pool_authority.key(), &metadata.up_usdc_mint)
    )]
    pub program_up_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = up_usdc_mint.key() == metadata.up_usdc_mint
    )]
    pub up_usdc_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"token_account", up_usdc_mint.key().as_ref()],
        bump
    )]
    /// CHECK: PDA used as signer for burning from program_up_usdc_account
    pub up_pool_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = referral_usdc_account.key() == anchor_lang::solana_program::system_program::ID || referral_usdc_account.mint == metadata.payment_token
    )]
    pub referral_usdc_account: Option<Account<'info, TokenAccount>>,
}

#[derive(Accounts)]
pub struct EarlyCloseLeverage<'info> {
    pub cranker: Signer<'info>,

    #[account(
        seeds = [b"token_account", metadata.payment_token.as_ref()],
        bump
    )]
    /// CHECK: signer for transferring from program_payment_token_account
    pub pool_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [b"metadata", token_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(mut)]
    ///CHECK: Used to derive vault PDA
    pub user: UncheckedAccount<'info>,

    #[account(mut, seeds = [b"leverage", user.key().as_ref()], bump)]
    pub leverage_position: Account<'info, LeveragePosition>,

    #[account(
        mut,
        seeds = [b"l_vault", user.key().as_ref()],
        bump
    )]
    /// CHECK: Only used as signer
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = vault_authority,
        constraint = vault_token_account.mint == metadata.mint
    )]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_usdc_account.mint == metadata.payment_token
    )]
    pub user_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = deployer_usdc_account.mint == metadata.payment_token,
        constraint = deployer_usdc_account.key() == anchor_spl::associated_token::get_associated_token_address(&metadata.team, &metadata.payment_token)
    )]
    pub deployer_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = program_payment_token_account.mint == metadata.payment_token
    )]
    pub program_payment_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = founder_pool_token_account.mint == metadata.payment_token,
        constraint = founder_pool_token_account.key() == anchor_spl::associated_token::get_associated_token_address(&founder_authority.key(), &metadata.payment_token)
    )]
    pub founder_pool_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut, seeds = [b"founders_pool"], bump)]
    pub founders_pool: Account<'info, FoundersPool>,

    #[account(
        seeds = [b"founder_authority"],
        bump
    )]
    /// CHECK: PDA used as authority/owner of founder pool ATA
    pub founder_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    #[account(
        mut,
        constraint = program_up_usdc_account.mint == metadata.up_usdc_mint,
        constraint = program_up_usdc_account.key() == anchor_spl::associated_token::get_associated_token_address(&up_pool_authority.key(), &metadata.up_usdc_mint)
    )]
    pub program_up_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = up_usdc_mint.key() == metadata.up_usdc_mint
    )]
    pub up_usdc_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"token_account", up_usdc_mint.key().as_ref()],
        bump
    )]
    /// CHECK: PDA used as signer for burning from program_up_usdc_account
    pub up_pool_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = referral_usdc_account.key() == anchor_lang::solana_program::system_program::ID || referral_usdc_account.mint == metadata.payment_token
    )]
    pub referral_usdc_account: Option<Account<'info, TokenAccount>>,
}

#[derive(Accounts)]
pub struct ClaimLockedTokens<'info> {
    pub cranker: Signer<'info>,

    #[account(
        seeds = [b"token_account", metadata.payment_token.as_ref()],
        bump
    )]
    /// CHECK: signer for transferring from program_payment_token_account
    pub pool_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [b"metadata", token_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(mut)]
    ///CHECK: Used to derive vault PDA
    pub user: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"locked", user.key().as_ref()],
        bump
    )]
    pub lock_state: Account<'info, LockedTokenState>,

    #[account(
        mut,
        seeds = [b"vault", user.key().as_ref()],
        bump
    )]
    /// CHECK: Only used as signer
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = vault_authority,
        constraint = vault_token_account.mint == metadata.mint
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_usdc_account.mint == metadata.payment_token,
        associated_token::mint = metadata.payment_token,
        associated_token::authority = user
    )]
    pub user_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = deployer_usdc_account.mint == metadata.payment_token,
        constraint = deployer_usdc_account.key() == anchor_spl::associated_token::get_associated_token_address(&metadata.team, &metadata.payment_token)
    )]
    pub deployer_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = program_payment_token_account.mint == metadata.payment_token
    )]
    pub program_payment_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = founder_pool_token_account.mint == metadata.payment_token,
        constraint = founder_pool_token_account.key() == anchor_spl::associated_token::get_associated_token_address(&founder_authority.key(), &metadata.payment_token)
    )]
    pub founder_pool_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut, seeds = [b"founders_pool"], bump)]
    pub founders_pool: Account<'info, FoundersPool>,

    #[account(
        seeds = [b"founder_authority"],
        bump
    )]
    /// CHECK: PDA used as authority/owner of founder pool ATA
    pub founder_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    #[account(
        mut,
        constraint = program_up_usdc_account.mint == metadata.up_usdc_mint,
        constraint = program_up_usdc_account.key() == anchor_spl::associated_token::get_associated_token_address(&up_pool_authority.key(), &metadata.up_usdc_mint)
    )]
    pub program_up_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = up_usdc_mint.key() == metadata.up_usdc_mint
    )]
    pub up_usdc_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"token_account", up_usdc_mint.key().as_ref()],
        bump
    )]
    /// CHECK: signer for burning from program_up_usdc_account
   pub up_pool_authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct EarlyUnlockTokens<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"locked", user.key().as_ref()],
        bump
    )]
    pub lock_state: Account<'info, LockedTokenState>,

    #[account(
        mut,
        seeds = [b"vault", user.key().as_ref()],
        bump
    )]
    /// CHECK: signer
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = vault_authority,
        constraint = vault_token_account.mint == metadata.mint
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_usdc_account.mint == metadata.payment_token
    )]
    pub user_usdc_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = deployer_usdc_account.mint == metadata.payment_token,
        constraint = deployer_usdc_account.key() == anchor_spl::associated_token::get_associated_token_address(&metadata.team, &metadata.payment_token)
    )]
    pub deployer_usdc_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = program_payment_token_account.mint == metadata.payment_token
    )]
    pub program_payment_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(
        seeds = [b"token_account", metadata.payment_token.as_ref()],
        bump
    )]
    /// CHECK
    pub pool_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = founder_pool_token_account.mint == metadata.payment_token,
        constraint = founder_pool_token_account.key() == anchor_spl::associated_token::get_associated_token_address(&founder_authority.key(), &metadata.payment_token)
    )]
    pub founder_pool_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut, seeds = [b"founders_pool"], bump)]
    pub founders_pool: Account<'info, FoundersPool>,

    #[account(
        seeds = [b"founder_authority"],
        bump
    )]
    /// CHECK: PDA used as authority/owner of founder pool ATA
    pub founder_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,

    #[account(
        mut,
        constraint = program_up_usdc_account.mint == metadata.up_usdc_mint,
        constraint = program_up_usdc_account.key() == anchor_spl::associated_token::get_associated_token_address(&up_pool_authority.key(), &metadata.up_usdc_mint)
    )]
    pub program_up_usdc_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = up_usdc_mint.key() == metadata.up_usdc_mint
    )]
    pub up_usdc_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"token_account", up_usdc_mint.key().as_ref()],
        bump
    )]
    /// CHECK: signer for burning from program_up_usdc_account
    pub up_pool_authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct AddFounder<'info> {
    #[account(mut, has_one = deployer)]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(mut, seeds = [b"founders_pool"], bump)]
    pub founders_pool: Account<'info, FoundersPool>,

    pub deployer: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializeUserVault<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: Only used as a derived signer authority
    #[account(seeds = [b"vault", user.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = vault_authority
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        constraint = token_mint.key() == metadata.mint
    )]
    pub token_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"metadata", token_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct InitializeLeverageUserVault<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: Only used as a derived signer authority
    #[account(seeds = [b"l_vault", user.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = vault_authority
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        constraint = token_mint.key() == metadata.mint
    )]
    pub token_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"metadata", token_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[account]
pub struct GlobalState {
    pub initialized: bool,
}

#[account]
pub struct UserState {
    pub referral: Pubkey,
    pub referral_set: bool,
}

#[account]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub mint: Pubkey,
    pub authority: Pubkey,
    pub payment_token: Pubkey,
    pub up_usdc_mint: Pubkey,
    pub deployer: Pubkey,
    pub team: Pubkey,
    pub initialized: bool,
}

#[account]
pub struct LockedTokenState {
    pub user: Pubkey,
    pub amount: u64,
    pub unlock_time: i64,
    pub referral: Option<Pubkey>,
    pub initialized: bool,
    pub lock_days: u64,
}

#[account]
pub struct LeveragePosition {
    pub user: Pubkey,
    pub amount_user_paid: u64,
    pub amount_borrowed: u64,
    pub unlock_time: i64,
    pub referral: Option<Pubkey>,
    pub initialized: bool,
    pub lock_days: u64,
    pub amount_minted: u64,
}
#[account]
pub struct FoundersPool {
    pub total_collected: u64,
    pub founders: Vec<Pubkey>,
    pub claim_status: Vec<u64>,
    pub founder_count: u8,
}

#[account]
pub struct LockFeeConfig {
    pub liquidity_bps: u64,
    pub team_bps: u64,
    pub founder_bps: u64,
}

#[error_code]
pub enum CustomError {
    #[msg("Token mint is already initialized")]
    AlreadyInitialized,

    #[msg("Referral cannot be the user themselves")]
    InvalidReferral,

    #[msg("Referral token account must be provided")]
    MissingReferralAccount,

    #[msg("Deployer token account must be provided")]
    MissingDeployerAccount,

    #[msg("Deployer token account does not match metadata")]
    InvalidDeployerAccount,

    #[msg("Maximum number of founders reached")]
    FounderLimitReached,

    #[msg("Caller is not a founder")]
    NotFounder,

    #[msg("Nothing to claim")]
    NothingToClaim,

    #[msg("You are not authorized to perform this action.")]
    Unauthorized,

    #[msg("Lock period has not ended")]
    LockPeriodNotOver,

    #[msg("Tokens already claimed")]
    AlreadyClaimed,

    #[msg("Invalid lock period")]
    InvalidLockPeriod,

    #[msg("Invalid leverage multiplier")]
    InvalidLeverageMultiplier,

    #[msg("Duplicate founder")]
    DuplicateFounder,

    #[msg("Insufficient amount to mint tokens")]
    InsufficientAmount,

    #[msg("Invalid token mint")]
    InvalidTokenMint,

    #[msg("Invalid token account owner")]
    InvalidOwner,

    #[msg("Invalid user USDC account owner")]
    InvalidUserUsdcAccount,

    #[msg("Invalid deployer USDC account owner")]
    InvalidDeployerUsdcAccount,

    #[msg("Invalid program payment token account owner")]
    InvalidProgramPaymentTokenAccount,

    #[msg("Invalid founder pool token account owner")]
    InvalidFounderPoolTokenAccount,

    #[msg("Invalid program upUSDC account owner")]
    InvalidProgramUpUsdcAccount,
}

