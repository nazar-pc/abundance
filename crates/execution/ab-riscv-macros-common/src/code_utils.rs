//! This module contains hacks for Rust nightly syntax to make it compatible with `syn`
// TODO: Remove this module once `syn` supports used Rust nightly syntax

const FROM_IMPL_CONST_1: &str = " const ";
const TO_IMPL_CONST_1: &str = " cnst::";
const FROM_IMPL_CONST_2: &str = " const  ";
const TO_IMPL_CONST_2: &str = " cnst ::";
const FROM_IMPL_CONST_REVERT_1: &str = "pub cnst::";
const TO_IMPL_CONST_REVERT_1: &str = "pub const ";
const FROM_IMPL_CONST_REVERT_2: &str = "pub cnst ::";
const TO_IMPL_CONST_REVERT_2: &str = "pub const  ";
const FROM_IMPL_CONST_REVERT_3: &str = ") cnst::";
const TO_IMPL_CONST_REVERT_3: &str = ") const ";
const FROM_IMPL_CONST_REVERT_4: &str = ") cnst ::";
const TO_IMPL_CONST_REVERT_4: &str = ") const  ";

const FROM_BRACKETS_CONST_1: &str = "[const] ";
const TO_BRACKETS_CONST_1: &str = "BRCONST+";
const FROM_BRACKETS_CONST_2: &str = " [const] ";
const TO_BRACKETS_CONST_2: &str = "BRCONST +";

/// Replace bits of Rust nightly syntax with something that is technically valid in stable Rust, so
/// `syn` can parse it
pub fn pre_process_rust_code(s: &mut str) {
    replace_inplace(s, FROM_IMPL_CONST_1, TO_IMPL_CONST_1);
    replace_inplace(s, FROM_IMPL_CONST_2, TO_IMPL_CONST_2);
    replace_inplace(s, FROM_IMPL_CONST_REVERT_1, TO_IMPL_CONST_REVERT_1);
    replace_inplace(s, FROM_IMPL_CONST_REVERT_2, TO_IMPL_CONST_REVERT_2);
    replace_inplace(s, FROM_IMPL_CONST_REVERT_3, TO_IMPL_CONST_REVERT_3);
    replace_inplace(s, FROM_IMPL_CONST_REVERT_4, TO_IMPL_CONST_REVERT_4);
    replace_inplace(s, FROM_BRACKETS_CONST_1, TO_BRACKETS_CONST_1);
    replace_inplace(s, FROM_BRACKETS_CONST_2, TO_BRACKETS_CONST_2);
}

/// The inverse of [`pre_process_rust_code()`]
pub fn post_process_rust_code(s: &mut str) {
    replace_inplace(s, TO_IMPL_CONST_1, FROM_IMPL_CONST_1);
    replace_inplace(s, TO_IMPL_CONST_2, FROM_IMPL_CONST_2);
    replace_inplace(s, TO_BRACKETS_CONST_1, FROM_BRACKETS_CONST_1);
    replace_inplace(s, TO_BRACKETS_CONST_2, FROM_BRACKETS_CONST_2);
}

fn replace_inplace(mut s: &mut str, from: &str, to: &str) {
    assert_eq!(from.len(), to.len(), "`{from}` != `{to}`");

    if from.is_empty() {
        return;
    }

    while let Some(found) = s.find(from) {
        let start = found;
        let end = found + from.len();

        // SAFETY: Replacing a valid string with a valid string of the same length
        unsafe { s.as_bytes_mut() }
            .get_mut(start..end)
            .expect("Just found a string of the desired length; qed")
            .copy_from_slice(to.as_bytes());

        s = &mut s[end..];
    }
}
