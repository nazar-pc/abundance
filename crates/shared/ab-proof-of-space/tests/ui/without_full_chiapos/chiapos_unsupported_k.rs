//! Without the `full-chiapos` feature only `K = 20` is supported, other values must be rejected
//! rather than silently compiled into code that was never meant to be used with them

use ab_proof_of_space::chiapos::Tables;

// `K = 19` is only supported with the `full-chiapos` feature
fn tables(_: Tables<19>) {}

fn main() {}
