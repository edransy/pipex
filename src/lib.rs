//! A pipeline processing library for Rust
//! 
//! This crate provides a powerful macro-based system for creating data processing
//! pipelines with various error handling strategies.
#![no_std]
#![deny(missing_docs)]
#![warn(clippy::all)]

// Core modules
mod result;
pub mod traits;

// Re-export public API
pub use result::PipexResult;
pub use traits::{PipelineOp, ErrorHandler, IntoPipelineItem, IsPure};


// Re-export the proc macros
pub use pipex_macros::{error_strategy, pure};


/// Ignore errors strategy - replaces errors with default values
pub struct IgnoreHandler;

/// Collect errors strategy - keeps all results including errors
pub struct CollectHandler;

/// FailFast handler - marks all as errors if any error is found
pub struct FailFastHandler;

impl<T, E, const N: usize> ErrorHandler<T, E, N> for IgnoreHandler {
    fn handle_results(mut results: [Result<T, E>; N]) -> [Result<T, E>; N] {
        for result in results.iter_mut() {
            if result.is_err() {
                // Safety: This is safe because we're only replacing error variants with a default value
                *result = Ok(unsafe { core::mem::zeroed() });
            }
        }
        results
    }
}

impl<T, E, const N: usize> ErrorHandler<T, E, N> for CollectHandler {
    fn handle_results(results: [Result<T, E>; N]) -> [Result<T, E>; N] {
        results
    }
}

impl<T, E, const N: usize> ErrorHandler<T, E, N> for FailFastHandler 
where
    E: Copy
{
    fn handle_results(mut results: [Result<T, E>; N]) -> [Result<T, E>; N] {
        // First find if there's an error and get its value
        let first_err = match results.iter().find_map(|r| r.as_ref().err()) {
            Some(err) => *err,
            None => return results,
        };

        // Then propagate the error to all results
        for result in results.iter_mut() {
            *result = Err(first_err);
        }
        
        results
    }
}

/// Apply an error handling strategy to results
pub fn apply_strategy<T, E, const N: usize>(
    strategy_name: &str,
    results: [Result<T, E>; N]
) -> [Result<T, E>; N]
where
    E: Copy,
{
    match strategy_name {
        "IgnoreHandler" => IgnoreHandler::handle_results(results),
        "CollectHandler" => CollectHandler::handle_results(results),
        "FailFastHandler" => FailFastHandler::handle_results(results),
        _ => results,
    }
}

/// Macro to apply an error handling strategy to a function
#[macro_export]
macro_rules! with_strategy {
    ($strategy:ident, $func:expr) => {{
        let result = $func;
        $crate::apply_strategy(stringify!($strategy), result)
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! pipex {
    // Entry point
    ($input:expr $(=> $($rest:tt)+)?) => {{
        let input_array = $input;
        let initial_results = core::array::from_fn(|i| Ok(input_array[i]));
        pipex!(@process initial_results $(=> $($rest)+)?)
    }};

    // SYNC step
    (@process $input:expr => |$var:ident| $body:expr $(=> $($rest:tt)+)?) => {{
        let mut sync_results = $input;
        let len = sync_results.len();
        
        for i in 0..len {
            sync_results[i] = match sync_results[i] {
                Ok($var) => ($body).into_pipeline_item(),
                Err(e) => Err(e),
            };
        }
        
        pipex!(@process sync_results $(=> $($rest)+)?)
    }};

    // Terminal case
    (@process $input:expr) => {
        $input
    };
}

#[cfg(test)]
mod tests {
    use super::*;


    #[derive(Debug, Copy, Clone, PartialEq)]
    enum TestError {
        Error1,
        Error2,
    }

    const TEST_INPUT: [i32; 3] = [1, 2, 3];

    #[pure]
    fn pure_function(x: i32) -> i32 {
        x * 2
    }

    #[test]
    fn test_pure_function() {
        assert_eq!(pure_function(2), 4);
    }

    // This function contains unsafe code and would fail to compile if
    // decorated with `#[pure]`.
    fn impure_function(x: i32) -> i32 {
        let mut y = x;
        let p = &mut y as *mut i32;
        unsafe {
            *p = 10;
            *p
        }
    }

    #[test]
    fn test_impure_function_is_not_pure() {
        // This test just ensures the impure function does what we expect.
        // The purity check is a compile-time error, so we can't directly
        // test for the failure in a unit test.
        assert_eq!(impure_function(5), 10);
    }

    #[test]
    fn test_recursive_purity() {
        // A helper function marked as pure.
        #[pure]
        fn pure_add_one(x: i32) -> i32 {
            x + 1
        }

        // A function that is NOT marked as pure.
        fn impure_double(x: i32) -> i32 {
            x * 2
        }

        // A function that calls another pure function. This is allowed.
        #[pure]
        fn pure_add_two(x: i32) -> i32 {
            pure_add_one(pure_add_one(x))
            
        }

        // The following function would fail to compile because it calls a non-pure function.
        //
        // #[pure]
        // fn test_fails_compilation(x: i32) -> i32 {
        //     impure_double(x)
        // }

        // Assert the logic works as expected.
        assert_eq!(pure_add_two(5), 7);
    }

    #[test]
    fn test_basic_pipeline() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| Ok(x * 2)
            => |x| Ok(x + 1)
        );
        
        assert!(result.iter().all(|r| r.is_ok()));
        for (i, r) in result.iter().enumerate() {
            assert_eq!(r.unwrap(), TEST_INPUT[i] * 2 + 1);
        }
    }

    #[test]
    fn test_error_handling() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| if x == 2 { Err(TestError::Error1) } else { Ok(x * 2) }
        );
        
        assert!(result[0].is_ok());
        assert!(result[1].is_err());
        assert!(result[2].is_ok());
    }

    #[test]
    fn test_ignore_handler() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| if x == 2 { Err(TestError::Error2) } else { Ok(x * 2) }
        );
        
        let handled = IgnoreHandler::handle_results(result);
        assert!(handled.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn test_collect_handler() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| if x == 2 { Err(TestError::Error1) } else { Ok(x * 2) }
        );
        
        let handled = CollectHandler::handle_results(result);
        assert_eq!(handled.iter().filter(|r| r.is_err()).count(), 1);
    }

    #[test]
    fn test_fail_fast() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| if x == 2 { Err(TestError::Error1) } else { Ok(x * 2) }
        );
        
        let handled = FailFastHandler::handle_results(result);
        assert!(handled.iter().all(|r| r.is_err()));
        assert!(handled.iter().all(|r| r == &Err(TestError::Error1)));
    }

    #[test]
    fn test_with_strategy() {
        let result: [Result<i32, TestError>; 3] = with_strategy!(IgnoreHandler, {
            pipex!(
                TEST_INPUT
                => |x| if x == 2 { Err(TestError::Error1) } else { Ok(x * 2) }
            )
        });
        
        assert!(result.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn test_error_skip_pipeline() {
        let result: [Result<i32, TestError>; 3] = pipex!(
            TEST_INPUT
            => |x| if x == 2 { Err(TestError::Error1) } else { Ok(x) }
            => |x| Ok(x + 100)  // This step should be skipped for errors
        );
        
        assert_eq!(result[0], Ok(101));     // First value processed normally
        assert_eq!(result[1], Err(TestError::Error1));  // Error preserved, not modified
        assert_eq!(result[2], Ok(103));     // Third value processed normally
    }

    #[test]
    fn test_solana_like_pipeline() {
        // 1. Keep the same types - they work well!
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct UpdateBalanceInstruction {
            new_balance: u64,
        }

        #[derive(Debug, Clone, Copy, PartialEq)]
        struct TokenAccount {
            balance: u64,
            owner_program_id: u64,
        }

        #[derive(Debug, Clone, Copy, PartialEq)]
        struct TransactionContext {
            instruction: UpdateBalanceInstruction,
            account: TokenAccount,
        }

        #[derive(Debug, Clone, Copy, PartialEq)]
        enum ContractError {
            ValidationFailure,
            WriteToChainFailed,
        }

        // 2. IMPURE: Read state from chain
        fn read_state_from_chain(
            ctx: TransactionContext,
        ) -> Result<TransactionContext, ContractError> {
            // In reality, this would load the TokenAccount from Solana
            let account = TokenAccount {
                balance: 100,
                owner_program_id: 12345,
            };
            Ok(TransactionContext { instruction: ctx.instruction, account })
        }

        // 3. PURE: Validate the transaction
        #[pure]
        fn validate_state(
            ctx: TransactionContext,
        ) -> Result<TransactionContext, ContractError> {
            if ctx.instruction.new_balance > ctx.account.balance + 1000 {
                return Err(ContractError::ValidationFailure);
            }
            Ok(ctx)
        }

        // 4. PURE: Calculate the new state
        #[pure]
        fn calculate_new_state(
            mut ctx: TransactionContext,
        ) -> Result<TransactionContext, ContractError> {
            ctx.account.balance = ctx.instruction.new_balance;
            Ok(ctx)
        }

        // 5. IMPURE: Save the new state
        fn save_state_to_chain(
            ctx: TransactionContext,
        ) -> Result<TransactionContext, ContractError> {
            // This would save the new state to Solana
            Ok(ctx)
        }

        // Test setup 1: Successful case (keep existing)
        let instruction = UpdateBalanceInstruction { new_balance: 500 };
        let initial_account = TokenAccount { balance: 0, owner_program_id: 0 };
        let context = TransactionContext { instruction, account: initial_account };
        let pipeline_input = [context];
        
        // Run pipeline 1: Success case (keep existing)
        let final_result: [Result<TransactionContext, ContractError>; 1] = pipex!(
            pipeline_input
            => |ctx| read_state_from_chain(ctx) // IMPURE: Load
            => |ctx| validate_state(ctx) // PURE: Validate
            => |ctx| calculate_new_state(ctx) // PURE: Transform
            => |ctx| save_state_to_chain(ctx) // IMPURE: Save
        );

        // Verify success case (keep existing)
        assert!(final_result[0].is_ok());
        let final_ctx = final_result[0].unwrap();
        assert_eq!(final_ctx.account.balance, 500);

        // Test setup 2: Validation failure case - trying to increase balance by more than 1000
        let invalid_instruction = UpdateBalanceInstruction { new_balance: 1200 }; // Will fail validation (100 + 1000 < 1200)
        let initial_account = TokenAccount { balance: 0, owner_program_id: 0 };
        let invalid_context = TransactionContext { instruction: invalid_instruction, account: initial_account };
        let pipeline_input = [invalid_context];
        
        // Run pipeline 2: Failure case
        let error_result: [Result<TransactionContext, ContractError>; 1] = pipex!(
            pipeline_input
            => |ctx| read_state_from_chain(ctx) // IMPURE: Load
            => |ctx| validate_state(ctx) // PURE: Validate
            => |ctx| calculate_new_state(ctx) // PURE: Transform
            => |ctx| save_state_to_chain(ctx) // IMPURE: Save
        );

        // Verify error case
        assert!(error_result[0].is_err());
        assert!(matches!(error_result[0], Err(ContractError::ValidationFailure)));
    }

    #[test]
    fn test_solana_dapp_pipeline() {

        // 1. Define complex DApp structures
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct SwapInstruction {
            token_in_amount: u64,
            minimum_token_out: u64,
            pool_index: u8,
        }

        #[derive(Debug, Clone, Copy, PartialEq)]
        struct LiquidityPool {
            token_a_reserve: u64,
            token_b_reserve: u64,
            fee_numerator: u64,
            fee_denominator: u64,
        }

        #[derive(Debug, Clone, Copy, PartialEq)]
        struct UserAccount {
            token_a_balance: u64,
            token_b_balance: u64,
            last_swap_timestamp: u64,
        }

        #[derive(Debug, Clone, Copy, PartialEq)]
        struct DexContext {
            instruction: SwapInstruction,
            pool: LiquidityPool,
            user: UserAccount,
            current_timestamp: u64,
        }

        #[derive(Debug, Clone, Copy, PartialEq)]
        enum DexError {
            InsufficientBalance,
            InsufficientLiquidity,
            ExcessiveSlippage,
            RateLimitExceeded,
            PoolIndexInvalid,
        }

        // 2. PURE: Calculate swap amount with fees
        #[pure]
        fn calculate_output_amount(
            input_amount: u64,
            input_reserve: u64,
            output_reserve: u64,
            fee_numerator: u64,
            fee_denominator: u64,
        ) -> u64 {
            let input_amount_with_fee = input_amount * (fee_denominator - fee_numerator);
            let numerator = input_amount_with_fee * output_reserve;
            let denominator = (input_reserve * fee_denominator) + input_amount_with_fee;
            numerator / denominator
        }

        // 3. PURE: Validate the swap
        #[pure]
        fn validate_swap(ctx: DexContext) -> Result<DexContext, DexError> {
            // Check user balance
            if ctx.user.token_a_balance < ctx.instruction.token_in_amount {
                return Err(DexError::InsufficientBalance);
            }

            // Check rate limiting
            if ctx.current_timestamp - ctx.user.last_swap_timestamp < 2 {
                return Err(DexError::RateLimitExceeded);
            }

            // Calculate expected output
            let output_amount = calculate_output_amount(
                ctx.instruction.token_in_amount,
                ctx.pool.token_a_reserve,
                ctx.pool.token_b_reserve,
                ctx.pool.fee_numerator,
                ctx.pool.fee_denominator,
            );

            // Check minimum output
            if output_amount < ctx.instruction.minimum_token_out {
                return Err(DexError::ExcessiveSlippage);
            }

            // Check sufficient liquidity
            if output_amount >= ctx.pool.token_b_reserve {
                return Err(DexError::InsufficientLiquidity);
            }

            Ok(ctx)
        }

        // 4. PURE: Calculate new state
        #[pure]
        fn calculate_new_state(mut ctx: DexContext) -> Result<DexContext, DexError> {
            let output_amount = calculate_output_amount(
                ctx.instruction.token_in_amount,
                ctx.pool.token_a_reserve,
                ctx.pool.token_b_reserve,
                ctx.pool.fee_numerator,
                ctx.pool.fee_denominator,
            );

            // Update pool reserves
            ctx.pool.token_a_reserve += ctx.instruction.token_in_amount;
            ctx.pool.token_b_reserve -= output_amount;

            // Update user balances
            ctx.user.token_a_balance -= ctx.instruction.token_in_amount;
            ctx.user.token_b_balance += output_amount;
            ctx.user.last_swap_timestamp = ctx.current_timestamp;

            Ok(ctx)
        }

        // 5. IMPURE: Mock functions for loading and saving state
        fn load_dex_state(ctx: DexContext) -> Result<DexContext, DexError> {
            // In reality, this would load accounts from Solana
            let pool = LiquidityPool {
                token_a_reserve: 1_000_000,
                token_b_reserve: 1_000_000,
                fee_numerator: 3,
                fee_denominator: 1000,
            };
            Ok(DexContext { pool, ..ctx })
        }

        fn save_dex_state(ctx: DexContext) -> Result<DexContext, DexError> {
            // In reality, this would save to Solana accounts
            Ok(ctx)
        }

        // 6. Test successful swap
        let swap_instruction = SwapInstruction {
            token_in_amount: 1000,
            minimum_token_out: 900,
            pool_index: 0,
        };

        let user = UserAccount {
            token_a_balance: 5000,
            token_b_balance: 5000,
            last_swap_timestamp: 0,
        };

        let initial_ctx = DexContext {
            instruction: swap_instruction,
            pool: LiquidityPool { token_a_reserve: 0, token_b_reserve: 0, fee_numerator: 0, fee_denominator: 0 },
            user,
            current_timestamp: 100,
        };

        let pipeline_input = [initial_ctx];

        // Run the swap pipeline
        let result: [Result<DexContext, DexError>; 1] = pipex!(
            pipeline_input
            => |ctx| load_dex_state(ctx)         // IMPURE: Load accounts
            => |ctx| validate_swap(ctx)          // PURE: Validate swap params
            => |ctx| calculate_new_state(ctx)    // PURE: Calculate new balances
            => |ctx| save_dex_state(ctx)         // IMPURE: Save updates
        );

        // Verify successful swap
        assert!(result[0].is_ok());
        let final_ctx = result[0].unwrap();
        assert!(final_ctx.user.token_a_balance < user.token_a_balance);  // User spent token A
        assert!(final_ctx.user.token_b_balance > user.token_b_balance);  // User received token B
        assert_eq!(final_ctx.user.last_swap_timestamp, 100);             // Timestamp updated

        // Result is {
        //     "user": {
        //         "token_a_balance": 4000,
        //         "token_b_balance": 6000,
        //         "last_swap_timestamp": 100
        //     }
        // }

        // 7. Test failed swap (rate limit)
        let rate_limited_ctx = DexContext {
            instruction: swap_instruction,
            pool: LiquidityPool { token_a_reserve: 0, token_b_reserve: 0, fee_numerator: 0, fee_denominator: 0 },
            user: UserAccount {
                token_a_balance: 5000,
                token_b_balance: 5000,
                last_swap_timestamp: 99,  // Too recent!
            },
            current_timestamp: 100,
        };

        let pipeline_input = [rate_limited_ctx];

        let error_result: [Result<DexContext, DexError>; 1] = pipex!(
            pipeline_input
            => |ctx| load_dex_state(ctx)
            => |ctx| validate_swap(ctx)
            => |ctx| calculate_new_state(ctx)
            => |ctx| save_dex_state(ctx)
        );

        // Verify rate limit error
        assert!(error_result[0].is_err());
        assert!(matches!(error_result[0], Err(DexError::RateLimitExceeded)));

        // Result is { DexError::RateLimitExceeded }
        
    }

}