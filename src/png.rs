//! Secure, in-memory PNG rasterization for embedders (Face / agents / CI).
//!
//! This module is the hardened SVG→PNG path:
//! - bundled Roboto Regular as the primary face (generics resolve to it)
//! - no `file://` / `http(s)://` / path image resolvers (data-URLs still work)
//! - source-byte, megapixel, and per-axis caps
//! - Face-style sizing (`target_width` / `min_width` / `max_height` / `scale`)
//!
//! The CLI [`crate::write_output_png`] path remains unchanged (system fonts,
//! filesystem output). Prefer these APIs when rendering untrusted Mermaid.

use std::sync::{Arc, OnceLock};

use usvg::fontdb;

use crate::error::RenderError;
use crate::layout::compute_layout;
use crate::parse_mermaid_strict;
use crate::render::{render_svg, render_svg_with_dimensions};
use crate::{RenderOptions, Theme};

/// Bundled primary sans face (Roboto Regular, Apache-2.0).
///
/// System fonts are consulted only as glyph fallback for non-ASCII text when
/// the SVG is not pure ASCII. Shaping for Latin/named families is pinned to
/// this face so untrusted SVG cannot hijack metrics via Arial/Inter/etc.
const BUNDLED_FONT: &[u8] = include_bytes!("../assets/Roboto-Regular.ttf");

/// Hard ceiling on output area (~32 MP ≈ 5657×5657).
pub const MAX_OUTPUT_MEGAPIXELS: f32 = 32.0;

/// Hard ceiling on either output axis.
pub const MAX_OUTPUT_DIMENSION: u32 = 16_384;

/// Default max Mermaid source size accepted by [`render_png_bytes`].
pub const DEFAULT_MAX_SOURCE_BYTES: usize = 64 * 1024;

/// Face light terminal surface (`#FAFAFA`), opaque.
pub const FACE_LIGHT_SURFACE: Rgba = Rgba::new(0xFA, 0xFA, 0xFA, 0xFF);

/// Face dark terminal surface (`#18181B`), opaque.
pub const FACE_DARK_SURFACE: Rgba = Rgba::new(0x18, 0x18, 0x1B, 0xFF);

/// Straight 8-bit-per-channel, non-premultiplied RGBA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Format as opaque `#RRGGBB` (alpha ignored).
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// Caps applied before parse / raster so untrusted source cannot trivially
/// exhaust memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderLimits {
    /// Maximum accepted source length in bytes. Larger input is rejected with
    /// [`RenderError::ResourceLimit`] without invoking the parser.
    pub max_source_bytes: usize,
}

impl Default for RenderLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: DEFAULT_MAX_SOURCE_BYTES,
        }
    }
}

/// Face-compatible sizing + background parameters for PNG output.
///
/// Mirrors the Face `RenderParams` contract: `target_width_px` drives size when
/// non-zero; otherwise `scale` is used. `min_width_px` raises the scale for
/// small diagrams; `max_height_px` clamps tall ones. Megapixel / axis caps
/// always apply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PngRenderParams {
    /// Target output width in pixels. `0` falls back to [`Self::scale`].
    pub target_width_px: u32,
    /// Hard ceiling on output height; `0` disables (area/axis caps still apply).
    pub max_height_px: u32,
    /// Oversample when `target_width_px == 0`.
    pub scale: f32,
    /// Minimum output width; `0` disables.
    pub min_width_px: u32,
    /// Opaque background fill. `None` leaves the pixmap transparent.
    pub background: Option<Rgba>,
}

impl Default for PngRenderParams {
    fn default() -> Self {
        Self {
            target_width_px: 1024,
            max_height_px: 4096,
            scale: 1.0,
            min_width_px: 0,
            background: None,
        }
    }
}

impl PngRenderParams {
    /// Opaque Face light/dark surface suitable for terminal-flush PNGs.
    pub fn for_terminal(dark: bool) -> Self {
        Self {
            background: Some(if dark {
                FACE_DARK_SURFACE
            } else {
                FACE_LIGHT_SURFACE
            }),
            ..Self::default()
        }
    }

    /// Sizing for OS image viewers: 2× intrinsic (or `min_width_px`), taller
    /// height budget, opaque Face surface.
    pub fn for_os_viewer(dark: bool, min_width_px: u32, max_height_px: u32) -> Self {
        Self {
            target_width_px: 0,
            max_height_px,
            scale: 2.0,
            min_width_px,
            background: Some(if dark {
                FACE_DARK_SURFACE
            } else {
                FACE_LIGHT_SURFACE
            }),
        }
    }
}

/// Encoded PNG plus exact raster dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPng {
    pub png: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
}

impl RenderedPng {
    /// Destructure into `(png_bytes, width, height)`.
    pub fn into_parts(self) -> (Vec<u8>, u32, u32) {
        (self.png, self.width_px, self.height_px)
    }
}

struct FontSet {
    db: Arc<fontdb::Database>,
    family: String,
    bundled_id: fontdb::ID,
}

fn build_font_set(with_system_fonts: bool) -> FontSet {
    let mut db = fontdb::Database::new();
    db.load_font_data(BUNDLED_FONT.to_vec());
    let (bundled_id, family) = db
        .faces()
        .next()
        .map(|face| {
            (
                face.id,
                face.families
                    .first()
                    .map(|(name, _)| name.clone())
                    .unwrap_or_else(|| "sans-serif".to_string()),
            )
        })
        .expect("the bundled font must parse to at least one face");
    if with_system_fonts {
        db.load_system_fonts();
    }
    // Theme SVG emits lists ending in a generic (`sans-serif`). Point every
    // generic at the bundled face so labels are never blank when system fonts
    // are off.
    db.set_serif_family(&family);
    db.set_sans_serif_family(&family);
    db.set_monospace_family(&family);
    db.set_cursive_family(&family);
    db.set_fantasy_family(&family);
    FontSet {
        db: Arc::new(db),
        family,
        bundled_id,
    }
}

fn bundled_font() -> &'static FontSet {
    static FONT: OnceLock<FontSet> = OnceLock::new();
    FONT.get_or_init(|| build_font_set(false))
}

fn font_with_system_fallback() -> &'static FontSet {
    static FONT: OnceLock<FontSet> = OnceLock::new();
    FONT.get_or_init(|| build_font_set(true))
}

fn font_set_for(svg: &str) -> &'static FontSet {
    if svg.is_ascii() {
        bundled_font()
    } else {
        font_with_system_fallback()
    }
}

fn pinned_resolver(bundled_id: fontdb::ID) -> usvg::FontResolver<'static> {
    usvg::FontResolver {
        select_font: Box::new(move |_font, _db| Some(bundled_id)),
        select_fallback: usvg::FontResolver::default_fallback_selector(),
    }
}

fn rgba_to_color(c: Rgba) -> resvg::tiny_skia::Color {
    resvg::tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

/// Compute the scale applied to the SVG's intrinsic size.
pub fn effective_scale(base_w: f32, base_h: f32, params: &PngRenderParams) -> f32 {
    let mut scale = if params.target_width_px > 0 {
        params.target_width_px as f32 / base_w
    } else {
        params.scale
    };
    if !scale.is_finite() || scale <= 0.0 {
        scale = 1.0;
    }

    if params.min_width_px > 0 {
        let min_scale = params.min_width_px as f32 / base_w;
        if min_scale.is_finite() && min_scale > scale {
            scale = min_scale;
        }
    }

    if params.max_height_px > 0 {
        let max_h = params.max_height_px as f32;
        if base_h * scale > max_h {
            scale = max_h / base_h;
        }
    }

    let max_area = MAX_OUTPUT_MEGAPIXELS * 1_000_000.0;
    let area = (base_w * scale) * (base_h * scale);
    if area > max_area {
        scale *= (max_area / area).sqrt();
    }

    scale.max(f32::MIN_POSITIVE)
}

/// Floor + hard-cap float dimensions into the final pixmap size.
pub fn clamp_dimensions(width_f: f32, height_f: f32) -> (u32, u32) {
    let mut width_px = (width_f.floor() as u32).clamp(1, MAX_OUTPUT_DIMENSION);
    let mut height_px = (height_f.floor() as u32).clamp(1, MAX_OUTPUT_DIMENSION);

    let max_area = (MAX_OUTPUT_MEGAPIXELS * 1_000_000.0) as u64;
    if width_px as u64 * height_px as u64 > max_area {
        if width_px >= height_px {
            width_px = ((max_area / height_px as u64) as u32).max(1);
        } else {
            height_px = ((max_area / width_px as u64) as u32).max(1);
        }
    }
    (width_px, height_px)
}

/// Resolve Face-style output pixel size from an intrinsic SVG size.
pub fn resolve_output_dimensions(base_w: f32, base_h: f32, params: &PngRenderParams) -> (u32, u32) {
    let scale = effective_scale(base_w, base_h, params);
    clamp_dimensions(base_w * scale, base_h * scale)
}

/// Measure Mermaid layout, then apply [`PngRenderParams`] sizing.
///
/// Useful when a host wants canvas size before calling
/// [`render_svg_with_dimensions`] or [`rasterize_svg_to_png`].
pub fn resolve_render_dimensions(
    input: &str,
    options: &RenderOptions,
    params: &PngRenderParams,
    limits: &RenderLimits,
) -> Result<(u32, u32), RenderError> {
    enforce_source_limit(input, limits)?;
    let parsed = parse_mermaid_strict(input)?;
    let layout = compute_layout(&parsed.graph, &options.theme, &options.layout);
    let base_w = layout.width.max(1.0);
    let base_h = layout.height.max(1.0);
    Ok(resolve_output_dimensions(base_w, base_h, params))
}

fn enforce_source_limit(input: &str, limits: &RenderLimits) -> Result<(), RenderError> {
    if input.len() > limits.max_source_bytes {
        return Err(RenderError::ResourceLimit(format!(
            "source is {} bytes, over the {}-byte limit",
            input.len(),
            limits.max_source_bytes
        )));
    }
    Ok(())
}

/// Rasterize an SVG string to PNG with the secure embed profile.
pub fn rasterize_svg_to_png(
    svg: &str,
    params: &PngRenderParams,
) -> Result<RenderedPng, RenderError> {
    rasterize_with_font(svg, params, font_set_for(svg))
}

fn rasterize_with_font(
    svg: &str,
    params: &PngRenderParams,
    font: &FontSet,
) -> Result<RenderedPng, RenderError> {
    let mut opt = usvg::Options {
        fontdb: Arc::clone(&font.db),
        font_family: font.family.clone(),
        font_resolver: pinned_resolver(font.bundled_id),
        ..Default::default()
    };
    // SECURITY: default string resolver reads image hrefs from disk / network.
    // Replace with a no-op; in-memory data-URLs stay supported via resolve_data.
    opt.image_href_resolver.resolve_string = Box::new(|_href, _opt| None);

    let tree =
        usvg::Tree::from_str(svg, &opt).map_err(|e| RenderError::Rasterize(e.to_string()))?;

    let size = tree.size();
    let (base_w, base_h) = (size.width(), size.height());
    if base_w <= 0.0 || base_h <= 0.0 {
        return Err(RenderError::Rasterize("diagram has zero size".to_string()));
    }

    let (width_px, height_px) = resolve_output_dimensions(base_w, base_h, params);

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width_px, height_px).ok_or_else(|| {
        RenderError::Rasterize(format!("invalid pixmap size {width_px}x{height_px}"))
    })?;
    if let Some(bg) = params.background {
        pixmap.fill(rgba_to_color(bg));
    }

    let transform = resvg::tiny_skia::Transform::from_scale(
        width_px as f32 / base_w,
        height_px as f32 / base_h,
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let png = pixmap
        .encode_png()
        .map_err(|e| RenderError::Rasterize(e.to_string()))?;

    Ok(RenderedPng {
        png,
        width_px,
        height_px,
    })
}

/// Parse → layout → SVG → secure PNG bytes + dimensions.
///
/// Returns `(png, width, height)` via [`RenderedPng::into_parts`] when needed.
pub fn render_png_bytes(
    input: &str,
    options: RenderOptions,
    params: &PngRenderParams,
    limits: &RenderLimits,
) -> Result<RenderedPng, RenderError> {
    enforce_source_limit(input, limits)?;
    let parsed = parse_mermaid_strict(input)?;
    let layout = compute_layout(&parsed.graph, &options.theme, &options.layout);
    let svg = render_svg(&layout, &options.theme, &options.layout);
    rasterize_svg_to_png(&svg, params)
}

/// Like [`render_png_bytes`], but forces the SVG canvas to
/// [`resolve_render_dimensions`] before rasterizing (1:1 with Face hosts that
/// want layout-side sizing via [`render_svg_with_dimensions`]).
pub fn render_png_bytes_with_sized_svg(
    input: &str,
    options: RenderOptions,
    params: &PngRenderParams,
    limits: &RenderLimits,
) -> Result<RenderedPng, RenderError> {
    enforce_source_limit(input, limits)?;
    let parsed = parse_mermaid_strict(input)?;
    let layout = compute_layout(&parsed.graph, &options.theme, &options.layout);
    let (w, h) = resolve_output_dimensions(layout.width.max(1.0), layout.height.max(1.0), params);
    let svg = render_svg_with_dimensions(
        &layout,
        &options.theme,
        &options.layout,
        Some((w as f32, h as f32)),
    );
    // SVG already matches the target canvas; rasterize at scale 1.
    let sized = PngRenderParams {
        target_width_px: 0,
        max_height_px: 0,
        scale: 1.0,
        min_width_px: 0,
        background: params.background,
    };
    rasterize_svg_to_png(&svg, &sized)
}

/// Theme preset for Face light surfaces (`#FAFAFA` background).
pub fn face_light_theme() -> Theme {
    Theme::face_light()
}

/// Theme preset for Face dark surfaces (`#18181B` background).
pub fn face_dark_theme() -> Theme {
    Theme::face_dark()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG_100X50: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50" viewBox="0 0 100 50"><rect x="0" y="0" width="10" height="10" fill="#0000ff"/></svg>"##;

    fn params(target_width_px: u32, max_height_px: u32) -> PngRenderParams {
        PngRenderParams {
            target_width_px,
            max_height_px,
            scale: 1.0,
            min_width_px: 0,
            background: None,
        }
    }

    fn png_wh(png: &[u8]) -> (u32, u32) {
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "missing PNG magic");
        // IHDR: length(4) + type(4) + width(4) + height(4) starts at byte 8.
        let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
        (w, h)
    }

    #[test]
    fn render_png_bytes_flowchart() {
        let out = render_png_bytes(
            "flowchart LR\nA-->B",
            RenderOptions::default(),
            &PngRenderParams::default(),
            &RenderLimits::default(),
        )
        .expect("render");
        assert!(out.width_px > 0 && out.height_px > 0);
        assert_eq!(png_wh(&out.png), (out.width_px, out.height_px));
        let (bytes, w, h) = out.into_parts();
        assert!(!bytes.is_empty());
        assert_eq!((w, h), png_wh(&bytes));
    }

    #[test]
    fn render_png_bytes_sequence() {
        let src = "sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: Hi";
        let out = render_png_bytes(
            src,
            RenderOptions {
                theme: Theme::face_dark(),
                ..RenderOptions::default()
            },
            &PngRenderParams::for_terminal(true),
            &RenderLimits::default(),
        )
        .expect("sequence");
        assert!(out.png.starts_with(b"\x89PNG"));
        assert!(out.width_px >= 1 && out.height_px >= 1);
    }

    #[test]
    fn oversized_source_is_resource_limit() {
        let limits = RenderLimits {
            max_source_bytes: 8,
        };
        let err = render_png_bytes(
            "flowchart LR; A-->B-->C",
            RenderOptions::default(),
            &PngRenderParams::default(),
            &limits,
        )
        .expect_err("must reject");
        assert!(matches!(err, RenderError::ResourceLimit(_)));
    }

    #[test]
    fn parse_error_is_typed() {
        let err = render_png_bytes(
            "not a diagram at all {{{",
            RenderOptions::default(),
            &PngRenderParams::default(),
            &RenderLimits::default(),
        )
        .expect_err("must fail parse");
        assert!(matches!(err, RenderError::Parse(_)));
    }

    #[test]
    fn target_width_drives_output() {
        let out = rasterize_svg_to_png(SVG_100X50, &params(200, 10_000)).expect("rasterize");
        assert_eq!((out.width_px, out.height_px), (200, 100));
    }

    #[test]
    fn min_width_raises_scale() {
        let mut p = params(0, 10_000);
        p.min_width_px = 400;
        let out = rasterize_svg_to_png(SVG_100X50, &p).expect("rasterize");
        assert_eq!((out.width_px, out.height_px), (400, 200));
    }

    #[test]
    fn megapixel_cap_bounds_huge_request() {
        let big = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="1000" viewBox="0 0 1000 1000"><rect width="1000" height="1000" fill="#123456"/></svg>"##;
        let out = rasterize_svg_to_png(big, &params(200_000, u32::MAX)).expect("rasterize");
        let area = out.width_px as u64 * out.height_px as u64;
        assert!(area <= (MAX_OUTPUT_MEGAPIXELS as u64) * 1_000_000);
    }

    #[test]
    fn remote_image_href_is_not_fetched() {
        // Blue rect fills the canvas; a remote <image> must not paint over it
        // (resolver returns None). Success = still mostly blue, no panic.
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="40" height="40" viewBox="0 0 40 40">
  <rect width="40" height="40" fill="#0000ff"/>
  <image width="40" height="40" href="https://example.com/does-not-exist.png"/>
  <image width="40" height="40" xlink:href="http://127.0.0.1:9/x.png"/>
</svg>"##;
        let mut p = params(0, 10_000);
        p.background = Some(Rgba::new(255, 0, 0, 255));
        let out = rasterize_svg_to_png(svg, &p).expect("rasterize");
        assert_eq!((out.width_px, out.height_px), (40, 40));
        // PNG must be valid; we do not assert pixel color without an image decoder.
        assert!(out.png.starts_with(b"\x89PNG"));
    }

    #[test]
    fn file_image_href_is_refused() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20">
  <rect width="20" height="20" fill="#00ff00"/>
  <image width="20" height="20" href="file:///etc/passwd"/>
  <image width="20" height="20" href="C:\Windows\System32\drivers\etc\hosts"/>
</svg>"##;
        let out = rasterize_svg_to_png(svg, &params(0, 10_000)).expect("rasterize");
        assert!(out.png.starts_with(b"\x89PNG"));
        assert_eq!(png_wh(&out.png), (out.width_px, out.height_px));
    }

    #[test]
    fn data_url_image_still_allowed() {
        // 1x1 red PNG as data URL — must decode (resolve_data stays enabled).
        let svg = concat!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8" viewBox="0 0 8 8">"##,
            r##"<image width="8" height="8" href="data:image/png;base64,"##,
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
            r##""/></svg>"##,
        );
        let out = rasterize_svg_to_png(svg, &params(0, 10_000)).expect("data-url");
        assert_eq!((out.width_px, out.height_px), (8, 8));
    }

    #[test]
    fn bundled_font_renders_generic_family_text() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="60" viewBox="0 0 200 60"><text x="10" y="38" font-family="Inter, ui-sans-serif, system-ui, sans-serif" font-size="28" fill="#000000">Hello</text></svg>"##;
        let mut p = params(0, 10_000);
        p.background = Some(Rgba::new(255, 255, 255, 255));
        let out = rasterize_svg_to_png(svg, &p).expect("rasterize");
        // Non-empty PNG larger than a blank canvas heuristic.
        assert!(out.png.len() > 200);
        assert_ne!(bundled_font().family, "sans-serif");
    }

    #[test]
    fn face_surface_constants() {
        assert_eq!(FACE_LIGHT_SURFACE.to_hex(), "#FAFAFA");
        assert_eq!(FACE_DARK_SURFACE.to_hex(), "#18181B");
        assert_eq!(Theme::face_light().background, "#FAFAFA");
        assert_eq!(Theme::face_dark().background, "#18181B");
    }

    #[test]
    fn resolve_dimensions_helper_matches_raster() {
        let (w, h) = resolve_output_dimensions(100.0, 50.0, &params(200, 10_000));
        assert_eq!((w, h), (200, 100));
        let out = rasterize_svg_to_png(SVG_100X50, &params(200, 10_000)).unwrap();
        assert_eq!((out.width_px, out.height_px), (w, h));
    }

    #[test]
    fn for_os_viewer_uses_min_width() {
        let p = PngRenderParams::for_os_viewer(false, 2560, 8192);
        let out = rasterize_svg_to_png(SVG_100X50, &p).expect("rasterize");
        assert_eq!((out.width_px, out.height_px), (2560, 1280));
        assert_eq!(p.background, Some(FACE_LIGHT_SURFACE));
    }

    #[test]
    fn ascii_svg_uses_bundled_only_database() {
        let set = font_set_for(SVG_100X50);
        assert!(std::ptr::eq(set, bundled_font()));
        assert_eq!(set.db.faces().count(), 1);
    }
}
