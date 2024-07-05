use std::os::raw::c_char;
use std::ffi::{CString, CStr};
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::inbox_name::InboxName;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InboxNameWrapper {
    inner: InboxName,
}

impl InboxNameWrapper {
    pub fn new(name: String) -> Result<Self, String> {
        InboxName::new(name).map(|inner| Self { inner }).map_err(|e| e.to_string())
    }

    pub fn get_value(&self) -> String {
        self.inner.get_value()
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn inbox_name_wrapper_new(name: *const c_char) -> *mut InboxNameWrapper {
    let c_str = unsafe {
        assert!(!name.is_null());
        CStr::from_ptr(name)
    };

    let name_str = match c_str.to_str() {
        Ok(str) => str,
        Err(_) => return std::ptr::null_mut(),
    };

    match InboxNameWrapper::new(name_str.to_string()) {
        Ok(wrapper) => Box::into_raw(Box::new(wrapper)),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn inbox_name_wrapper_get_value(wrapper: *const InboxNameWrapper) -> *const c_char {
    unsafe {
        assert!(!wrapper.is_null());
        let wrapper = &*wrapper;
        CString::new(wrapper.get_value()).unwrap().into_raw()
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn inbox_name_wrapper_free(wrapper: *mut InboxNameWrapper) {
    unsafe {
        if !wrapper.is_null() {
            let _ = Box::from_raw(wrapper);
        }
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn inbox_name_wrapper_get_unique_id(wrapper: *const InboxNameWrapper) -> *const c_char {
    let wrapper = unsafe {
        assert!(!wrapper.is_null());
        &*wrapper
    };
    match &wrapper.inner {
        InboxName::JobInbox { unique_id, .. } => CString::new(unique_id.clone()).unwrap().into_raw(),
        _ => std::ptr::null(),
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn inbox_name_wrapper_create_regular_inbox_name(
    sender: *const c_char,
    sender_subidentity: *const c_char,
    recipient: *const c_char,
    recipient_subidentity: *const c_char,
    is_e2e: bool,
) -> *mut InboxNameWrapper {
    let sender = unsafe { CStr::from_ptr(sender).to_str().unwrap().to_string() };
    let sender_subidentity = unsafe { CStr::from_ptr(sender_subidentity).to_str().unwrap().to_string() };
    let recipient = unsafe { CStr::from_ptr(recipient).to_str().unwrap().to_string() };
    let recipient_subidentity = unsafe { CStr::from_ptr(recipient_subidentity).to_str().unwrap().to_string() };

    let result = InboxName::get_regular_inbox_name_from_params(
        sender,
        sender_subidentity,
        recipient,
        recipient_subidentity,
        is_e2e,
    );

    match result {
        Ok(inbox_name) => Box::into_raw(Box::new(InboxNameWrapper { inner: inbox_name })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn inbox_name_wrapper_create_job_inbox_name(unique_id: *const c_char) -> *mut InboxNameWrapper {
    let unique_id = unsafe { CStr::from_ptr(unique_id).to_str().unwrap().to_string() };

    let result = InboxName::get_job_inbox_name_from_params(unique_id);

    match result {
        Ok(inbox_name) => Box::into_raw(Box::new(InboxNameWrapper { inner: inbox_name })),
        Err(_) => std::ptr::null_mut(),
    }
}