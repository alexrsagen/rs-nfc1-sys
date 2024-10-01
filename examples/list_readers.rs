type BoxError = Box<dyn std::error::Error + Send + Sync>;
type BoxResult<T> = Result<T, BoxError>;

use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::os::raw::c_char;

use nfc1_sys::{nfc_connstring, nfc_context, nfc_exit, nfc_init, nfc_list_devices};

fn main() -> BoxResult<()> {
    let context: *mut nfc_context = unsafe {
        let mut ctx = MaybeUninit::uninit();
        nfc_init(ctx.as_mut_ptr());
        ctx.assume_init()
    };

    let sized_array: nfc_connstring = vec![0 as c_char; 1024]
        .try_into()
        .map_err(|_e| "unable to allocate sized array for connection strings")?;
    let mut connstrings: Vec<nfc_connstring> = vec![sized_array; 10];
    let count =
        unsafe { nfc_list_devices(context, connstrings.as_mut_ptr(), connstrings.len()) } as usize;
    connstrings.resize(count, sized_array);
    let devices: Vec<String> = connstrings
        .into_iter()
        .map(|connstring| {
            unsafe { CStr::from_ptr(connstring.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    for device in &devices {
        println!("{}", device)
    }

    unsafe {
        nfc_exit(context);
    }

    Ok(())
}
