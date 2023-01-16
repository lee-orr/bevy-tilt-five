
use std::mem::{MaybeUninit};

use std::sync::mpsc::{channel, Receiver};

use anyhow::{Result};

// use beryllium::*;
use gl33::global_loader::*;

use bevy::asset::FileAssetIo;

use bevy::render::renderer::RenderDevice;
use bevy::render::RenderStage;
use bevy::{prelude::*, render::RenderApp};


use crate::bridge::ffi::{T5_Quat, T5_Vec3};
use crate::bridge::{
    Glasses, DEFAULT_GLASSES_HEIGHT, DEFAULT_GLASSES_WIDTH,
};
use crate::{BufferSender, T5ClientRenderApp, TEXTURE_FORMAT};

pub struct OGLPlugin;

impl Plugin for OGLPlugin {
    fn build(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);

        let (sender, receiver) = channel();

        render_app
            .insert_non_send_resource(OLGDeviceResource {
                devices: None,
                receiver,
            })
            .insert_non_send_resource(BufferSender { sender })
            .add_system_to_stage(RenderStage::Prepare, setup_ogl_interface)
            .add_system_to_stage(RenderStage::Extract, send_frames);
    }
}

fn create_ogl_device() -> Result<OGLDevices> {
    // let sdl = Sdl::init(init::InitFlags::EVERYTHING);

    // let win = sdl.create_gl_window(video::CreateWinArgs {
    //     title: "T5 View",
    //     width: 240,
    //     height: 240,
    //     allow_high_dpi: false,
    //     borderless: false,
    //     resizable: false,
    // });

    Ok(OGLDevices {})
    // unsafe {
    //     let mut device: MaybeUninit<*mut ID3D11Device> = MaybeUninit::uninit();
    //     let mut context: MaybeUninit<*mut ID3D11DeviceContext> = MaybeUninit::uninit();
    //     let instance: HINSTANCE = null::<HINSTANCE__>().cast_mut();
    //     let result = winapi::um::d3d11::D3D11CreateDevice(
    //         null::<IDXGIAdapter>().cast_mut(),
    //         winapi::um::d3dcommon::D3D_DRIVER_TYPE_HARDWARE,
    //         instance,
    //         D3D11_CREATE_DEVICE_SINGLETHREADED,
    //         null(),
    //         0,
    //         D3D11_SDK_VERSION,
    //         device.as_mut_ptr(),
    //         null::<u32>().cast_mut(),
    //         context.as_mut_ptr(),
    //     );
    //     if result != 0 {
    //         bail!("Error Creating DX11 {result}");
    //     }
    //     Ok(OGLDevices {
    //         device: device.assume_init(),
    //         context: context.assume_init(),
    //     })
    // }
}

struct OLGDeviceResource {
    devices: Option<OGLDevices>,
    receiver: Receiver<(Glasses, Vec<u8>, Vec<u8>, T5_Vec3, T5_Vec3, T5_Quat)>,
}

struct OGLDevices {
    // device: *mut ID3D11Device,
    // context: *mut ID3D11DeviceContext,
}

fn setup_ogl_interface(
    _device: Res<RenderDevice>,
    _oglresource: NonSendMut<OLGDeviceResource>,
    _client: NonSendMut<T5ClientRenderApp>,
) {
    // if oglresource.devices.is_none() {
    //     if let Ok(devices) = create_ogl_device() {
    //         oglresource.devices = Some(devices);
    //     } else {
    //         error!("Couldn't setup opengl device...");
    //     }
    // }
}

fn send_frames(
    resource: NonSendMut<OLGDeviceResource>,
    _client: NonSendMut<T5ClientRenderApp>,
) {
    let fmt = TEXTURE_FORMAT.describe();
    let _bytes_per_row =
        DEFAULT_GLASSES_WIDTH * (fmt.block_dimensions.0 as u32) * (fmt.block_size as u32);

    // if let Some(device) = &resource.devices {
    while let Ok((glasses, left, right, _lpos, _rpos, _rot)) = resource.receiver.try_recv() {
        save_frame_to_file(&left, &glasses, "left");
        save_frame_to_file(&right, &glasses, "right");

        unsafe {
            let mut textures = MaybeUninit::uninit();

            glGenTextures(2, textures.as_mut_ptr());
        }

        // unsafe {
        //     let mut left_tex = MaybeUninit::uninit();
        //     let mut right_tex = MaybeUninit::uninit();

        //     let description = D3D11_TEXTURE2D_DESC {
        //         Width: DEFAULT_GLASSES_WIDTH,
        //         Height: DEFAULT_GLASSES_HEIGHT,
        //         MipLevels: 1,
        //         ArraySize: 1,
        //         Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        //         SampleDesc: DXGI_SAMPLE_DESC {
        //             Count: 1,
        //             Quality: 0,
        //         },
        //         Usage: D3D11_USAGE_DEFAULT,
        //         BindFlags: D3D11_BIND_SHADER_RESOURCE,
        //         CPUAccessFlags: D3D11_CPU_ACCESS_READ,
        //         MiscFlags: 0,
        //     };

        //     let left_data = D3D11_SUBRESOURCE_DATA {
        //         pSysMem: left.as_ptr() as *const c_void,
        //         SysMemPitch: bytes_per_row,
        //         SysMemSlicePitch: 0,
        //     };

        //     let right_data = D3D11_SUBRESOURCE_DATA {
        //         pSysMem: right.as_ptr() as *const c_void,
        //         SysMemPitch: bytes_per_row,
        //         SysMemSlicePitch: 0,
        //     };

        //     let desc = MaybeUninit::new(description);
        //     let ldata = MaybeUninit::new(left_data);
        //     let rdata = MaybeUninit::new(right_data);

        //     if let Some(device) = device.device.as_ref() {
        //         device.CreateTexture2D(desc.as_ptr(), ldata.as_ptr(), left_tex.as_mut_ptr());
        //         device.CreateTexture2D(desc.as_ptr(), rdata.as_ptr(), right_tex.as_mut_ptr());
        //     }

        //     let start_y_vci =
        //         -1.0 * (DEFAULT_GLASSES_FOV * 0.5 * std::f32::consts::PI / 180.).tan();
        //     let start_x_vci =
        //         start_y_vci * (DEFAULT_GLASSES_WIDTH as f32 / DEFAULT_GLASSES_HEIGHT as f32);
        //     let width_vci = -2.0 * start_x_vci;
        //     let height_vci = -2.0 * start_y_vci;

        //     let frame_info = bridge::ffi::T5_FrameInfo {
        //         leftTexHandle: left_tex.assume_init() as *mut c_void,
        //         rightTexHandle: right_tex.assume_init() as *mut c_void,
        //         texWidth_PIX: DEFAULT_GLASSES_WIDTH as u16,
        //         texHeight_PIX: DEFAULT_GLASSES_HEIGHT as u16,
        //         isSrgb: false,
        //         isUpsideDown: false,
        //         rotToLVC_GBD: rot.clone(),
        //         posLVC_GBD: lpos,
        //         rotToRVC_GBD: rot,
        //         posRVC_GBD: rpos,
        //         vci: T5_FrameInfo__bindgen_ty_1 {
        //             startX_VCI: start_x_vci,
        //             startY_VCI: start_y_vci,
        //             width_VCI: width_vci,
        //             height_VCI: height_vci,
        //         },
        //     };

        //     let info = MaybeUninit::new(frame_info);

        //     let _ = client.client.send_frame_to_glasses(&glasses, info.as_ptr());
        // }
    }
    // }
}

fn save_frame_to_file(data: &Vec<u8>, _glasses: &Glasses, eye: &str) {
    let mut path = FileAssetIo::get_base_path();

    path.pop();
    path.push(format!("capture_{eye}.png"));
    info!("Capture Path: {path:?}");
    if let Some(_buffer) = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
        DEFAULT_GLASSES_WIDTH,
        DEFAULT_GLASSES_HEIGHT,
        data.clone(),
    ) {
        // let _ = buffer.save(path);
    } else {
        error!("Failed to save image");
    }
}
