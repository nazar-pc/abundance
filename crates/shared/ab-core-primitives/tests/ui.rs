//! Compilation tests for invalid usage of the API, see `tests/ui` for individual cases

// Miri can't run the compiler, and all cases need the `alloc` feature
#[cfg(all(not(miri), feature = "alloc"))]
#[test]
fn ui() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
}
