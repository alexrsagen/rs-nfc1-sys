use std::ptr;
use crate::*;

#[test]
fn nfc_init_exit() {
	unsafe {
		let mut p: *mut nfc_context = ptr::null_mut();
		nfc_init(&mut p);
		nfc_exit(p);
	}
}