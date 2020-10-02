use core::hash::BuildHasher;

use glyph_brush::ab_glyph::Font;
use glyph_brush::delegate_glyph_brush_builder_fns;
use glyph_brush::DefaultSectionHasher;
use winapi::um::d3d11::{
    ID3D11Device, D3D11_DEPTH_STENCIL_DESC, D3D11_FILTER, D3D11_FILTER_MIN_MAG_MIP_LINEAR,
};
use wio::com::ComPtr;

use crate::util::HResult;

use super::GlyphBrush;

/// Builder for a [`GlyphBrush`](struct.GlyphBrush.html).
pub struct GlyphBrushBuilder<D, F, H = DefaultSectionHasher> {
    inner: glyph_brush::GlyphBrushBuilder<F, H>,
    texture_filter_method: D3D11_FILTER,
    depth: D,
}

impl<F, H> From<glyph_brush::GlyphBrushBuilder<F, H>> for GlyphBrushBuilder<(), F, H> {
    fn from(inner: glyph_brush::GlyphBrushBuilder<F, H>) -> Self {
        GlyphBrushBuilder {
            inner,
            texture_filter_method: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            depth: (),
        }
    }
}

impl GlyphBrushBuilder<(), ()> {
    /// Specifies the default font used to render glyphs.
    /// Referenced with `FontId(0)`, which is default.
    #[inline]
    pub fn using_font<F: Font>(font: F) -> GlyphBrushBuilder<(), F> {
        GlyphBrushBuilder {
            inner: glyph_brush::GlyphBrushBuilder::using_font(font),
            texture_filter_method: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            depth: (),
        }
    }

    /// Create a new builder with multiple fonts.
    pub fn using_fonts<F: Font>(fonts: Vec<F>) -> GlyphBrushBuilder<(), F> {
        GlyphBrushBuilder {
            inner: glyph_brush::GlyphBrushBuilder::using_fonts(fonts),
            texture_filter_method: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            depth: (),
        }
    }

    /// Create a new builder without any fonts.
    pub fn without_fonts() -> GlyphBrushBuilder<(), ()> {
        GlyphBrushBuilder {
            inner: glyph_brush::GlyphBrushBuilder::without_fonts(),
            texture_filter_method: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            depth: (),
        }
    }
}

impl<F: Font, D, H: BuildHasher> GlyphBrushBuilder<D, F, H> {
    delegate_glyph_brush_builder_fns!(inner);

    /// Sets the texture filtering method.
    pub fn texture_filter_method(mut self, filter_method: D3D11_FILTER) -> Self {
        self.texture_filter_method = filter_method;
        self
    }

    /// Sets the section hasher. `GlyphBrush` cannot handle absolute section
    /// hash collisions so use a good hash algorithm.
    ///
    /// This hasher is used to distinguish sections, rather than for hashmap
    /// internal use.
    ///
    /// Defaults to [seahash](https://docs.rs/seahash).
    pub fn section_hasher<T: BuildHasher>(self, section_hasher: T) -> GlyphBrushBuilder<D, F, T> {
        GlyphBrushBuilder {
            inner: self.inner.section_hasher(section_hasher),
            texture_filter_method: self.texture_filter_method,
            depth: self.depth,
        }
    }

    pub fn depth_stencil_state(
        self,
        depth_stencil: D3D11_DEPTH_STENCIL_DESC,
    ) -> GlyphBrushBuilder<D3D11_DEPTH_STENCIL_DESC, F, H> {
        GlyphBrushBuilder {
            inner: self.inner,
            texture_filter_method: self.texture_filter_method,
            depth: depth_stencil,
        }
    }
}

impl<F: Font, H: BuildHasher> GlyphBrushBuilder<(), F, H> {
    /// Builds a `GlyphBrush` using the given `ID3D11Device`.
    pub fn build(self, device: ComPtr<ID3D11Device>) -> HResult<GlyphBrush<(), F, H>> {
        GlyphBrush::<(), F, H>::new(device, self.texture_filter_method, self.inner)
    }
}

impl<F: Font, H: BuildHasher> GlyphBrushBuilder<D3D11_DEPTH_STENCIL_DESC, F, H> {
    /// Builds a `GlyphBrush` using the given `ID3D11Device`.
    pub fn build(
        self,
        device: ComPtr<ID3D11Device>,
    ) -> HResult<GlyphBrush<D3D11_DEPTH_STENCIL_DESC, F, H>> {
        GlyphBrush::<D3D11_DEPTH_STENCIL_DESC, F, H>::new(
            device,
            self.texture_filter_method,
            self.depth,
            self.inner,
        )
    }
}
