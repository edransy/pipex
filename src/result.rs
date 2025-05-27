//! Result wrapper that carries strategy information

/// Result wrapper that carries strategy information
/// 
/// This type wraps a standard `Result<T, E>` along with the name of the error
/// handling strategy that should be applied to it. This allows the pipeline
/// macro to apply different error handling strategies based on the function
/// that produced the result.
/// 
/// # Examples
/// 
/// ```rust
/// use pipex::PipexResult;
/// 
/// let result: PipexResult<i32, String> = PipexResult::new(Ok(42), "IgnoreHandler");
/// assert_eq!(result.strategy_name, "IgnoreHandler");
/// assert_eq!(result.result, Ok(42));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PipexResult<T, E> {
    /// The actual result value
    pub result: Result<T, E>,
    /// The name of the error handling strategy to apply
    pub strategy_name: &'static str,
}

impl<T, E> PipexResult<T, E> {
    /// Create a new PipexResult with the given result and strategy name
    pub fn new(result: Result<T, E>, strategy_name: &'static str) -> Self {
        Self {
            result,
            strategy_name,
        }
    }
    
    /// Get a reference to the inner result
    pub fn as_result(&self) -> &Result<T, E> {
        &self.result
    }
    
    /// Convert into the inner result, discarding strategy information
    pub fn into_result(self) -> Result<T, E> {
        self.result
    }
    
    /// Check if the result is Ok
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }
    
    /// Check if the result is Err
    pub fn is_err(&self) -> bool {
        self.result.is_err()
    }
} 