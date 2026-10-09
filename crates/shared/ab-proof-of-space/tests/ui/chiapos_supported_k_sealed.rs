//! `SupportedKValue` is declared with `impl(self)`, so it can only be implemented by the crate
//! itself, which implements it for `Tables<K>` with the `K` values supported by the current
//! implementation. The orphan rule already prevents downstream impls for `Tables<K>`, the
//! restriction additionally rules out impls for downstream types.

use ab_proof_of_space::chiapos::SupportedKValue;

struct Local;

impl SupportedKValue for Local {}

fn main() {}
