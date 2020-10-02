use std::convert::TryInto;
use std::marker::PhantomData;
use std::{mem, ptr};

use glyph_brush::Rectangle;
use winapi::shared::dxgiformat::{
    DXGI_FORMAT_R32G32B32A32_FLOAT, DXGI_FORMAT_R32G32B32_FLOAT, DXGI_FORMAT_R32G32_FLOAT,
};
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::um::d3d11::{
    ID3D11BlendState, ID3D11Buffer, ID3D11DepthStencilState, ID3D11Device, ID3D11DeviceContext,
    ID3D11InputLayout, ID3D11PixelShader, ID3D11RasterizerState, ID3D11SamplerState,
    ID3D11VertexShader, D3D11_BLEND_DESC, D3D11_BUFFER_DESC, D3D11_DEPTH_STENCILOP_DESC,
    D3D11_DEPTH_STENCIL_DESC, D3D11_FILTER, D3D11_INPUT_ELEMENT_DESC, D3D11_RASTERIZER_DESC,
    D3D11_RECT, D3D11_RENDER_TARGET_BLEND_DESC, D3D11_SAMPLER_DESC, D3D11_SUBRESOURCE_DATA,
};
use winapi::um::d3d11::{
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_VERTEX_BUFFER, D3D11_BLEND_INV_SRC_ALPHA,
    D3D11_BLEND_ONE, D3D11_BLEND_OP_ADD, D3D11_BLEND_SRC_ALPHA, D3D11_COLOR_WRITE_ENABLE_ALL,
    D3D11_COMPARISON_ALWAYS, D3D11_CPU_ACCESS_WRITE, D3D11_CULL_NONE, D3D11_DEPTH_WRITE_MASK_ALL,
    D3D11_FILL_SOLID, D3D11_INPUT_PER_INSTANCE_DATA, D3D11_MAP_WRITE_DISCARD,
    D3D11_STENCIL_OP_KEEP, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DYNAMIC,
};
use winapi::um::d3dcommon::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP;
use wio::com::ComPtr;

use crate::cache::Cache;
use crate::util::{com_ptr_from_fn, com_ref_cast, hresult, HResult};

#[derive(Debug)]
struct Buffer {
    ptr: ComPtr<ID3D11Buffer>,
    capacity: usize,
    len: usize,
}

pub struct Pipeline<Depth> {
    device: ComPtr<ID3D11Device>,
    ctx: ComPtr<ID3D11DeviceContext>,
    vertex_buffer: Buffer,
    transform_buf: ComPtr<ID3D11Buffer>,
    transform: [f32; 16],
    sampler: ComPtr<ID3D11SamplerState>,
    cache: Cache,
    blend_state: ComPtr<ID3D11BlendState>,
    rasterizer_state: ComPtr<ID3D11RasterizerState>,
    depth_stencil_state: ComPtr<ID3D11DepthStencilState>,
    input_layout: ComPtr<ID3D11InputLayout>,
    pixel_shader: ComPtr<ID3D11PixelShader>,
    vertex_shader: ComPtr<ID3D11VertexShader>,
    _pd: PhantomData<Depth>,
}

impl Pipeline<()> {
    #[inline]
    pub fn new(
        device: ComPtr<ID3D11Device>,
        filter_mode: D3D11_FILTER,
        cache_width: u32,
        cache_height: u32,
    ) -> HResult<Pipeline<()>> {
        unsafe { build(device, filter_mode, None, cache_width, cache_height) }
    }

    #[inline]
    pub fn draw(&mut self, transform: [f32; 16], rect: Option<D3D11_RECT>) -> HResult<()> {
        unsafe { draw(self, transform, rect) }
    }
}

impl<Depth> Pipeline<Depth> {
    #[inline]
    pub fn update_cache(&mut self, rect: Rectangle<u32>, data: &[u8]) {
        self.cache.update(&self.ctx, rect, data);
    }

    #[inline]
    pub fn increase_cache_size(&mut self, width: u32, height: u32) {
        self.cache = Cache::new(&self.device, width, height).unwrap();
    }

    pub fn upload(&mut self, vertices: &[Vertex]) -> HResult<()> {
        if vertices.is_empty() {
            self.vertex_buffer.len = 0;
            return Ok(());
        }

        if vertices.len() > self.vertex_buffer.capacity {
            self.vertex_buffer =
                unsafe { Self::create_vertex_buffer(&self.device, vertices.len())? };
        }

        unsafe {
            let vtx_resource = {
                let mut vtx_resource = mem::MaybeUninit::zeroed();
                hresult(self.ctx.Map(
                    com_ref_cast(&self.vertex_buffer.ptr).as_raw(),
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    vtx_resource.as_mut_ptr(),
                ))?;
                vtx_resource.assume_init()
            };
            ptr::copy_nonoverlapping(
                vertices.as_ptr(),
                vtx_resource.pData.cast::<Vertex>(),
                vertices.len(),
            );
            self.ctx.Unmap(self.vertex_buffer.ptr.as_raw().cast(), 0);
        }
        self.vertex_buffer.len = vertices.len();
        Ok(())
    }

    unsafe fn create_vertex_buffer(device: &ID3D11Device, capacity: usize) -> HResult<Buffer> {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: (capacity * mem::size_of::<Vertex>()).try_into().unwrap(),
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_VERTEX_BUFFER,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        com_ptr_from_fn(|vertex_buffer| device.CreateBuffer(&desc, ptr::null(), vertex_buffer)).map(
            |vb| Buffer {
                ptr: vb,
                capacity,
                len: 0,
            },
        )
    }
}

#[rustfmt::skip]
const IDENTITY_MATRIX: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0,
];

unsafe fn build<D>(
    device: ComPtr<ID3D11Device>,
    filter_mode: D3D11_FILTER,
    depth_stencil_state: Option<()>,
    cache_width: u32,
    cache_height: u32,
) -> HResult<Pipeline<D>> {
    let context = {
        let mut context = ptr::null_mut();
        device.GetImmediateContext(&mut context);
        ComPtr::from_raw(context)
    };

    let mut desc = D3D11_BLEND_DESC {
        AlphaToCoverageEnable: FALSE,
        IndependentBlendEnable: FALSE,
        RenderTarget: std::mem::zeroed(),
    };
    desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: TRUE,
        SrcBlend: D3D11_BLEND_SRC_ALPHA,
        DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOp: D3D11_BLEND_OP_ADD,
        SrcBlendAlpha: D3D11_BLEND_ONE,
        DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOpAlpha: D3D11_BLEND_OP_ADD,
        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL as u8,
    };
    let blend_state = com_ptr_from_fn(|blend_state| device.CreateBlendState(&desc, blend_state))?;

    let desc = D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_NONE,
        FrontCounterClockwise: 0,
        DepthBias: 0,
        DepthBiasClamp: 0.0,
        SlopeScaledDepthBias: 0.0,
        DepthClipEnable: FALSE,
        ScissorEnable: FALSE,
        MultisampleEnable: 0,
        AntialiasedLineEnable: 0,
    };
    let rasterizer_state =
        com_ptr_from_fn(|rasterizer_state| device.CreateRasterizerState(&desc, rasterizer_state))?;

    let stencil_op_desc = D3D11_DEPTH_STENCILOP_DESC {
        StencilFailOp: D3D11_STENCIL_OP_KEEP,
        StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
        StencilPassOp: D3D11_STENCIL_OP_KEEP,
        StencilFunc: D3D11_COMPARISON_ALWAYS,
    };
    let desc = D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: FALSE,
        DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
        DepthFunc: D3D11_COMPARISON_ALWAYS,
        StencilEnable: FALSE,
        StencilReadMask: 0,
        StencilWriteMask: 0,
        FrontFace: stencil_op_desc,
        BackFace: stencil_op_desc,
    };
    let depth_stencil_state = com_ptr_from_fn(|depth_stencil_state| {
        device.CreateDepthStencilState(&desc, depth_stencil_state)
    })?;

    let desc = D3D11_BUFFER_DESC {
        ByteWidth: mem::size_of::<[f32; 16]>() as _,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let transform_buf = com_ptr_from_fn(|vertex_constant_buffer| {
        let subresource = D3D11_SUBRESOURCE_DATA {
            pSysMem: IDENTITY_MATRIX.as_ptr().cast(),
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };
        device.CreateBuffer(&desc, &subresource, vertex_constant_buffer)
    })?;

    let desc = D3D11_SAMPLER_DESC {
        Filter: filter_mode,
        AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
        MipLODBias: 0.0,
        MaxAnisotropy: 0,
        ComparisonFunc: D3D11_COMPARISON_ALWAYS,
        BorderColor: [0.0; 4],
        MinLOD: 0.0,
        MaxLOD: 0.0,
    };
    let sampler = com_ptr_from_fn(|sampler| device.CreateSamplerState(&desc, sampler))?;

    let cache = Cache::new(&device, cache_width, cache_height)?;

    let vertices = Pipeline::<()>::create_vertex_buffer(&device, 1024)?;

    const VERTEX_SHADER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/vertex_shader.vs_4_0"));
    let vertex_shader = com_ptr_from_fn(|vs_shader| {
        device.CreateVertexShader(
            VERTEX_SHADER.as_ptr().cast(),
            VERTEX_SHADER.len(),
            ptr::null_mut(),
            vs_shader,
        )
    })?;

    let local_layout = [
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: "POSITION\0".as_ptr().cast(),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32B32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 0,
            InputSlotClass: D3D11_INPUT_PER_INSTANCE_DATA,
            InstanceDataStepRate: 1,
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: "POSITION\0".as_ptr().cast(),
            SemanticIndex: 1,
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 4 * 3,
            InputSlotClass: D3D11_INPUT_PER_INSTANCE_DATA,
            InstanceDataStepRate: 1,
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: "TEXCOORD\0".as_ptr().cast(),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 4 * (3 + 2),
            InputSlotClass: D3D11_INPUT_PER_INSTANCE_DATA,
            InstanceDataStepRate: 1,
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: "TEXCOORD\0".as_ptr().cast(),
            SemanticIndex: 1,
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 4 * (3 + 2 + 2),
            InputSlotClass: D3D11_INPUT_PER_INSTANCE_DATA,
            InstanceDataStepRate: 1,
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: "COLOR\0".as_ptr().cast(),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 4 * (3 + 2 + 2 + 2),
            InputSlotClass: D3D11_INPUT_PER_INSTANCE_DATA,
            InstanceDataStepRate: 1,
        },
    ];

    let input_layout = com_ptr_from_fn(|input_layout| {
        device.CreateInputLayout(
            local_layout.as_ptr(),
            local_layout.len() as _,
            VERTEX_SHADER.as_ptr().cast(),
            VERTEX_SHADER.len(),
            input_layout,
        )
    })?;

    const PIXEL_SHADER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pixel_shader.ps_4_0"));
    let pixel_shader = com_ptr_from_fn(|ps_shader| {
        device.CreatePixelShader(
            PIXEL_SHADER.as_ptr().cast(),
            PIXEL_SHADER.len(),
            ptr::null_mut(),
            ps_shader,
        )
    })?;

    Ok(Pipeline {
        device,
        ctx: context,
        blend_state,
        rasterizer_state,
        depth_stencil_state,
        vertex_buffer: vertices,
        transform_buf,
        transform: IDENTITY_MATRIX,
        cache,
        input_layout,
        sampler,
        vertex_shader,
        pixel_shader,
        _pd: PhantomData,
    })
}

unsafe fn draw<D>(
    pipeline: &mut Pipeline<D>,
    transform: [f32; 16],
    rect: Option<D3D11_RECT>,
) -> HResult<()> {
    let ctx = &*pipeline.ctx;
    #[allow(clippy::float_cmp)]
    if transform != pipeline.transform {
        let mut mapped_resource = mem::MaybeUninit::zeroed();
        hresult(ctx.Map(
            com_ref_cast(&pipeline.transform_buf).as_raw(),
            0,
            D3D11_MAP_WRITE_DISCARD,
            0,
            mapped_resource.as_mut_ptr(),
        ))?;
        let mapped_resource = mapped_resource.assume_init();

        // FIXME alignment?
        *mapped_resource.pData.cast::<[f32; 16]>() = transform;
        ctx.Unmap(com_ref_cast(&pipeline.transform_buf).as_raw(), 0);

        pipeline.transform = transform;
    }

    let stride = mem::size_of::<Vertex>() as u32;
    ctx.IASetInputLayout(pipeline.input_layout.as_raw());
    ctx.IASetVertexBuffers(0, 1, &pipeline.vertex_buffer.ptr.as_raw(), &stride, &0);
    ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
    ctx.VSSetShader(pipeline.vertex_shader.as_raw(), ptr::null(), 0);
    ctx.VSSetConstantBuffers(0, 1, &pipeline.transform_buf.as_raw());
    ctx.PSSetShader(pipeline.pixel_shader.as_raw(), ptr::null(), 0);
    ctx.PSSetSamplers(0, 1, &pipeline.sampler.as_raw());
    ctx.GSSetShader(ptr::null_mut(), ptr::null(), 0);
    ctx.HSSetShader(ptr::null_mut(), ptr::null(), 0);
    ctx.DSSetShader(ptr::null_mut(), ptr::null(), 0);
    ctx.CSSetShader(ptr::null_mut(), ptr::null(), 0);

    ctx.OMSetBlendState(pipeline.blend_state.as_raw(), &[0.0; 4], 0xFFFFFFFF);
    ctx.OMSetDepthStencilState(pipeline.depth_stencil_state.as_raw(), 0);
    ctx.RSSetState(pipeline.rasterizer_state.as_raw());

    ctx.PSSetShaderResources(0, 1, &pipeline.cache.view());

    if let Some(ref rect) = rect {
        ctx.RSSetScissorRects(1, rect);
    }

    ctx.DrawInstanced(4, pipeline.vertex_buffer.len as u32, 0, 0);
    Ok(())
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    left_top: [f32; 3],
    right_bottom: [f32; 2],
    tex_left_top: [f32; 2],
    tex_right_bottom: [f32; 2],
    color: [f32; 4],
}

impl<'gv> From<glyph_brush::GlyphVertex<'gv>> for Vertex {
    fn from(
        glyph_brush::GlyphVertex {
            mut tex_coords,
            mut pixel_coords,
            bounds,
            extra,
        }: glyph_brush::GlyphVertex,
    ) -> Self {
        // handle overlapping bounds, modify uv_rect to preserve texture aspect
        if pixel_coords.max.x > bounds.max.x {
            let old_width = pixel_coords.width();
            pixel_coords.max.x = bounds.max.x;
            tex_coords.max.x =
                tex_coords.min.x + tex_coords.width() * pixel_coords.width() / old_width;
        }

        if pixel_coords.min.x < bounds.min.x {
            let old_width = pixel_coords.width();
            pixel_coords.min.x = bounds.min.x;
            tex_coords.min.x =
                tex_coords.max.x - tex_coords.width() * pixel_coords.width() / old_width;
        }

        if pixel_coords.max.y > bounds.max.y {
            let old_height = pixel_coords.height();
            pixel_coords.max.y = bounds.max.y;
            tex_coords.max.y =
                tex_coords.min.y + tex_coords.height() * pixel_coords.height() / old_height;
        }

        if pixel_coords.min.y < bounds.min.y {
            let old_height = pixel_coords.height();
            pixel_coords.min.y = bounds.min.y;
            tex_coords.min.y =
                tex_coords.max.y - tex_coords.height() * pixel_coords.height() / old_height;
        }

        Vertex {
            left_top: [pixel_coords.min.x, pixel_coords.max.y, extra.z],
            right_bottom: [pixel_coords.max.x, pixel_coords.min.y],
            tex_left_top: [tex_coords.min.x, tex_coords.max.y],
            tex_right_bottom: [tex_coords.max.x, tex_coords.min.y],
            color: extra.color,
        }
    }
}
