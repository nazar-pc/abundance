//! `Env` can't be shared between threads, since the executor context it refers to isn't required
//! to support that

use ab_contracts_common::env::Env;

fn require_sync<T>()
where
    T: Sync,
{
}

fn main() {
    require_sync::<Env<'static>>();
}
