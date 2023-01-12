mod ffi {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

use std::{
    collections::HashMap,
    ffi::{c_char, CStr, CString},
    mem::MaybeUninit,
    thread,
    time::Duration,
};

use ffi::*;

use anyhow::{bail, Result};

pub struct T5Client {
    app: String,
    bridge: TiltFiveNative,
    ctx: T5_Context,
    glasses: HashMap<String, T5_Glasses>,
}

#[derive(Clone, Debug)]
pub struct Glasses(String);

fn op<T: FnMut() -> u32, const N: usize>(mut f: T) -> Result<()> {
    #![allow(unused_assignments)]
    let mut err = u32::MAX;
    let mut attempts = 0;
    loop {
        err = f();
        if err == 0 {
            return Ok(());
        } else if err == T5_ERROR_NO_SERVICE && attempts < N {
            thread::sleep(Duration::from_millis(10));
            attempts += 1;
            continue;
        }
        break;
    }
    if err != 0 {
        bail!("T5 Client Error: {err}");
    } else {
        Ok(())
    }
}

impl T5Client {
    pub fn new<T: Into<String>, R: Into<String>>(app: T, version: R) -> Result<T5Client> {
        unsafe {
            let app: String = app.into();
            let version: String = version.into();
            let app_id = CString::new(app.clone())?;
            let version = CString::new(version)?;
            let bridge = TiltFiveNative::new("TiltFiveNative.dll")?;
            let mut ctx = MaybeUninit::uninit();
            let info = T5_ClientInfo {
                applicationId: app_id.as_ptr(),
                applicationVersion: version.as_ptr(),
                sdkType: 0u8,
                reserved: 0u64,
            };
            op::<_, 100>(|| bridge.t5CreateContext(ctx.as_mut_ptr(), &info, std::ptr::null::<u64>()))?;

            let ctx = ctx.assume_init();

            Ok(T5Client {
                app,
                bridge,
                ctx,
                glasses: Default::default(),
            })
        }
    }

    pub fn get_gameboard_size(&mut self, _gameboard: T5GameboardType) -> Result<T5_GameboardSize> {
        unsafe {
            let mut gameboard = MaybeUninit::uninit();

            op::<_,100>(|| {
                self.bridge.t5GetGameboardSize(
                    self.ctx,
                    T5_GameboardType_kT5_GameboardType_LE,
                    gameboard.as_mut_ptr(),
                )
            })?;
            let val = gameboard.assume_init();
            Ok(val)
        }
    }

    pub fn list_glasses(&mut self) -> Result<Vec<String>> {
        let mut result = vec![];
        unsafe {
            let mut buffer = [c_char::MIN; 1024];
            let mut num_glasses = 1024;
            op::<_,1>(|| {
                self.bridge
                    .t5ListGlasses(self.ctx, buffer.as_mut_ptr(), &mut num_glasses)
            })?;

            let buffer = CStr::from_ptr(buffer.as_ptr());
            println!("Buffer: {buffer:?}");
            let value = buffer.to_str()?;
            if !value.is_empty() {
                result.push(value.into());
            }
        }
        Ok(result)
    }

    pub fn create_glasses(&mut self, glasses_id: &str) -> Result<Glasses> {
        unsafe {
            let id = CString::new(glasses_id)?;
            let mut glasses = MaybeUninit::uninit();
            op::<_,100>(|| {
                self.bridge
                    .t5CreateGlasses(self.ctx, id.as_ptr(), glasses.as_mut_ptr())
            })?;

            let value = glasses.assume_init();

            let app = &self.app;

            let name = CString::new(format!("{app} - {glasses_id}"))?;

            op::<_,100>(|| self.bridge.t5ReserveGlasses(value, name.as_ptr()))?;

            let id: String = glasses_id.to_owned();
            self.glasses.insert(id.clone(), value);
            Ok(Glasses(id))
        }
    }

    pub fn release_glasses(&mut self, glasses: Glasses) -> Result<()> {
        if let Some(glasses) = self.glasses.remove(&glasses.0) {
            unsafe { op::<_,100>(|| self.bridge.t5ReleaseGlasses(glasses)) }
        } else {
            Ok(())
        }
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
    XeRaised = 4,
}

#[cfg(test)]
mod tests {
    use crate::bridge::T5GameboardType;

    use super::T5Client;

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
