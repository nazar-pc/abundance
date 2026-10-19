//! `Env` is invariant over its lifetime in every build, so its lifetime can't be shortened. A
//! shared reference is used, since an exclusive one would be invariant anyway.

use ab_contracts_common::env::Env;

fn shorten<'short, 'long: 'short>(env: &'short Env<'long>) -> &'short Env<'short> {
    env
}

fn main() {
    let _ = shorten;
}
