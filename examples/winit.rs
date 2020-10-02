use std::time::Instant;
use std::{mem, ptr};

use d3d11_glyph::{ab_glyph, GlyphBrushBuilder, Section, Text};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use winapi::shared::dxgi::*;
use winapi::shared::dxgiformat::*;
use winapi::shared::dxgitype::*;
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::shared::windef::HWND;
use winapi::shared::winerror::S_OK;

use winapi::um::d3d11::*;
use winapi::um::d3dcommon::*;

use winapi::Interface as _;

use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};
use wio::com::ComPtr;

const WINDOW_WIDTH: f64 = 760.0;
const WINDOW_HEIGHT: f64 = 760.0;

unsafe fn create_device(
    hwnd: HWND,
) -> Option<(
    ComPtr<IDXGISwapChain>,
    ComPtr<ID3D11Device>,
    ComPtr<ID3D11DeviceContext>,
)> {
    let sc_desc = DXGI_SWAP_CHAIN_DESC {
        BufferDesc: DXGI_MODE_DESC {
            Width: 0,
            Height: 0,
            RefreshRate: DXGI_RATIONAL {
                Numerator: 60,
                Denominator: 1,
            },
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            ScanlineOrdering: 0,
            Scaling: 0,
        },
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 3,
        OutputWindow: hwnd,
        Windowed: TRUE,
        SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
        Flags: DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH,
    };

    let mut swapchain = ptr::null_mut();
    let mut device = ptr::null_mut();
    let mut context = ptr::null_mut();

    let mut feature_level = 0;
    let feature_levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_10_0];
    if D3D11CreateDeviceAndSwapChain(
        ptr::null_mut(),
        D3D_DRIVER_TYPE_HARDWARE,
        ptr::null_mut(),
        0,
        feature_levels.as_ptr(),
        feature_levels.len() as u32,
        D3D11_SDK_VERSION,
        &sc_desc,
        &mut swapchain,
        &mut device,
        &mut feature_level,
        &mut context,
    ) != S_OK
    {
        None
    } else {
        Some((
            ComPtr::from_raw(swapchain),
            ComPtr::from_raw(device),
            ComPtr::from_raw(context),
        ))
    }
}

unsafe fn create_render_target(
    swapchain: &ComPtr<IDXGISwapChain>,
    device: &ComPtr<ID3D11Device>,
) -> (
    ComPtr<ID3D11RenderTargetView>,
    ComPtr<ID3D11DepthStencilView>,
) {
    let mut back_buffer = ptr::null_mut::<ID3D11Texture2D>();
    let mut main_rtv = ptr::null_mut();
    swapchain.GetBuffer(
        0,
        &ID3D11Resource::uuidof(),
        &mut back_buffer as *mut *mut _ as *mut *mut _,
    );
    device.CreateRenderTargetView(back_buffer.cast(), ptr::null_mut(), &mut main_rtv);

    let mut bb_desc = mem::zeroed();
    (*back_buffer).GetDesc(&mut bb_desc);
    (*back_buffer).Release();

    let desc = D3D11_TEXTURE2D_DESC {
        Width: bb_desc.Width,
        Height: bb_desc.Height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R24G8_TYPELESS,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_DEPTH_STENCIL,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut depth_stencil = ptr::null_mut();
    device.CreateTexture2D(&desc, ptr::null(), &mut depth_stencil);

    let mut dsv_desc = D3D11_DEPTH_STENCIL_VIEW_DESC {
        Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
        ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
        Flags: 0,
        u: mem::zeroed(),
    };
    dsv_desc.u.Texture2D_mut().MipSlice = 0;
    let mut dsv = ptr::null_mut();
    device.CreateDepthStencilView(depth_stencil.cast(), &dsv_desc, &mut dsv);
    (ComPtr::from_raw(main_rtv), ComPtr::from_raw(dsv))
}

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("d3d11-glyph winit example")
        .with_inner_size(LogicalSize {
            width: WINDOW_WIDTH,
            height: WINDOW_HEIGHT,
        })
        .build(&event_loop)
        .unwrap();
    let hwnd = if let RawWindowHandle::Windows(handle) = window.raw_window_handle() {
        handle.hwnd
    } else {
        unreachable!()
    };
    let (swapchain, device, context) = unsafe { create_device(hwnd.cast()) }.unwrap();
    let (mut main_rtv, mut depth_stencil) = unsafe { create_render_target(&swapchain, &device) };

    let mut size = window.inner_size();
    let clear_color = [0.45, 0.55, 0.60, 1.00];

    let inconsolata =
        ab_glyph::FontArc::try_from_slice(include_bytes!("Inconsolata-Regular.ttf")).unwrap();

    let mut glyph_brush = GlyphBrushBuilder::using_font(inconsolata)
        .depth_stencil_state(D3D11_DEPTH_STENCIL_DESC {
            DepthEnable: TRUE,
            DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
            DepthFunc: D3D11_COMPARISON_GREATER,
            StencilEnable: FALSE,
            StencilReadMask: 0,
            StencilWriteMask: 0,
            FrontFace: D3D11_DEPTH_STENCILOP_DESC {
                StencilFailOp: D3D11_STENCIL_OP_KEEP,
                StencilDepthFailOp: D3D11_STENCIL_OP_INCR,
                StencilPassOp: D3D11_STENCIL_OP_KEEP,
                StencilFunc: D3D11_COMPARISON_ALWAYS,
            },
            BackFace: D3D11_DEPTH_STENCILOP_DESC {
                StencilFailOp: D3D11_STENCIL_OP_KEEP,
                StencilDepthFailOp: D3D11_STENCIL_OP_DECR,
                StencilPassOp: D3D11_STENCIL_OP_KEEP,
                StencilFunc: D3D11_COMPARISON_ALWAYS,
            },
        })
        .build(device.clone())
        .unwrap();

    let mut _last_frame = Instant::now();

    event_loop.run(move |event, _, control_flow| match event {
        Event::NewEvents(_) => {
            _last_frame = Instant::now();
        }
        Event::MainEventsCleared => {
            window.request_redraw();
        }
        Event::RedrawRequested(_) => {
            unsafe {
                context.OMSetRenderTargets(1, &main_rtv.as_raw(), ptr::null_mut());
                context.ClearRenderTargetView(main_rtv.as_raw(), &clear_color);
                context.ClearDepthStencilView(depth_stencil.as_raw(), D3D11_CLEAR_DEPTH, 0.0, 0);
            }

            glyph_brush.queue(Section {
                screen_position: (30.0, 30.0),
                text: vec![Text::default()
                    .with_text("On top")
                    .with_scale(95.0)
                    .with_color([0.8, 0.8, 0.8, 1.0])
                    .with_z(0.9)],
                ..Section::default()
            });

            glyph_brush.queue(Section {
                bounds: (size.width as f32, size.height as f32),
                text: vec![Text::default()
                    .with_text(&"da ".repeat(500))
                    .with_scale(30.0)
                    .with_color([0.05, 0.05, 0.1, 1.0])
                    .with_z(0.2)],
                ..Section::default()
            });
            let vp = D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: size.width as f32,
                Height: size.height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            };
            unsafe { context.RSSetViewports(1, &vp) };
            // Draw the text!
            glyph_brush
                .draw_queued(&main_rtv, &depth_stencil, size.width, size.height)
                .expect("Draw queued");

            unsafe {
                swapchain.Present(1, 0);
            }
        }
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *control_flow = winit::event_loop::ControlFlow::Exit,
        Event::WindowEvent {
            event: WindowEvent::Resized(winit::dpi::PhysicalSize { width, height }),
            ..
        } => unsafe {
            size = winit::dpi::PhysicalSize { width, height };
            ptr::drop_in_place(&mut main_rtv);
            swapchain.ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, 0);
            let (new_rtv, new_depth_stencil) = create_render_target(&swapchain, &device);
            ptr::write(&mut main_rtv, new_rtv);
            ptr::write(&mut depth_stencil, new_depth_stencil);
        },
        Event::LoopDestroyed => (),
        _ => {}
    });
}
