use std::{mem, ptr};

use glyph_brush::Rectangle;
use winapi::shared::dxgiformat::DXGI_FORMAT_R8_UNORM;
use winapi::shared::dxgitype::DXGI_SAMPLE_DESC;
use winapi::um::d3d11::{
    ID3D11Device, ID3D11DeviceContext, ID3D11ShaderResourceView, ID3D11Texture2D,
    D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_SHADER_RESOURCE_VIEW_DESC, D3D11_TEX2D_SRV,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
};
use winapi::um::d3dcommon::D3D11_SRV_DIMENSION_TEXTURE2D;
use wio::com::ComPtr;

use crate::util::{com_ptr_from_fn, com_ref_cast, HResult};

pub struct Cache {
    texture: ComPtr<ID3D11Texture2D>,
    view: ComPtr<ID3D11ShaderResourceView>,
}

impl Cache {
    pub fn new(device: &ID3D11Device, width: u32, height: u32) -> HResult<Cache> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let texture = unsafe {
            com_ptr_from_fn(|texture| device.CreateTexture2D(&desc, ptr::null(), texture))?
        };

        let view = unsafe {
            com_ptr_from_fn(|font_texture_view| {
                let mut desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
                    Format: DXGI_FORMAT_R8_UNORM,
                    ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
                    u: mem::zeroed(),
                };
                *desc.u.Texture2D_mut() = D3D11_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                };
                device.CreateShaderResourceView(
                    com_ref_cast(&texture).as_raw(),
                    &desc,
                    font_texture_view,
                )
            })?
        };

        Ok(Cache { texture, view })
    }

    pub fn update(&mut self, ctx: &ID3D11DeviceContext, rect: Rectangle<u32>, data: &[u8]) {
        unsafe {
            ctx.UpdateSubresource(
                com_ref_cast(&self.texture).as_raw(),
                0,
                &D3D11_BOX {
                    left: rect.min[0],
                    right: rect.max[0],
                    top: rect.min[1],
                    bottom: rect.max[1],
                    front: 0,
                    back: 1,
                },
                data.as_ptr().cast(),
                rect.width(),
                rect.width() * rect.height(),
            );
        }
    }

    pub fn view(&self) -> *mut ID3D11ShaderResourceView {
        self.view.as_raw()
    }
}
