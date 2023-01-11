mod ffi {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

use std::{mem::MaybeUninit, ffi::{c_char, CString, CStr}, thread, time::Duration};

use ffi::*;

use anyhow::{Result, bail};

pub struct T5Client {
    bridge: TiltFiveNative,
    ctx: T5_Context,
}

impl T5Client {
    pub fn new<T: Into<String>, R: Into<String>>(app: T, version: R) -> Result<T5Client> {
        unsafe {
            let bridge = TiltFiveNative::new("TiltFiveNative.dll")?;
            let mut ctx = MaybeUninit::uninit();
            let info = T5_ClientInfo {
                applicationId: "test".as_ptr() as *const i8,
                applicationVersion: "1".as_ptr() as *const i8,
                sdkType: 0u8,
                reserved: 0u64,
            };
            let err = bridge.t5CreateContext(ctx.as_mut_ptr(), &info, 0u64 as *const u64);

            if err != 0 {
                bail!("Failed to create context, Error Code: {err}");
            }

            let ctx = ctx.assume_init();

            Ok(T5Client { bridge, ctx })
        }
    }

    pub fn get_gameboard_size(&mut self, gameboard: T5GameboardType) -> Result<T5_GameboardSize> {
        unsafe {
            let mut gameboard = MaybeUninit::uninit();

            let err = self.bridge.t5GetGameboardSize(self.ctx, T5_GameboardType_kT5_GameboardType_LE, gameboard.as_mut_ptr());
            if err != 0 {
                bail!("Couldn't get gameboard size, Error Code: {err}");
            }
            let val = gameboard.assume_init();
            Ok(val)
        }
    }

    pub fn list_glasses(&mut self) -> Result<Vec<String>> {
        let mut result = vec![];
        unsafe {
            let mut buffer = [c_char::MIN; 1024];
            let mut num_glasses = 1024;
            let mut attempts = 0;
            let mut err = 0;
            loop {
                err = self.bridge.t5ListGlasses(self.ctx, buffer.as_mut_ptr(), &mut num_glasses);
                if err == T5_ERROR_NO_SERVICE && attempts < 100 {
                    thread::sleep(Duration::from_millis(10));
                    attempts+= 1;
                    continue;
                }
                break;
            }

            if err != 0 {
                bail!("Couldn't get glasses list, Error Code: {err}");
            }

            let buffer = CStr::from_ptr(buffer.as_ptr());
            println!("Buffer: {buffer:?}");
            let value = buffer.to_str()?;
            if !value.is_empty() {
                result.push(value.into());
            }
        }
        Ok(result)
    }
}

impl Drop for T5Client {
    fn drop(&mut self) {
        unsafe {
            self.bridge.t5DestroyContext(&mut self.ctx);
        }
    }
}

pub enum T5GameboardType {
    None = 1,
    LE = 2,
    XE = 3,
    XE_Raised = 4
}

#[cfg(test)]
mod tests {
    use core::num;
    use std::mem::MaybeUninit;

    use cxx::{CxxVector, private::c_char};

    use crate::bridge::T5GameboardType;

    use super::{ffi::*, T5Client};


    #[test]
    fn can_create_context() {
        let client = T5Client::new("test", "1");

        assert!(client.is_ok())
    }

    #[test]
    fn can_get_gameboard_size() {
        let mut client = T5Client::new("test", "1").unwrap();
        let val = client.get_gameboard_size(T5GameboardType::LE).unwrap();
        assert_eq!(val.viewableExtentNegativeX, 0.35);
        assert_eq!(val.viewableExtentPositiveX, 0.35);
        assert_eq!(val.viewableExtentPositiveY, 0.35);
        assert_eq!(val.viewableExtentNegativeY, 0.35);
        assert_eq!(val.viewableExtentPositiveZ, 0.0);
    }

    #[test]
    fn can_get_glasses() {
        let mut client = T5Client::new("test", "1").unwrap();
        let glasses = client.list_glasses();
        println!("Glasses: {glasses:?}");
        assert!(glasses.is_ok());
    }
}
