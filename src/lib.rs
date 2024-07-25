#![feature(try_trait_v2)]

use std::{
    convert::Infallible,
    fmt::Debug,
    ops::{ControlFlow, FromResidual},
    panic::Location,
};

/// A wrapper class that stores an error as well as a call stack associated with it.
/// This call stack is guaranteed to contain at least the location of this error's construction (see `new`), and, if used with a `TracedResult`, will also contain the source location of every position where it was propagated using the `?` operator. See `TracedResult` for more info.
#[derive(Debug)]
pub struct TracedError<E> {
    trace: Vec<&'static Location<'static>>,
    inner: E,
}

impl<E> TracedError<E> {
    /// Create a new `TracedError` with the specified error.
    /// The caller location of this method will become the first entry in its call stack.
    #[track_caller]
    pub fn new(inner: E) -> Self {
        let trace = vec![Location::caller()];
        Self { trace, inner }
    }

    /// Get the error's value, discarding the call stack associated with it.    
    #[inline(always)]
    pub fn into_inner(self) -> E {
        self.inner
    }

    pub fn trace(&self) -> &Vec<&'static Location<'static>> {
        &self.trace
    }

    /// Convert the `TracedError` into a tuple of error and call stack.
    #[inline(always)]
    pub fn split(self) -> (E, Vec<&'static Location<'static>>) {
        (self.inner, self.trace)
    }
}

impl<E> From<E> for TracedError<E> {
    #[track_caller]
    fn from(value: E) -> Self {
        Self::new(value)
    }
}

impl<E: std::fmt::Display> std::fmt::Display for TracedError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)?;

        for location in self.trace.iter().rev() {
            writeln!(
                f,
                "At ({line}:{col}) in {file}",
                file = location.file(),
                line = location.line(),
                col = location.column()
            )?;
        }
        Ok(())
    }
}

impl<E: std::error::Error> std::error::Error for TracedError<E> {}

/// A `Result` that traces the call stack of `Err` values.
/// Every time an `Err` value is propagated using the `?` operator, `TracedResult`s custom `Try` implementation will automatically append the location of the `?` operator to the `TracedError`s call stack.
/// Note that both `TracedError::new()` and `TracedResult::try()` use the `#[track_caller]` attribute to get their caller's location. This won't affect most users of this crate; However, if you use #[track_caller] on your own methods, you should be aware that the locations tracked by `trace_error` may be further up the stack than their "actual" locations. See [the Rust reference](https://doc.rust-lang.org/std/panic/struct.Location.html#method.caller) for more info.
#[derive(Debug)]
pub enum TracedResult<T, E> {
    Ok(T),
    Err(TracedError<E>),
}

impl<T, E> TracedResult<T, E> {
    /// Convert this `TracedResult<T, E>` into a `std::result::Result<T, TracedError<E>>`.
    /// This is useful when working with functions that do not support `TracedResult`, but causes the error's (if any) call stack to freeze, and subsequent uses of the `?` operator will no longer be tracked.
    #[inline(always)]
    pub fn into_result(self) -> std::result::Result<T, TracedError<E>> {
        match self {
            TracedResult::Ok(ok) => Ok(ok),
            TracedResult::Err(err) => Err(err),
        }
    }

    #[inline(always)]
    pub fn discard_call_stack(self) -> std::result::Result<T, E> {
        match self {
            TracedResult::Ok(ok) => Ok(ok),
            TracedResult::Err(err) => Err(err.into_inner()),
        }
    }

    /// Returns `true` if this value is an `Ok()` value
    #[inline(always)]
    pub fn is_ok(&self) -> bool {
        matches!(self, TracedResult::Ok(_))
    }

    /// Returns `true` if this value is an `Err()` value
    #[inline(always)]
    pub fn is_err(&self) -> bool {
        matches!(self, TracedResult::Err(_))
    }

    /// Equivalent to `std::result::Result::<T, TracedError<E>>::map()`
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> TracedResult<U, E> {
        match self {
            TracedResult::Ok(ok) => TracedResult::Ok(map(ok)),
            TracedResult::Err(err) => TracedResult::Err(err),
        }
    }

    /// Map the `Err` value of this result, if present.
    /// This does **not** add the call location of this method to the stack trace.
    pub fn map_err<F>(self, map: impl FnOnce(E) -> F) -> TracedResult<T, F> {
        match self {
            TracedResult::Ok(ok) => TracedResult::Ok(ok),
            TracedResult::Err(TracedError { inner, trace }) => TracedResult::Err(TracedError {
                inner: map(inner),
                trace,
            }),
        }
    }

    /// Equivalent to `std::result::Result::<T, TracedError<E>>::map_or()`
    pub fn map_or<U>(self, map: impl FnOnce(T) -> U, default: U) -> U {
        match self {
            TracedResult::Ok(ok) => map(ok),
            TracedResult::Err(_) => default,
        }
    }

    /// Equivalent to `std::result::Result::<T, TracedError<E>>::map_or_else()`
    pub fn map_or_else<U>(
        self,
        op: impl FnOnce(TracedError<E>) -> U,
        map: impl FnOnce(T) -> U,
    ) -> U {
        match self {
            TracedResult::Ok(ok) => map(ok),
            TracedResult::Err(err) => op(err),
        }
    }

    /// Equivalent to `std::result::Result::<T, TracedError<E>>::unwrap_or_default()`
    #[inline(always)]
    pub fn unwrap_or_default(self) -> T
    where
        T: Default,
    {
        self.into_result().unwrap_or_default()
    }

    /// Equivalent to `std::result::Result::<T, TracedError<E>>::unwrap_or()`
    #[inline(always)]
    pub fn unwrap_or(self, default: T) -> T {
        self.into_result().unwrap_or(default)
    }

    /// Equivalent to `std::result::Result::<T, TracedError<E>>::else()`
    #[inline(always)]
    pub fn unwrap_or_else(self, op: impl FnOnce(TracedError<E>) -> T) -> T {
        self.into_result().unwrap_or_else(op)
    }
}

// Standard `Result` methods.
// Internally, all these use the actual std::result::Result methods. Conversion overhead for this should be basically zero since it's done using an inlined function with a single match expression.
// The upside of this is that the panicking behavior of these methods will stay consistent with their `std` counterparts
impl<T: Debug, E: Debug> TracedResult<T, E> {
    /// Equivalent to `std::result::Result::<T, TracedError<E>>::unwrap()`
    #[inline(always)]
    pub fn unwrap(self) -> T {
        self.into_result().unwrap()
    }

    /// Equivalent to `std::result::Result::<T, TracedError<E>>::unwrap_err()`
    #[inline(always)]
    pub fn unwrap_err(self) -> TracedError<E> {
        self.into_result().unwrap_err()
    }

    /// Equivalent to `std::result::Result::<T, TracedError<E>>::expect()`
    #[inline(always)]
    pub fn expect(self, msg: &'static str) -> T {
        self.into_result().expect(msg)
    }

    /// Equivalent to `std::result::Result::<T, TracedError<E>>::unwrap_unchecked()`
    #[inline(always)]
    pub unsafe fn unwrap_unchecked(self) -> T {
        self.into_result().unwrap_unchecked()
    }

    /// Equivalent to `std::result::Result::<T, TracedError<E>>::unwrap_err_unchecked()`
    #[inline(always)]
    pub unsafe fn unwrap_err_unchecked(self) -> TracedError<E> {
        self.into_result().unwrap_err_unchecked()
    }
}

impl<T, E> std::ops::Try for TracedResult<T, E> {
    type Output = T;

    type Residual = TracedResult<Infallible, E>;

    fn from_output(output: Self::Output) -> Self {
        TracedResult::Ok(output)
    }

    #[track_caller]
    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            TracedResult::Ok(output) => ControlFlow::Continue(output),
            TracedResult::Err(mut error) => {
                let branched_at = Location::caller();
                error.trace.push(branched_at);
                ControlFlow::Break(TracedResult::Err(error))
            }
        }
    }
}

impl<T, R, E: From<R>> FromResidual<TracedResult<Infallible, R>> for TracedResult<T, E> {
    fn from_residual(residual: TracedResult<Infallible, R>) -> Self {
        match residual {
            TracedResult::Err(TracedError { trace, inner }) => TracedResult::Err(TracedError {
                trace,
                inner: From::from(inner),
            }),
            _ => unreachable!(),
        }
    }
}

impl<T, E> From<Result<T, E>> for TracedResult<T, E> {
    #[track_caller]
    fn from(value: Result<T, E>) -> Self {
        match value {
            Ok(ok) => TracedResult::Ok(ok),
            Err(err) => TracedResult::Err(err.into()),
        }
    }
}

impl<T, E> From<TracedResult<T, E>> for Result<T, TracedError<E>> {
    fn from(value: TracedResult<T, E>) -> Self {
        value.into_result()
    }
}
