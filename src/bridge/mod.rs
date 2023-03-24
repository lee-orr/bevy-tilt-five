pub mod ffi {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr, CString},
    mem::MaybeUninit,
    thread,
    time::Duration,
};

use bevy::prelude::{Quat, Vec2, Vec3};
use ffi::*;

use anyhow::{bail, Result};

pub struct T5Client {
    app: String,
    bridge: TiltFiveNative,
    ctx: T5_Context,
    glasses: HashMap<String, T5_Glasses>,
    graphics_context: Option<(T5_GraphicsApi, *mut c_void)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Glasses(String);

pub const DEFAULT_GLASSES_WIDTH: u32 = 1216;
pub const DEFAULT_GLASSES_HEIGHT: u32 = 768;
pub const DEFAULT_GLASSES_FOV: f32 = 48.0;

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
            let mut platform = MaybeUninit::uninit();
            let info = T5_ClientInfo {
                applicationId: app_id.as_ptr(),
                applicationVersion: version.as_ptr(),
                sdkType: 0u8,
                reserved: 0u64,
            };
            op::<_, 100>(|| {
                bridge.t5CreateContext(ctx.as_mut_ptr(), &info, platform.as_mut_ptr())
            })?;

            let ctx = ctx.assume_init();

            Ok(T5Client {
                app,
                bridge,
                ctx,
                glasses: Default::default(),
                graphics_context: None,
            })
        }
    }

    #[allow(dead_code)]
    pub fn get_gameboard_size(
        &mut self,
        gameboard_type: T5GameboardType,
    ) -> Result<T5_GameboardSize> {
        unsafe {
            let mut gameboard = MaybeUninit::uninit();

            op::<_, 100>(|| {
                self.bridge.t5GetGameboardSize(
                    self.ctx,
                    gameboard_type as i32,
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
            op::<_, 1>(|| {
                self.bridge
                    .t5ListGlasses(self.ctx, buffer.as_mut_ptr(), &mut num_glasses)
            })?;

            let buffer = CStr::from_ptr(buffer.as_ptr());
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
            op::<_, 100>(|| {
                self.bridge
                    .t5CreateGlasses(self.ctx, id.as_ptr(), glasses.as_mut_ptr())
            })?;

            let value = glasses.assume_init();

            let app = &self.app;

            let name = CString::new(format!("{app} - {glasses_id}"))?;

            op::<_, 100>(|| self.bridge.t5ReserveGlasses(value, name.as_ptr()))?;
            op::<_, 100>(|| self.bridge.t5EnsureGlassesReady(value))?;
            if let Some((api, ctx)) = &self.graphics_context {
                op::<_, 100>(|| self.bridge.t5InitGlassesGraphicsContext(value, *api, *ctx))?;
            }

            let config = T5_WandStreamConfig { enabled: true };
            op::<_, 100>(|| self.bridge.t5ConfigureWandStreamForGlasses(value, &config))?;

            let id: String = glasses_id.to_owned();
            self.glasses.insert(id.clone(), value);
            Ok(Glasses(id))
        }
    }

    pub fn release_glasses(&mut self, glasses: Glasses) -> Result<()> {
        if let Some(glasses) = self.glasses.remove(&glasses.0) {
            unsafe {
                let config = T5_WandStreamConfig { enabled: false };
                op::<_, 100>(|| {
                    self.bridge
                        .t5ConfigureWandStreamForGlasses(glasses, &config)
                })?;
                op::<_, 100>(|| self.bridge.t5ReleaseGlasses(glasses))
            }
        } else {
            Ok(())
        }
    }

    pub fn get_glasses_pose(&mut self, glasses: &Glasses) -> Result<T5_GlassesPose> {
        if let Some(glasses) = self.glasses.get(&glasses.0) {
            unsafe {
                let mut pose = MaybeUninit::uninit();
                op::<_, 1>(|| {
                    self.bridge.t5GetGlassesPose(
                        *glasses,
                        T5_GlassesPoseUsage_kT5_GlassesPoseUsage_GlassesPresentation,
                        pose.as_mut_ptr(),
                    )
                })?;
                Ok(pose.assume_init())
            }
        } else {
            bail!("Couldn't find glasses");
        }
    }

    #[allow(dead_code)]
    pub fn get_wand_stream_events(&mut self, glasses: &Glasses) -> Result<Vec<T5_WandStreamEvent>> {
        if let Some(glasses) = self.glasses.get(&glasses.0) {
            unsafe {
                let mut events = vec![];

                loop {
                    let mut event = MaybeUninit::uninit();
                    let result = op::<_, 1>(|| {
                        self.bridge
                            .t5ReadWandStreamForGlasses(*glasses, event.as_mut_ptr(), 1)
                    });
                    if result.is_err() {
                        break;
                    }
                    let event = event.assume_init();

                    events.push(event);
                }

                Ok(events)
            }
        } else {
            bail!("Couldn't find glasses");
        }
    }

    pub unsafe fn send_frame_to_glasses(
        &mut self,
        id: &Glasses,
        info: *const T5_FrameInfo,
    ) -> Result<()> {
        if let Some(glasses) = self.glasses.get(&id.0) {
            op::<_, 1>(|| self.bridge.t5SendFrameToGlasses(*glasses, info))
        } else {
            bail!("couldn't find glasses");
        }
    }

    pub fn set_dx11_graphics_context(&mut self, device: *mut c_void) {
        self.graphics_context = Some((T5_GraphicsApi_kT5_GraphicsApi_D3D11, device));
    }

    pub fn get_ipd(&mut self, id: &Glasses) -> Result<f32> {
        if let Some(glasses) = self.glasses.get(&id.0) {
            unsafe {
                let mut ipd = MaybeUninit::uninit();
                op::<_, 1>(|| {
                    self.bridge.t5GetGlassesFloatParam(
                        *glasses,
                        0,
                        T5_ParamGlasses_kT5_ParamGlasses_Float_IPD,
                        ipd.as_mut_ptr(),
                    )
                })?;
                let ipd = ipd.assume_init() as f32;
                Ok(ipd)
            }
        } else {
            bail!("couldn't find glasses");
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

#[derive(Clone, Copy)]
pub enum T5GameboardType {
    None = 1,
    LE = 2,
    XE = 3,
    XeRaised = 4,
}

impl From<T5_Vec2> for Vec2 {
    fn from(val: T5_Vec2) -> Self {
        Vec2::new(val.x, val.y)
    }
}

impl From<T5_Vec3> for Vec3 {
    fn from(val: T5_Vec3) -> Self {
        Vec3::new(val.x, val.y, val.z)
    }
}

impl From<T5_Quat> for Quat {
    fn from(val: T5_Quat) -> Self {
        Quat::from_xyzw(val.x, val.y, val.z, val.w)
    }
}

impl From<Vec2> for T5_Vec2 {
    fn from(val: Vec2) -> Self {
        T5_Vec2 { x: val.x, y: val.y }
    }
}

impl From<Vec3> for T5_Vec3 {
    fn from(val: Vec3) -> Self {
        T5_Vec3 {
            x: val.x,
            y: val.y,
            z: val.z,
        }
    }
}

impl From<Quat> for T5_Quat {
    fn from(val: Quat) -> Self {
        T5_Quat {
            x: val.x,
            y: val.y,
            z: val.z,
            w: val.w,
        }
    }
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
