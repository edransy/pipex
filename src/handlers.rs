// //! Error handling strategies for pipeline operations

// /// Trait for custom error handling strategies
// /// 
// /// This trait defines how different error handling strategies should process
// /// a collection of results. Implementors can define custom logic for handling
// /// errors in different ways.
// /// 
// /// # Examples
// /// 
// /// ```rust
// /// use pipex::ErrorHandler;
// /// 
// /// struct MyCustomHandler;
// /// 
// /// impl<T, E> ErrorHandler<T, E> for MyCustomHandler {
// ///     fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
// ///         // Custom logic here
// ///         results
// ///     }
// /// }
// /// ```
// pub trait ErrorHandler<T, E> {
//     /// Handle a collection of results according to the strategy
//     fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>>;
// }

// /// Ignore errors strategy
// /// 
// /// This strategy filters out all error results, returning only the successful ones.
// /// This is useful when you want to continue processing despite some failures.
// /// 
// /// # Examples
// /// 
// /// ```rust
// /// use pipex::{ErrorHandler, IgnoreHandler};
// /// 
// /// let results = vec![Ok(1), Err("error"), Ok(3)];
// /// let handled = IgnoreHandler::handle_results(results);
// /// assert_eq!(handled, vec![Ok(1), Ok(3)]);
// /// ```
// pub struct IgnoreHandler;

// impl<T, E> ErrorHandler<T, E> for IgnoreHandler {
//     fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
//         results.into_iter()
//             .filter(|r| r.is_ok())
//             .collect()
//     }
// }

// /// Collect all strategy
// /// 
// /// This strategy returns all results as-is, both successful and error results.
// /// This is the default behavior when no specific error handling is needed.
// /// 
// /// # Examples
// /// 
// /// ```rust
// /// use pipex::{ErrorHandler, CollectHandler};
// /// 
// /// let results = vec![Ok(1), Err("error"), Ok(3)];
// /// let handled = CollectHandler::handle_results(results);
// /// assert_eq!(handled, vec![Ok(1), Err("error"), Ok(3)]);
// /// ```
// pub struct CollectHandler;

// impl<T, E> ErrorHandler<T, E> for CollectHandler {
//     fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
//         results
//     }
// }

// /// Fail fast strategy
// /// 
// /// This strategy returns only the error results, filtering out all successful ones.
// /// This is useful when you want to focus on handling errors and stopping
// /// processing on the first error encountered.
// /// 
// /// # Examples
// /// 
// /// ```rust
// /// use pipex::{ErrorHandler, FailFastHandler};
// /// 
// /// let results = vec![Ok(1), Err("error"), Ok(3)];
// /// let handled = FailFastHandler::handle_results(results);
// /// assert_eq!(handled, vec![Err("error")]);
// /// ```
// pub struct FailFastHandler;

// impl<T, E> ErrorHandler<T, E> for FailFastHandler {
//     fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
//         results.into_iter()
//             .filter(|r| r.is_err())
//             .collect()
//     }
// }

// /// Log and ignore errors strategy
// /// 
// /// This strategy logs all errors to stderr and then filters them out,
// /// returning only the successful results. This combines error visibility
// /// with continued processing.
// /// 
// /// # Examples
// /// 
// /// ```rust
// /// use pipex::{ErrorHandler, LogAndIgnoreHandler};
// /// 
// /// let results = vec![Ok(1), Err("error"), Ok(3)];
// /// let handled = LogAndIgnoreHandler::handle_results(results);
// /// // This will print "Pipeline error (ignored): error" to stderr
// /// assert_eq!(handled, vec![Ok(1), Ok(3)]);
// /// ```
// pub struct LogAndIgnoreHandler;

// impl<T, E> ErrorHandler<T, E> for LogAndIgnoreHandler 
// where
//     E: std::fmt::Debug,
// {
//     fn handle_results(results: Vec<Result<T, E>>) -> Vec<Result<T, E>> {
//         results.into_iter()
//             .filter_map(|r| match r {
//                 Ok(val) => Some(Ok(val)),
//                 Err(err) => {
//                     eprintln!("Pipeline error (ignored): {:?}", err);
//                     None
//                 }
//             })
//             .collect()
//     }
// } 