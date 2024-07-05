use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use shinkai_message_primitives::shinkai_utils::file_encryption::calculate_blake3_hash;

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn calculate_blake3_hash_c(input: *const c_char) -> *mut c_char {
    let input_c_str = unsafe {
        assert!(!input.is_null());
        CStr::from_ptr(input)
    };

    let input_str = match input_c_str.to_str() {
        Ok(str) => str,
        Err(_) => return std::ptr::null_mut(), // Handle UTF-8 conversion error
    };

    let hash_result = calculate_blake3_hash(input_str); // Call the existing Rust function

    let c_string = CString::new(hash_result).unwrap();
    c_string.into_raw() // Convert Rust String to C string
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn free_calculate_blake3_hash_c_result(s: *mut c_char) {
    unsafe {
        if s.is_null() { return }
        let _ = CString::from_raw(s); // Free the CString
    }
}