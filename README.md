# traced_result
## An proof-of-concept to automatically backtrace errors propagated with the `?` operator#
**Note**: This crate relies on the unstable [`try_trait_v2`](https://rust-lang.github.io/rfcs/3058-try-trait-v2.html) language feature. This means it can only be used with the `nightly` toolchain, may break at any time, and is thus not recommended for use in production code until this feature is stabilized. 

## Usage 
`traced_result` differs from crates like [`trace_error`](https://crates.io/crates/trace_error) in that it does not use macros to trace call stacks, but instead uses the (currently unstable) `Try` trait to be as consistent with regular `Result`s as possible.
The two types at the core of this crate are `TracedResult<T, E>`, designed to work like `std::result::Result<T, E>`, and `TracedError<E>`, which is simply a wrapper around `E` and a `Vec<&'static Location<'static>>`. To get started, simply replace `Result` with `TracedResult`:

```rust
// From
fn foo() -> Result<Bar, Baz> {
    // ...
    return Err(Baz(/*...*/));
}

// To
fn foo() -> TracedResult<Bar, Baz> {
    // ...
    return TracedResult::Err(TracedError::new(Baz(/*...*/)))
}
```

`TracedResult` and `TracedError` also come with convenient `From` implementations, allowing you to write something like 
```rust
    return TracedResult::Err(Baz(/*...*/).into()) 
```
or even 
```rust
return Err(Baz(/*...*/)).into() 
```

Now, whenever a `TracedResult` is propagated with the `?` operator, `TracedResult`'s `Try` impl will store the location of the operators usage to the errors call stack, if any:

```rust
fn foo() -> TracedResult<Bar, Baz> {
    Err(Baz(/*...*/)).into()
}

fn do_something() -> TracedResult<(), Baz> {
    let value = foo()?; 
    Ok(())
}

fn main() {
    if let TracedResult::Err(error) = do_something() {
        println!("{}", error) 
        // Baz at (40:21) in file example.rs
        // at (2:26) in file example.rs
    }
}

```

## `Result` methods
`TracedResult<T, E>` currently has its the following methods:
- `unwrap()` and all related methods, including the `unchecked` methods
- `is_ok()` and `is_err()` 
- `map()` and all related methods
- conversion to an `std::result::Result<T, TracedError<E>>` using `into_result()` or the `From` trait for compatibility any remaining methods – note that subsequent uses of the `?` operator will no longer be tracked. To discard the call stack completely, you can also use `TracedResult::discard_call_stack()` to get a `Result<T, E>` without the `TracedError` wrapper around `E`.

## Note: the `#[track_caller]` attribute
Internally, `TracedResult` uses the `#[track_caller]` attribute to get the location at which the `?` operator was used. This means that if the result is propagated from a function which itself is annotated with `#[track_caller]`, the `Location` added to the call stack will be that of the function's caller, not that of the `Try` operator itself.