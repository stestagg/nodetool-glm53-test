//! Shared by the registry-error test binaries below: each carries one
//! registration problem itself and asserts the registry reports it rather
//! than silently accepting or dropping it.

/// Runs `panic_on`, expects a panic, and asserts its message contains every
/// fragment in `expected`.
pub fn assert_panic_message(panic_on: impl FnOnce() + std::panic::UnwindSafe, expected: &[&str]) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let payload = std::panic::catch_unwind(panic_on).unwrap_err();
    std::panic::set_hook(default_hook);

    let message = panic_message(&*payload);
    for fragment in expected {
        assert!(
            message.contains(fragment),
            "missing `{fragment}` in: {message}"
        );
    }
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> &str {
    panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .expect("panic payload should be a string")
}
