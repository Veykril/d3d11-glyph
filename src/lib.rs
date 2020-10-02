pub use builder::GlyphBrushBuilder;
pub use glyph_brush::ab_glyph;
pub use glyph_brush::{
    BuiltInLineBreaker, Extra, FontId, GlyphCruncher, GlyphPositioner, HorizontalAlign, Layout,
    LineBreak, LineBreaker, Section, SectionGeometry, SectionGlyph, SectionGlyphIter, SectionText,
    Text, VerticalAlign,
};
use util::HResult;

use std::borrow::Cow;
use std::hash::BuildHasher;

use ab_glyph::{Font, Rect};
use glyph_brush::{BrushAction, BrushError, DefaultSectionHasher};
use pipeline::{Pipeline, Vertex};
use winapi::um::d3d11::{
    ID3D11Device, D3D11_FILTER, D3D11_RECT, D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION,
};
use wio::com::ComPtr;

mod builder;
mod cache;
mod pipeline;
mod util;

pub struct GlyphBrush<Depth, F = ab_glyph::FontArc, H = DefaultSectionHasher> {
    pipeline: Pipeline<Depth>,
    glyph_brush: glyph_brush::GlyphBrush<Vertex, Extra, F, H>,
}

impl<Depth, F: Font, H: BuildHasher> GlyphBrush<Depth, F, H> {
    /// Queues a section/layout to be processed by the next call of
    /// [`process_queued`](struct.GlyphBrush.html#method.process_queued). Can be called multiple
    /// times to queue multiple sections for drawing.
    ///
    /// Benefits from caching, see [caching behaviour](#caching-behaviour).
    #[inline]
    pub fn queue<'a, S>(&mut self, section: S)
    where
        S: Into<Cow<'a, Section<'a>>>,
    {
        self.glyph_brush.queue(section)
    }

    /// Queues a section/layout to be processed by the next call of
    /// [`process_queued`](struct.GlyphBrush.html#method.process_queued). Can be called multiple
    /// times to queue multiple sections for drawing.
    ///
    /// Used to provide custom `GlyphPositioner` logic, if using built-in
    /// [`Layout`](enum.Layout.html) simply use [`queue`](struct.GlyphBrush.html#method.queue)
    ///
    /// Benefits from caching, see [caching behaviour](#caching-behaviour).
    #[inline]
    pub fn queue_custom_layout<'a, S, G>(&mut self, section: S, custom_layout: &G)
    where
        G: GlyphPositioner,
        S: Into<Cow<'a, Section<'a>>>,
    {
        self.glyph_brush.queue_custom_layout(section, custom_layout)
    }

    /// Queues pre-positioned glyphs to be processed by the next call of
    /// [`process_queued`](struct.GlyphBrush.html#method.process_queued). Can be called multiple
    /// times.
    #[inline]
    pub fn queue_pre_positioned(
        &mut self,
        glyphs: Vec<SectionGlyph>,
        extra: Vec<Extra>,
        bounds: Rect,
    ) {
        self.glyph_brush.queue_pre_positioned(glyphs, extra, bounds)
    }

    /// Retains the section in the cache as if it had been used in the last draw-frame.
    ///
    /// Should not generally be necessary, see [caching behaviour](#caching-behaviour).
    #[inline]
    pub fn keep_cached_custom_layout<'a, S, G>(&mut self, section: S, custom_layout: &G)
    where
        S: Into<Cow<'a, Section<'a>>>,
        G: GlyphPositioner,
    {
        self.glyph_brush
            .keep_cached_custom_layout(section, custom_layout)
    }

    /// Retains the section in the cache as if it had been used in the last draw-frame.
    ///
    /// Should not generally be necessary, see [caching behaviour](#caching-behaviour).
    #[inline]
    pub fn keep_cached<'a, S>(&mut self, section: S)
    where
        S: Into<Cow<'a, Section<'a>>>,
    {
        self.glyph_brush.keep_cached(section)
    }

    /// Returns the available fonts.
    ///
    /// The `FontId` corresponds to the index of the font data.
    #[inline]
    pub fn fonts(&self) -> &[F] {
        self.glyph_brush.fonts()
    }

    pub fn add_font(&mut self, font: F) -> FontId {
        self.glyph_brush.add_font(font)
    }
}

impl<F, H> GlyphBrush<(), F, H>
where
    F: Font,
    H: BuildHasher,
{
    fn new(
        device: ComPtr<ID3D11Device>,
        filter_mode: D3D11_FILTER,
        raw_builder: glyph_brush::GlyphBrushBuilder<F, H>,
    ) -> HResult<Self> {
        let glyph_brush = raw_builder.build();
        let (cache_width, cache_height) = glyph_brush.texture_dimensions();
        Ok(GlyphBrush {
            pipeline: Pipeline::<()>::new(device, filter_mode, cache_width, cache_height)?,
            glyph_brush,
        })
    }
}

impl<D, F, H> GlyphBrush<D, F, H>
where
    F: Font + Sync,
    H: BuildHasher,
{
    fn process_queued(&mut self) -> HResult<()> {
        let pipeline = &mut self.pipeline;

        let mut brush_action;

        let brush_action = loop {
            brush_action = self.glyph_brush.process_queued(
                |rect, tex_data| {
                    pipeline.update_cache(rect, tex_data);
                },
                |v| v.into(),
            );

            match brush_action {
                Ok(action) => break action,
                Err(BrushError::TextureTooSmall { suggested }) => {
                    let max_image_dimension = D3D11_REQ_TEXTURE2D_U_OR_V_DIMENSION;

                    let (new_width, new_height) = if (suggested.0 > max_image_dimension
                        || suggested.1 > max_image_dimension)
                        && (self.glyph_brush.texture_dimensions().0 < max_image_dimension
                            || self.glyph_brush.texture_dimensions().1 < max_image_dimension)
                    {
                        (max_image_dimension, max_image_dimension)
                    } else {
                        suggested
                    };

                    if log::log_enabled!(log::Level::Warn) {
                        log::warn!(
                            "Increasing glyph texture size {old:?} -> {new:?}. \
                             Consider building with `.initial_cache_size({new:?})` to avoid \
                             resizing",
                            old = self.glyph_brush.texture_dimensions(),
                            new = (new_width, new_height),
                        );
                    }

                    pipeline.increase_cache_size(new_width, new_height);
                    self.glyph_brush.resize_texture(new_width, new_height);
                }
            }
        };

        match brush_action {
            BrushAction::Draw(verts) => self.pipeline.upload(&verts),
            BrushAction::ReDraw => Ok(()),
        }
    }
}

impl<F: Font + Sync, H: BuildHasher> GlyphBrush<(), F, H> {
    #[inline]
    pub fn draw_queued(&mut self, target_width: u32, target_height: u32) -> HResult<()> {
        self.draw_queued_with_transform(orthographic_projection(target_width, target_height))
    }

    #[inline]
    pub fn draw_queued_with_transform(&mut self, transform: [f32; 16]) -> HResult<()> {
        self.process_queued()?;
        self.pipeline.draw(transform, None)
    }

    #[inline]
    pub fn draw_queued_with_transform_and_scissoring(
        &mut self,
        transform: [f32; 16],
        rect: D3D11_RECT,
    ) -> HResult<()> {
        self.process_queued()?;
        self.pipeline.draw(transform, Some(rect))
    }
}

#[rustfmt::skip]
pub fn orthographic_projection(width: u32, height: u32) -> [f32; 16] {
    let width = width as f32;
    let height = height as f32;
    [
         2.0 / width, 0.0,           0.0, 0.0,
         0.0,         -2.0 / height, 0.0, 0.0,
         0.0,         0.0,           0.0, 0.0,
        -1.0,         1.0,           0.0, 1.0,
    ]
}

impl<D, F: Font, H: BuildHasher> GlyphCruncher<F> for GlyphBrush<D, F, H> {
    #[inline]
    fn glyphs_custom_layout<'a, 'b, S, L>(
        &'b mut self,
        section: S,
        custom_layout: &L,
    ) -> SectionGlyphIter<'b>
    where
        L: GlyphPositioner + std::hash::Hash,
        S: Into<Cow<'a, Section<'a>>>,
    {
        self.glyph_brush
            .glyphs_custom_layout(section, custom_layout)
    }

    #[inline]
    fn fonts(&self) -> &[F] {
        self.glyph_brush.fonts()
    }

    #[inline]
    fn glyph_bounds_custom_layout<'a, S, L>(
        &mut self,
        section: S,
        custom_layout: &L,
    ) -> Option<Rect>
    where
        L: GlyphPositioner + std::hash::Hash,
        S: Into<Cow<'a, Section<'a>>>,
    {
        self.glyph_brush
            .glyph_bounds_custom_layout(section, custom_layout)
    }
}

impl<F, H> std::fmt::Debug for GlyphBrush<F, H> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GlyphBrush")
    }
}
