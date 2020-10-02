use d3d11_glyph::{ab_glyph, GlyphBrushBuilder, Section, Text};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use winapi::shared::windef::HWND;
use winapi::shared::winerror::S_OK;
use winapi::Interface as _;

use winapi::shared::dxgi::*;
use winapi::shared::dxgiformat::*;
use winapi::shared::dxgitype::*;

use winapi::um::d3d11::*;
use winapi::um::d3dcommon::*;

use winapi::shared::minwindef::TRUE;
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

use wio::com::ComPtr;

use std::ptr;
use std::time::Instant;

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
) -> ComPtr<ID3D11RenderTargetView> {
    let mut back_buffer = ptr::null_mut::<ID3D11Texture2D>();
    let mut main_rtv = ptr::null_mut();
    swapchain.GetBuffer(
        0,
        &ID3D11Resource::uuidof(),
        &mut back_buffer as *mut *mut _ as *mut *mut _,
    );
    device.CreateRenderTargetView(back_buffer.cast(), ptr::null_mut(), &mut main_rtv);
    (&*back_buffer).Release();
    ComPtr::from_raw(main_rtv)
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
    let mut main_rtv = unsafe { create_render_target(&swapchain, &device) };

    let mut size = window.inner_size();
    let clear_color = [0.45, 0.55, 0.60, 1.00];

    let inconsolata =
        ab_glyph::FontArc::try_from_slice(include_bytes!("Inconsolata-Regular.ttf")).unwrap();

    let mut glyph_brush = GlyphBrushBuilder::using_font(inconsolata)
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
            }

            glyph_brush.queue(Section {
                screen_position: (30.0, 30.0),
                bounds: (size.width as f32, size.height as f32),
                text: vec![Text::new("Hello d3d11-glyph!")
                    .with_color([0.0, 0.0, 0.0, 1.0])
                    .with_scale(40.0)],
                ..Section::default()
            });

            glyph_brush.queue(Section {
                screen_position: (30.0, 90.0),
                bounds: (size.width as f32, size.height as f32),
                text: vec![Text::new("Hello d3d11-glyph!")
                    .with_color([1.0, 1.0, 1.0, 1.0])
                    .with_scale(40.0)],
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
                .draw_queued(size.width, size.height)
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
            ptr::write(&mut main_rtv, create_render_target(&swapchain, &device));
        },
        Event::LoopDestroyed => (),
        _ => {}
    });
}
