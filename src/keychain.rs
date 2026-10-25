//! macOS Keychain generic passwords. Never place secrets in command arguments or diagnostics.
use std::ffi::c_void;
use std::ptr;
fn service() -> &'static [u8] {
    if std::env::var_os("NOTYPE_DATA_DIR").is_some() {
        b"app.notype.providers.development"
    } else {
        b"app.notype.providers"
    }
}
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecKeychainFindGenericPassword(
        keychain: *const c_void,
        service_len: u32,
        service: *const u8,
        account_len: u32,
        account: *const u8,
        length: *mut u32,
        data: *mut *mut c_void,
        item: *mut *mut c_void,
    ) -> i32;
    fn SecKeychainAddGenericPassword(
        keychain: *const c_void,
        service_len: u32,
        service: *const u8,
        account_len: u32,
        account: *const u8,
        length: u32,
        data: *const c_void,
        item: *mut *mut c_void,
    ) -> i32;
    fn SecKeychainItemModifyAttributesAndData(
        item: *mut c_void,
        attrs: *const c_void,
        length: u32,
        data: *const c_void,
    ) -> i32;
    fn SecKeychainItemDelete(item: *mut c_void) -> i32;
    fn SecKeychainItemFreeContent(attrs: *mut c_void, data: *mut c_void) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: *mut c_void);
}
fn error(code: i32) -> String {
    format!("钥匙串操作失败（{code}），请检查系统授权")
}
pub fn load(account: &str) -> Result<Option<String>, String> {
    let mut len = 0;
    let mut data = ptr::null_mut();
    // Security returns an allocated buffer on success, freed exactly once below.
    unsafe {
        let code = SecKeychainFindGenericPassword(
            ptr::null(),
            service().len() as u32,
            service().as_ptr(),
            account.len() as u32,
            account.as_ptr(),
            &mut len,
            &mut data,
            ptr::null_mut(),
        );
        if code == -25300 {
            return Ok(None);
        }
        if code != 0 {
            return Err(error(code));
        }
        let value = if len == 0 {
            Ok(String::new())
        } else {
            String::from_utf8(std::slice::from_raw_parts(data.cast::<u8>(), len as usize).to_vec())
                .map_err(|_| "钥匙串值格式无效".into())
        };
        SecKeychainItemFreeContent(ptr::null_mut(), data);
        value.map(Some)
    }
}
pub fn save(account: &str, value: &str) -> Result<(), String> {
    // Item references from Find are owned and released after modification.
    unsafe {
        let mut item = ptr::null_mut();
        let code = SecKeychainFindGenericPassword(
            ptr::null(),
            service().len() as u32,
            service().as_ptr(),
            account.len() as u32,
            account.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut item,
        );
        let result = if code == -25300 {
            SecKeychainAddGenericPassword(
                ptr::null(),
                service().len() as u32,
                service().as_ptr(),
                account.len() as u32,
                account.as_ptr(),
                value.len() as u32,
                value.as_ptr().cast(),
                ptr::null_mut(),
            )
        } else if code == 0 {
            let result = SecKeychainItemModifyAttributesAndData(
                item,
                ptr::null(),
                value.len() as u32,
                value.as_ptr().cast(),
            );
            CFRelease(item);
            result
        } else {
            code
        };
        if result != 0 {
            return Err(error(result));
        }
    }
    if load(account)?.as_deref() != Some(value) {
        return Err("钥匙串保存校验失败".into());
    }
    Ok(())
}
pub fn delete(account: &str) -> Result<(), String> {
    // Delete does not consume the retained item reference.
    unsafe {
        let mut item = ptr::null_mut();
        let code = SecKeychainFindGenericPassword(
            ptr::null(),
            service().len() as u32,
            service().as_ptr(),
            account.len() as u32,
            account.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut item,
        );
        if code == -25300 {
            return Ok(());
        }
        if code != 0 {
            return Err(error(code));
        }
        let result = SecKeychainItemDelete(item);
        CFRelease(item);
        if result == 0 {
            Ok(())
        } else {
            Err(error(result))
        }
    }
}
