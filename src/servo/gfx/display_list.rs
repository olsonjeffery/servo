use azure::azure_hl::DrawTarget;
use gfx::render_task::{draw_solid_color, draw_image, draw_glyphs};
use gfx::geometry::*;
use geom::rect::Rect;
use image::base::Image;
use render_task::RenderContext;

use std::arc::{ARC, clone};
use dvec::DVec;
use text::glyph::Glyph;

pub use layout::display_list_builder::DisplayListBuilder;

struct DisplayItem {
    draw: ~fn((&DisplayItem), (&RenderContext)),
    bounds : Rect<au>, // TODO: whose coordinate system should this use?
    data : DisplayItemData
}

enum DisplayItemData {
    SolidColorData(u8, u8, u8),
    GlyphData(GlyphRun),
    ImageData(ARC<~image::base::Image>),
    PaddingData(u8, u8, u8, u8) // This is a hack to make fonts work (?)
}

/**
A run of glyphs in a single font. This is distinguished from any similar
structure used by layout in that this must be sendable, whereas the text
shaping data structures may end up unsendable.
*/
struct GlyphRun {
    glyphs: ~[Glyph]
}

fn draw_SolidColor(self: &DisplayItem, ctx: &RenderContext) {
    match self.data {
        SolidColorData(r,g,b) => draw_solid_color(ctx, &self.bounds, r, g, b),
        _ => fail
    }        
}

fn draw_Glyphs(self: &DisplayItem, ctx: &RenderContext) {
    match self.data {
        GlyphData(run) => draw_glyphs(ctx, self.bounds, &run),
        _ => fail
    }        
}

fn draw_Image(self: &DisplayItem, ctx: &RenderContext) {
    match self.data {
        ImageData(img) => draw_image(ctx, self.bounds, img),
        _ => fail
    }        
}

fn SolidColor(bounds: Rect<au>, r: u8, g: u8, b: u8) -> DisplayItem {
    DisplayItem { 
        // TODO: this seems wrong.
        draw: |self, ctx| draw_SolidColor(self, ctx),
        bounds: bounds,
        data: SolidColorData(r, g, b)
    }
}

fn Glyphs(bounds: Rect<au>, run: GlyphRun) -> DisplayItem {
    DisplayItem {
        draw: |self, ctx| draw_Glyphs(self, ctx),
        bounds: bounds,
        data: GlyphData(run)
    }
}

// ARC should be cloned into ImageData, but Images are not sendable
fn Image(bounds: Rect<au>, image: ARC<~image::base::Image>) -> DisplayItem {
    DisplayItem {
        // TODO: this seems wrong.
        draw: |self, ctx| draw_Image(self, ctx),
        bounds: bounds,
        data: ImageData(clone(&image))
    }
}

type DisplayList = DVec<~DisplayItem>;

trait DisplayListMethods {
    fn draw(ctx: &RenderContext);
}

impl DisplayList : DisplayListMethods {
    fn draw(ctx: &RenderContext) {
        for self.each |item| {
            debug!("drawing %?", *item);
            item.draw(*item, ctx);
        }
    }
}
