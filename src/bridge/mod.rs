mod ffi {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use ffi::*;

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;

    use super::ffi::*;


    #[test]
    fn can_create_context() {
        unsafe {
            let bridge = TiltFiveNative::new("TiltFiveNative.dll").unwrap();
            let mut ctx = MaybeUninit::uninit();
            let info = T5_ClientInfo {
                applicationId: "test".as_ptr() as *const i8,
                applicationVersion: "1".as_ptr() as *const i8,
                sdkType: 0u8,
                reserved: 0u64,
            };
            let err = bridge.t5CreateContext(ctx.as_mut_ptr(), &info, 0i64 as *const i64);

            assert_eq!(err, 0);
        }
    }

    #[test]
    fn can_get_gameboard_size() {
        
        unsafe {
            let bridge = TiltFiveNative::new("TiltFiveNative.dll").unwrap();
            let mut ctx = MaybeUninit::uninit();
            let info = T5_ClientInfo {
                applicationId: "test".as_ptr() as *const i8,
                applicationVersion: "1".as_ptr() as *const i8,
                sdkType: 0u8,
                reserved: 0u64,
            };
            let err = bridge.t5CreateContext(ctx.as_mut_ptr(), &info, 0i64 as *const i64);

            let mut ctx = ctx.assume_init();

            let mut gameboard = MaybeUninit::uninit();

            let err = bridge.t5GetGameboardSize(ctx, T5_GameboardType_kT5_GameboardType_LE, gameboard.as_mut_ptr());
            assert_eq!(err, 0);
            let val = gameboard.assume_init();
            assert_eq!(val.viewableExtentNegativeX, 0.35);
            assert_eq!(val.viewableExtentPositiveX, 0.35);
            assert_eq!(val.viewableExtentPositiveY, 0.35);
            assert_eq!(val.viewableExtentNegativeY, 0.35);
            assert_eq!(val.viewableExtentPositiveZ, 0.0);
        }
    }
}
