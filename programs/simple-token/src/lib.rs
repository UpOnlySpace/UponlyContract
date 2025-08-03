use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::spl_token::instruction::AuthorityType;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer};

declare_id!("Gj5uHr66YFdJLKx6yY4hasp8XzG4tkGLBK43GshhN4BM");

#[program]
pub mod up_only {
    use super::*;

    const TEAM_FEE_BPS: u64 = 300; // 3%
    const FOUNDER_FEE_BPS: u64 = 25; // 0.25%
    const LOCKED_LIQUIDITY_BPS: u64 = 925; // 9.25%

    pub fn initialize(ctx: Context<Initialize>, team: Pubkey) -> Result<()> {
        if ctx.accounts.metadata.initialized {
            return Err(CustomError::AlreadyInitialized.into());
        }

        let (mint_authority, _) =
            Pubkey::find_program_address(&[b"mint_authority"], ctx.program_id);

        let metadata = &mut ctx.accounts.metadata;
        metadata.name = "UpOnly".to_string();
        metadata.symbol = "UP".to_string();
        metadata.mint = ctx.accounts.up_only_mint.key();
        metadata.authority = mint_authority;
        metadata.payment_token = ctx.accounts.payment_token_mint.key();
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
                from: ctx.accounts.user_up_only_account.to_account_info(),
                to: ctx.accounts.program_up_only_account.to_account_info(),
                authority: ctx.accounts.authority.to_account_info(),
            },
        );
        token::transfer(cpi_context, 1_000_000_000)?;

        let mint_authority_bump = ctx.bumps.mint_authority;
        let signer_seeds: &[&[&[u8]]] = &[&[b"mint_authority", &[mint_authority_bump]]];

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

        Ok(())
    }

    pub fn initialize_founders_pool(ctx: Context<InitializeFoundersPool>) -> Result<()> {
        let pool = &mut ctx.accounts.founders_pool;
        pool.total_collected = 0;
        pool.founder_count = 0;
        pool.founders = vec![Pubkey::default(); 60];
        pool.claim_status = vec![0u64; 60];

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

        Ok(())
    }

    pub fn buy_pass(ctx: Context<BuyPass>, referral: Option<Pubkey>) -> Result<()> {
        let user_state = &mut ctx.accounts.user_state;
        require!(!user_state.has_pass, CustomError::AlreadyHasPass);

        if !user_state.referral_set {
            if let Some(ref_pubkey) = referral {
                require!(
                    ref_pubkey != ctx.accounts.user.key(),
                    CustomError::InvalidReferral
                );
                user_state.referral = ref_pubkey;
                user_state.referral_set = true;
            }
        }

        let price = 10_000 * 10u64.pow(6);
        let referral_share = price / 2;

        let deployer_acc_info = &ctx.accounts.deployer_usdc_account;

        require!(
            deployer_acc_info.owner == ctx.accounts.metadata.team,
            CustomError::InvalidDeployerAccount
        );

        if user_state.referral_set {
            let referral_token_account = ctx
                .accounts
                .referral_usdc_account
                .as_ref()
                .ok_or(CustomError::MissingReferralAccount)?;

            require!(
                referral_token_account.owner == user_state.referral,
                CustomError::InvalidReferral
            );

            let cpi_ctx = CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.user_usdc_account.to_account_info(),
                    to: referral_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            );
            token::transfer(cpi_ctx, referral_share)?;

            let cpi_ctx = CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.user_usdc_account.to_account_info(),
                    to: ctx.accounts.deployer_usdc_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            );
            token::transfer(cpi_ctx, referral_share)?;
        } else {
            let cpi_ctx = CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.user_usdc_account.to_account_info(),
                    to: deployer_acc_info.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            );
            token::transfer(cpi_ctx, price)?;
        }

        user_state.has_pass = true;
        Ok(())
    }

    pub fn give_pass(ctx: Context<GivePassInit>) -> Result<()> {
        let user_state = &mut ctx.accounts.user_state;

        require!(!user_state.has_pass, CustomError::AlreadyHasPass);

        user_state.has_pass = true;
        Ok(())
    }

    pub fn buy_token(ctx: Context<BuyToken>, amount: u64, _referral: Option<Pubkey>) -> Result<()> {
        let user_state = &ctx.accounts.user_state;
        require!(user_state.has_pass, CustomError::NoPass);

        let price = amount;

        let deployer_acc_info = &ctx.accounts.deployer_usdc_account;
        require!(
            deployer_acc_info.owner == ctx.accounts.metadata.team,
            CustomError::InvalidDeployerAccount
        );

        let referral_share = price * TEAM_FEE_BPS / 10_000 / 2;
        let total_usdc = amount;
        let team_share = total_usdc * TEAM_FEE_BPS / 10_000;
        let locked_share = total_usdc * LOCKED_LIQUIDITY_BPS / 10_000;
        let founder_fee = total_usdc * FOUNDER_FEE_BPS / 10_000;
        let usdc_for_tokens = total_usdc - team_share - locked_share - founder_fee;

        let liquidity_balance =
            token::accessor::amount(&ctx.accounts.program_payment_token_account.to_account_info())?;
        let token_supply = ctx.accounts.token_mint.supply;

        let price_start = (liquidity_balance as u128) * 1_000_000_000 / (token_supply as u128);
        let estimated_tokens = (usdc_for_tokens as u128) * 1_000_000_000 / price_start;

        let liquidity_growth =
            (liquidity_balance as u128) + (usdc_for_tokens as u128) + (locked_share as u128);
        let price_end =
            liquidity_growth * 1_000_000_000 / ((token_supply as u128) + estimated_tokens);
        let avg_price = (price_start + price_end) / 2;

        let mintable_tokens = ((usdc_for_tokens as u128) * 1_000_000_000 / avg_price) as u64;

        if user_state.referral_set {
            let referral_token_account = ctx
                .accounts
                .referral_usdc_account
                .as_ref()
                .ok_or(CustomError::MissingReferralAccount)?;

            require!(
                referral_token_account.owner == user_state.referral,
                CustomError::InvalidReferral
            );

            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.user_usdc_account.to_account_info(),
                        to: referral_token_account.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                referral_share,
            )?;

            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.user_usdc_account.to_account_info(),
                        to: ctx.accounts.deployer_usdc_account.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                team_share - referral_share,
            )?;
        } else {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.user_usdc_account.to_account_info(),
                        to: deployer_acc_info.to_account_info(),
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

        let total_liquidity_amount = usdc_for_tokens + locked_share;
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.user_usdc_account.to_account_info(),
                    to: ctx.accounts.program_payment_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            total_liquidity_amount,
        )?;

        let (_, mint_bump) = Pubkey::find_program_address(&[b"mint_authority"], ctx.program_id);
        let signer_seeds: &[&[&[u8]]] = &[&[b"mint_authority", &[mint_bump]]];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.mint_authority.to_account_info(),
                },
                signer_seeds,
            ),
            mintable_tokens,
        )?;

        Ok(())
    }

    pub fn sell_token(ctx: Context<SellToken>, amount: u64) -> Result<()> {
        let user_state = &ctx.accounts.user_state;

        let liquidity_balance_raw =
            token::accessor::amount(&ctx.accounts.program_payment_token_account.to_account_info())?;
        let token_supply_raw = ctx.accounts.token_mint.supply.max(1);
        let tokens_to_sell_raw = amount;

        let liquidity_balance = (liquidity_balance_raw as u128) * 1_000_000_000 / 1_000_000;
        let token_supply = (token_supply_raw as u128) * 1_000_000_000 / 1_000_000_000;
        let tokens_to_sell = (tokens_to_sell_raw as u128) * 1_000_000_000 / 1_000_000_000;

        let price_per_token = liquidity_balance / token_supply;
        let total_value = tokens_to_sell * price_per_token;
        let total_value_scaled = total_value / 1_000_000_000;

        let locked_share = (total_value_scaled * LOCKED_LIQUIDITY_BPS as u128) / 10_000;
        let team_cut_u64 = (total_value_scaled * TEAM_FEE_BPS as u128) / 10_000;
        let founders_cut_u64 = (total_value_scaled * FOUNDER_FEE_BPS as u128) / 10_000;

        let user_cut_u64 = total_value_scaled - team_cut_u64 - founders_cut_u64 - locked_share;

        let team_cut_u64 = team_cut_u64 as u64;
        let founders_cut_u64 = founders_cut_u64 as u64;
        let user_cut_u64 = user_cut_u64 as u64;

        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Burn {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    from: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
        )?;

        let bump = ctx.bumps.pool_authority;
        let usdc_mint_key = ctx.accounts.metadata.payment_token;
        let signer_seeds: &[&[&[u8]]] = &[&[b"token_account", usdc_mint_key.as_ref(), &[bump]]];

        if user_state.referral_set {
            let referral_token_account = ctx
                .accounts
                .referral_usdc_account
                .as_ref()
                .ok_or(CustomError::MissingReferralAccount)?;

            require!(
                referral_token_account.owner == user_state.referral,
                CustomError::InvalidReferral
            );

            let referral_share = team_cut_u64 / 2;
            let deployer_share = team_cut_u64 - referral_share;

            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.program_payment_token_account.to_account_info(),
                        to: referral_token_account.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    signer_seeds,
                ),
                referral_share,
            )?;

            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.program_payment_token_account.to_account_info(),
                        to: ctx.accounts.deployer_usdc_account.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    signer_seeds,
                ),
                deployer_share,
            )?;
        } else {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.program_payment_token_account.to_account_info(),
                        to: ctx.accounts.deployer_usdc_account.to_account_info(),
                        authority: ctx.accounts.pool_authority.to_account_info(),
                    },
                    signer_seeds,
                ),
                team_cut_u64,
            )?;
        }
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.program_payment_token_account.to_account_info(),
                    to: ctx.accounts.founder_pool_token_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                signer_seeds,
            ),
            founders_cut_u64,
        )?;

        let pool = &mut ctx.accounts.founders_pool;
        pool.total_collected += founders_cut_u64;

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.program_payment_token_account.to_account_info(),
                    to: ctx.accounts.user_usdc_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                signer_seeds,
            ),
            user_cut_u64,
        )?;

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
            matches!(lock_days, 3 | 7 | 14 | 30 | 60 | 90 | 180),
            CustomError::InvalidLockPeriod
        );

        let config = get_lock_fee_config(lock_days);
        let total_usdc = amount;
        let team_share = total_usdc * config.team_bps / 10_000;
        let founder_fee = total_usdc * config.founder_bps / 10_000;
        let locked_share = total_usdc * config.liquidity_bps / 10_000;
        let usdc_for_tokens = total_usdc - team_share - founder_fee - locked_share;

        let liquidity_balance =
            token::accessor::amount(&ctx.accounts.program_payment_token_account.to_account_info())?;
        let token_supply = ctx.accounts.token_mint.supply;

        let price_start =
            (liquidity_balance as u128) * 1_000_000_000 / (token_supply.max(1) as u128);
        let estimated_tokens = (usdc_for_tokens as u128) * 1_000_000_000 / price_start;
        let liquidity_growth =
            (liquidity_balance as u128) + (usdc_for_tokens as u128) + (locked_share as u128);
        let price_end =
            liquidity_growth * 1_000_000_000 / ((token_supply as u128) + estimated_tokens);
        let avg_price = (price_start + price_end) / 2;
        let mintable_tokens = ((usdc_for_tokens as u128) * 1_000_000_000 / avg_price) as u64;

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
        lock_state.unlock_time = clock.unix_timestamp + (lock_days as i64) * 86400;
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

        let token_amount = lock_state.amount;
        let lock_days = lock_state.lock_days;
        let config = get_lock_fee_config(lock_days);
        let liquidity_balance_raw =
            token::accessor::amount(&ctx.accounts.program_payment_token_account.to_account_info())?
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

        lock_state.initialized = false;
        lock_state.amount = 0;

        Ok(())
    }

    pub fn early_unlock_tokens(ctx: Context<EarlyUnlockTokens>) -> Result<()> {
        let lock_state = &mut ctx.accounts.lock_state;

        require!(lock_state.initialized, CustomError::AlreadyClaimed);

        let token_amount = lock_state.amount;
        let lock_days = lock_state.lock_days;
        let mut config = get_lock_fee_config(lock_days);
        config.team_bps += 50; // Add 0.5% penalty

        let liquidity_balance_raw =
            token::accessor::amount(&ctx.accounts.program_payment_token_account.to_account_info())?
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

        lock_state.initialized = false;
        lock_state.amount = 0;

        Ok(())
    }

    pub fn leverage(
        ctx: Context<Leverage>,
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
            matches!(lock_days, 3 | 7 | 14 | 30 | 60 | 90 | 180),
            CustomError::InvalidLockPeriod
        );

        let total_usdc = amount
            .checked_mul(leverage_multiplier as u64)
            .ok_or(ProgramError::InvalidArgument)?;

        let borrow_amount = total_usdc - amount;
        let available = ctx.accounts.borrow_pool_usdc_account.amount;
        require!(
            available >= borrow_amount,
            CustomError::InsufficientBorrowPool
        );

        let config = get_lock_fee_config(lock_days);

        let team_share = total_usdc * config.team_bps / 10_000;
        let founder_fee = total_usdc * config.founder_bps / 10_000;
        let locked_share = total_usdc * config.liquidity_bps / 10_000;
        let usdc_for_tokens = total_usdc - team_share - founder_fee - locked_share;
        let user_amount_after_fees =
            total_usdc - borrow_amount - team_share - founder_fee - locked_share;
        let liquidity_balance =
            token::accessor::amount(&ctx.accounts.program_payment_token_account.to_account_info())?
                as f64;

        let token_supply = ctx.accounts.token_mint.supply as f64;
        let price_start = liquidity_balance / token_supply.max(1.0);
        let estimated_tokens = (usdc_for_tokens as f64) / price_start;
        let liquidity_growth = liquidity_balance + usdc_for_tokens as f64 + locked_share as f64;
        let price_end = (liquidity_growth) / (token_supply + estimated_tokens);
        let avg_price = (price_start + price_end) / 2.0;
        let mintable_tokens = ((usdc_for_tokens as f64) / avg_price).floor() as u64;
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

        let borrow_bump = ctx.bumps.borrow_pool_authority;
        let borrow_seeds: &[&[&[u8]]] = &[&[b"borrow_pool_authority", &[borrow_bump]]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.borrow_pool_usdc_account.to_account_info(),
                    to: ctx.accounts.program_payment_token_account.to_account_info(),
                    authority: ctx.accounts.borrow_pool_authority.to_account_info(),
                },
                borrow_seeds,
            ),
            borrow_amount,
        )?;

        ctx.accounts.borrow_pool.total_deposited = ctx
            .accounts
            .borrow_pool
            .total_deposited
            .saturating_sub(borrow_amount);

        leverage_position.user = ctx.accounts.user.key();
        leverage_position.amount_user_paid = amount;
        leverage_position.amount_borrowed = borrow_amount;
        leverage_position.unlock_time = clock.unix_timestamp + (lock_days as i64) * 86400;
        leverage_position.referral = referral;
        leverage_position.initialized = true;
        leverage_position.lock_days = lock_days;
        leverage_position.amount_minted = mintable_tokens;

        Ok(())
    }

    pub fn claim_leverage_tokens(ctx: Context<ClaimLeverageTokens>) -> Result<()> {
        let clock = Clock::get()?;
        let position = &mut ctx.accounts.leverage_position;
        require!(position.initialized, CustomError::AlreadyClaimed);
        require!(
            clock.unix_timestamp >= position.unlock_time,
            CustomError::LockPeriodNotOver
        );

        let amount_minted = position.amount_minted;
        let liquidity_balance_raw =
            token::accessor::amount(&ctx.accounts.program_payment_token_account.to_account_info())?
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
                    to: ctx.accounts.borrow_pool_usdc_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                pool_seeds,
            ),
            borrowed,
        )?;

        ctx.accounts.borrow_pool.total_deposited = ctx
            .accounts
            .borrow_pool
            .total_deposited
            .saturating_add(borrowed);

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

        let amount_minted = leverage_position.amount_minted;
        let lock_days = leverage_position.lock_days;
        let mut config = get_lock_fee_config(lock_days);
        config.team_bps += 50; // Add 0.5% penalty

        let borrowed = leverage_position.amount_borrowed;

        let liquidity_balance_raw =
            token::accessor::amount(&ctx.accounts.program_payment_token_account.to_account_info())?
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
                    to: ctx.accounts.borrow_pool_usdc_account.to_account_info(),
                    authority: ctx.accounts.pool_authority.to_account_info(),
                },
                pool_seeds,
            ),
            borrowed,
        )?;

        ctx.accounts.borrow_pool.total_deposited = ctx
            .accounts
            .borrow_pool
            .total_deposited
            .saturating_add(borrowed);

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

    pub fn add_founder(ctx: Context<AddFounder>, new_founder: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.deployer.key() == ctx.accounts.metadata.deployer,
            CustomError::Unauthorized
        );

        let pool = &mut ctx.accounts.founders_pool;
        require!(pool.founder_count < 60, CustomError::FounderLimitReached);

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

    pub fn initialize_borrow_pool(ctx: Context<InitializeBorrowPool>) -> Result<()> {
        require!(
            !ctx.accounts.borrow_pool.initialized,
            CustomError::AlreadyInitialized
        );
        ctx.accounts.borrow_pool.total_deposited = 0;
        ctx.accounts.borrow_pool.initialized = true;
        Ok(())
    }

    pub fn deposit_to_borrow_pool(ctx: Context<DepositToBorrowPool>, amount: u64) -> Result<()> {
        require!(
            ctx.accounts.deployer.key() == ctx.accounts.metadata.team,
            CustomError::Unauthorized
        );

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.admin_usdc_account.to_account_info(),
                to: ctx.accounts.borrow_pool_usdc_account.to_account_info(),
                authority: ctx.accounts.deployer.to_account_info(),
            },
        );
        token::transfer(cpi_ctx, amount)?;

        ctx.accounts.borrow_pool.total_deposited += amount;
        Ok(())
    }

    pub fn withdraw_from_borrow_pool(
        ctx: Context<WithdrawFromBorrowPool>,
        amount: u64,
    ) -> Result<()> {
        require!(
            ctx.accounts.deployer.key() == ctx.accounts.metadata.team,
            CustomError::Unauthorized
        );

        let bump = ctx.bumps.borrow_pool_authority;
        let signer_seeds: &[&[&[u8]]] = &[&[b"borrow_pool_authority", &[bump]]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.borrow_pool_usdc_account.to_account_info(),
                to: ctx.accounts.admin_usdc_account.to_account_info(),
                authority: ctx.accounts.borrow_pool_authority.to_account_info(),
            },
            signer_seeds,
        );
        token::transfer(cpi_ctx, amount)?;

        ctx.accounts.borrow_pool.total_deposited = ctx
            .accounts
            .borrow_pool
            .total_deposited
            .saturating_sub(amount);
        Ok(())
    }
}

pub fn get_lock_fee_config(lock_days: u64) -> LockFeeConfig {
    match lock_days {
        0..=3 => LockFeeConfig {
            liquidity_bps: 150,
            team_bps: 75,
            founder_bps: 25,
        },
        4..=7 => LockFeeConfig {
            liquidity_bps: 225,
            team_bps: 100,
            founder_bps: 25,
        },
        8..=14 => LockFeeConfig {
            liquidity_bps: 300,
            team_bps: 125,
            founder_bps: 25,
        },
        15..=30 => LockFeeConfig {
            liquidity_bps: 375,
            team_bps: 150,
            founder_bps: 25,
        },
        32..=60 => LockFeeConfig {
            liquidity_bps: 450,
            team_bps: 175,
            founder_bps: 25,
        },
        61..=90 => LockFeeConfig {
            liquidity_bps: 550,
            team_bps: 200,
            founder_bps: 25,
        },
        _ => LockFeeConfig {
            liquidity_bps: 725,
            team_bps: 250,
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
        space = 8 + 4 + 6 + 4 + 2 + 160 + 1,
        seeds = [b"metadata", up_only_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(mut)]
    pub user_up_only_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub program_up_only_account: Account<'info, TokenAccount>,

    pub payment_token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub user_payment_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub program_payment_token_account: Account<'info, TokenAccount>,

    #[account(
        seeds = [b"mint_authority"],
        bump
    )]
    /// CHECK: This PDA is derived within the program and only used as a signer; it's safe.
    pub mint_authority: UncheckedAccount<'info>,

    pub current_mint_authority: Signer<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

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

    #[account(mut)]
    pub founder_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub founder_pool_token_account: Account<'info, TokenAccount>,

    /// CHECK: signer PDA
    #[account(seeds = [b"founder_authority"], bump)]
    pub founder_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BuyPass<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 1 + 32 + 1,
        seeds = [b"user_state", user.key().as_ref()],
        bump
    )]
    pub user_state: Account<'info, UserState>,

    #[account(mut)]
    pub user_usdc_account: Account<'info, TokenAccount>,

    #[account(
        seeds = [b"metadata", up_only_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    pub up_only_mint: Account<'info, Mint>,

    #[account(mut)]
    pub deployer_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub referral_usdc_account: Option<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BuyToken<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [b"user_state", user.key().as_ref()],
        bump
    )]
    pub user_state: Account<'info, UserState>,

    #[account(mut)]
    pub user_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub deployer_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub program_payment_token_account: Account<'info, TokenAccount>,

    #[account(
        seeds = [b"metadata", token_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        seeds = [b"mint_authority"],
        bump
    )]
    /// CHECK: only used as signer
    pub mint_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub referral_usdc_account: Option<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,

    #[account(mut, seeds = [b"founders_pool"], bump)]
    pub founders_pool: Account<'info, FoundersPool>,

    #[account(mut)]
    pub founder_pool_token_account: Account<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct SellToken<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [b"user_state", user.key().as_ref()],
        bump
    )]
    pub user_state: Account<'info, UserState>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub deployer_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub program_payment_token_account: Account<'info, TokenAccount>,

    #[account(
        seeds = [b"metadata", token_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    /// CHECK: this is a PDA, only used as a signer
    #[account(
        seeds = [b"token_account", metadata.payment_token.as_ref()],
        bump
    )]
    /// CHECK: just a signer
    pub pool_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub referral_usdc_account: Option<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    #[account(mut, seeds = [b"founders_pool"], bump)]
    pub founders_pool: Account<'info, FoundersPool>,

    #[account(mut)]
    pub founder_pool_token_account: Account<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct BuyAndLockToken<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(init_if_needed, payer = user, space = 8 + 32 + 8 + 8 + 1 + 1 + 32 + 8, seeds = [b"locked", user.key().as_ref()], bump)]
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

    #[account(mut)]
    pub founder_pool_token_account: Account<'info, TokenAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct Leverage<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 32 + 8 + 8 + 8 + 1 + 1 + 32 + 8 + 8,
        seeds = [b"leverage", user.key().as_ref()],
        bump
    )]
    pub leverage_position: Account<'info, LeveragePosition>,

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

    #[account(seeds = [b"l_vault", user.key().as_ref()], bump)]
    /// CHECK: Vault PDA signer
    pub vault_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub referral_usdc_account: Option<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,

    #[account(mut, seeds = [b"founders_pool"], bump)]
    pub founders_pool: Account<'info, FoundersPool>,

    #[account(mut)]
    pub founder_pool_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    /// CHECK: PDA that owns the borrow pool
    pub borrow_pool: Box<Account<'info, BorrowPool>>,

    #[account(mut)]
    pub borrow_pool_usdc_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: PDA that should sign for the pool’s ATA
    #[account(seeds = [b"borrow_pool_authority"], bump)]
    pub borrow_pool_authority: UncheckedAccount<'info>,
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
        associated_token::authority = vault_authority
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub deployer_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub program_payment_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub founder_pool_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub founders_pool: Account<'info, FoundersPool>,

    pub token_program: Program<'info, Token>,
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
        associated_token::authority = vault_authority
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub deployer_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
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

    #[account(mut)]
    pub founder_pool_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub founders_pool: Account<'info, FoundersPool>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ClaimLeverageTokens<'info> {
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
        associated_token::authority = vault_authority
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub deployer_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub program_payment_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub founder_pool_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub founders_pool: Account<'info, FoundersPool>,

    pub token_program: Program<'info, Token>,

    #[account(mut)]
    pub borrow_pool_usdc_account: Account<'info, TokenAccount>,

    #[account(mut, seeds = [b"borrow_pool_authority"], bump)]
    /// CHECK: PDA used only as a signer
    pub borrow_pool_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub borrow_pool: Account<'info, BorrowPool>,
}

#[derive(Accounts)]
pub struct EarlyCloseLeverage<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut, seeds = [b"leverage", user.key().as_ref()], bump)]
    pub leverage_position: Account<'info, LeveragePosition>,

    #[account(mut, seeds = [b"l_vault", user.key().as_ref()], bump)]
    /// CHECK: signer
    pub vault_authority: UncheckedAccount<'info>,

    #[account(mut, associated_token::mint = token_mint, associated_token::authority = vault_authority)]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub deployer_usdc_account: Account<'info, TokenAccount>,

    #[account(mut)]
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

    #[account(mut)]
    pub founder_pool_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub founders_pool: Account<'info, FoundersPool>,

    pub token_program: Program<'info, Token>,

    #[account(mut)]
    pub borrow_pool_usdc_account: Account<'info, TokenAccount>,

    #[account(mut, seeds = [b"borrow_pool_authority"], bump)]
    /// CHECK: PDA used only as a signer
    pub borrow_pool_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub borrow_pool: Account<'info, BorrowPool>,
}

#[derive(Accounts)]
pub struct GivePassInit<'info> {
    #[account(mut, has_one = deployer)]
    pub metadata: Account<'info, TokenMetadata>,

    #[account(
        init_if_needed,
        payer = deployer,
        space = 8 + 1 + 32 + 1,
        seeds = [b"user_state", user.key().as_ref()],
        bump
    )]
    pub user_state: Account<'info, UserState>,

    /// CHECK: Not signer
    pub user: AccountInfo<'info>,

    #[account(mut, address = metadata.deployer)]
    pub deployer: Signer<'info>,

    pub system_program: Program<'info, System>,
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

    /// CHECK: ATA for vault
    #[account(mut)]
    pub vault_token_account: AccountInfo<'info>,

    pub token_mint: Account<'info, Mint>,

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

    /// CHECK: ATA for vault
    #[account(mut)]
    pub vault_token_account: AccountInfo<'info>,

    pub token_mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct InitializeBorrowPool<'info> {
    #[account(mut)]
    pub deployer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = deployer,
        space = 8 + 8 + 1,
        seeds = [b"borrow_pool"],
        bump
    )]
    pub borrow_pool: Account<'info, BorrowPool>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DepositToBorrowPool<'info> {
    #[account(mut)]
    pub deployer: Signer<'info>,

    #[account(
        seeds = [b"metadata", up_only_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    pub up_only_mint: Account<'info, Mint>,

    #[account(mut)]
    pub borrow_pool: Account<'info, BorrowPool>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = deployer
    )]
    pub admin_usdc_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = borrow_pool_authority
    )]
    pub borrow_pool_usdc_account: Account<'info, TokenAccount>,

    #[account(
        seeds = [b"borrow_pool_authority"],
        bump
    )]
    /// CHECK: PDA used only as a signer
    pub borrow_pool_authority: UncheckedAccount<'info>,

    pub usdc_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawFromBorrowPool<'info> {
    #[account(mut)]
    pub deployer: Signer<'info>,

    #[account(
        seeds = [b"metadata", up_only_mint.key().as_ref()],
        bump
    )]
    pub metadata: Account<'info, TokenMetadata>,

    pub up_only_mint: Account<'info, Mint>,

    #[account(mut)]
    pub borrow_pool: Account<'info, BorrowPool>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = deployer
    )]
    pub admin_usdc_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = borrow_pool_authority
    )]
    pub borrow_pool_usdc_account: Account<'info, TokenAccount>,

    #[account(
        seeds = [b"borrow_pool_authority"],
        bump
    )]
    /// CHECK: PDA used only as a signer
    pub borrow_pool_authority: UncheckedAccount<'info>,

    pub usdc_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
}

#[account]
pub struct UserState {
    pub has_pass: bool,
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

#[account]
pub struct BorrowPool {
    pub total_deposited: u64,
    pub initialized: bool,
}

#[error_code]
pub enum CustomError {
    #[msg("Token mint is already initialized")]
    AlreadyInitialized,

    #[msg("User already has a pass")]
    AlreadyHasPass,

    #[msg("Referral cannot be the user themselves")]
    InvalidReferral,

    #[msg("Referral token account must be provided")]
    MissingReferralAccount,

    #[msg("Deployer token account must be provided")]
    MissingDeployerAccount,

    #[msg("Deployer token account does not match metadata")]
    InvalidDeployerAccount,

    #[msg("User does not have a pass")]
    NoPass,

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

    #[msg("Borrow pool has insufficient funds")]
    InsufficientBorrowPool,
}
