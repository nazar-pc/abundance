//! Compilation tests for invalid usage of the API, see `tests/ui` for individual cases

// Miri can't run the compiler
#[cfg(not(miri))]
#[test]
fn ui() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
    #[cfg(feature = "alloc")]
    tests.compile_fail("tests/ui/alloc/*.rs");
    #[cfg(not(feature = "full-chiapos"))]
    tests.compile_fail("tests/ui/without_full_chiapos/*.rs");
}
