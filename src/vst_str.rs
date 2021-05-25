use std::ffi::CString;
use vst3_sys::vst::TChar;
use widestring::U16CStr;
use widestring::U16CString;

/// Create an i8 array containing a UTF-8 C string. May panic.
pub fn str_8<const N: usize>(from: &str) -> [i8; N] {
	let mut to = [0i8; N];
	let c_str = CString::new(from).unwrap();
	let from = c_str.to_bytes_with_nul();
	for i in 0..from.len() {
		to[i] = from[i] as i8;
	}
	to
}

/// Create an i16 array containing a UTF-16 C string. May panic.
pub fn str_16<const N: usize>(from: &str) -> [i16; N] {
	let mut to = [0; N];
	let wc_str = U16CString::from_str(from).unwrap();
	let from = wc_str.as_slice_with_nul();
	for i in 0..from.len() {
		to[i] = from[i] as i16;
	}
	to
}

pub unsafe fn wcstr_to_str(from: *const TChar) -> String {
	let wc_str = U16CStr::from_ptr_str(from as *const u16);
	wc_str.to_string().unwrap()
}
