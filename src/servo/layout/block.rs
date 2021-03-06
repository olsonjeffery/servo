use au = gfx::geometry;
use css::values::*;
use dl = gfx::display_list;
use geom::point::Point2D;
use geom::rect::Rect;
use geom::size::Size2D;
use gfx::geometry::au;
use layout::box::{RenderBox};
use layout::context::LayoutContext;
use layout::flow::{FlowContext, FlowTree, InlineBlockFlow, BlockFlow, RootFlow};
use util::tree;

struct BlockFlowData {
    mut box: Option<@RenderBox>
}

fn BlockFlowData() -> BlockFlowData {
    BlockFlowData {
        box: None
    }
}

trait BlockLayout {
    pure fn starts_block_flow() -> bool;
    pure fn access_block<T>(fn(&&BlockFlowData) -> T) -> T;
    pure fn with_block_box(fn(&&@RenderBox) -> ()) -> ();

    fn bubble_widths_block(ctx: &LayoutContext);
    fn assign_widths_block(ctx: &LayoutContext);
    fn assign_height_block(ctx: &LayoutContext);

    fn build_display_list_block(a: &dl::DisplayListBuilder, b: &Rect<au>,
                                c: &Point2D<au>, d: &dl::DisplayList);
}

impl @FlowContext : BlockLayout {

    pure fn starts_block_flow() -> bool {
        match self.kind {
            RootFlow(*) | BlockFlow(*) | InlineBlockFlow(*) => true,
            _ => false 
        }
    }

    pure fn access_block<T>(cb:fn(&&BlockFlowData) -> T) -> T {
        match self.kind {
            BlockFlow(d) => cb(d),
            _  => fail fmt!("Tried to access() data of BlockFlow, but this is a %?", self.kind)
        }
    }

    /* Get the current flow's corresponding block box, if it exists, and do something with it. 
       This works on both BlockFlow and RootFlow, since they are mostly the same. */
    pure fn with_block_box(cb:fn(&&@RenderBox) -> ()) -> () {
        match self.kind {
            BlockFlow(*) => { 
                do self.access_block |d| {
                    let mut box = d.box;
                    box.iter(cb)
                }
            },
            RootFlow(*) => {
                do self.access_root |d| {
                    let mut box = d.box;
                    box.iter(cb)
                }
            },
            _  => fail fmt!("Tried to do something with_block_box(), but this is a %?", self.kind)
        }
    }

    /* Recursively (bottom-up) determine the context's preferred and
    minimum widths.  When called on this context, all child contexts
    have had their min/pref widths set. This function must decide
    min/pref widths based on child context widths and dimensions of
    any boxes it is responsible for flowing.  */

    /* TODO: floats */
    /* TODO: absolute contexts */
    /* TODO: inline-blocks */
    fn bubble_widths_block(_ctx: &LayoutContext) {
        assert self.starts_block_flow();

        let mut min_width = au(0);
        let mut pref_width = au(0);

        /* find max width from child block contexts */
        for FlowTree.each_child(self) |child_ctx| {
            assert child_ctx.starts_block_flow() || child_ctx.starts_inline_flow();

            min_width  = au::max(min_width, child_ctx.data.min_width);
            pref_width = au::max(pref_width, child_ctx.data.pref_width);
        }

        /* if not an anonymous block context, add in block box's widths.
           these widths will not include child elements, just padding etc. */
        do self.with_block_box |box| {
            min_width = min_width.add(box.get_min_width());
            pref_width = pref_width.add(box.get_pref_width());
        }

        self.data.min_width = min_width;
        self.data.pref_width = pref_width;
    }
 
    /* Recursively (top-down) determines the actual width of child
    contexts and boxes. When called on this context, the context has
    had its width set by the parent context.

    Dual boxes consume some width first, and the remainder is assigned to
    all child (block) contexts. */

    fn assign_widths_block(_ctx: &LayoutContext) { 
        assert self.starts_block_flow();

        let mut remaining_width = self.data.position.size.width;
        let mut _right_used = au(0);
        let mut left_used = au(0);

        /* Let the box consume some width. It will return the amount remaining
           for its children. */
        do self.with_block_box |box| {
            box.data.position.size.width = remaining_width;
            let (left_used, right_used) = box.get_used_width();
            remaining_width = remaining_width.sub(left_used.add(right_used));
        }

        for FlowTree.each_child(self) |child_ctx| {
            assert child_ctx.starts_block_flow() || child_ctx.starts_inline_flow();
            child_ctx.data.position.origin.x = left_used;
            child_ctx.data.position.size.width = remaining_width;
        }
    }

    fn assign_height_block(_ctx: &LayoutContext) {
        assert self.starts_block_flow();

        let mut cur_y = au(0);

        for FlowTree.each_child(self) |child_ctx| {
            child_ctx.data.position.origin.y = cur_y;
            cur_y = cur_y.add(child_ctx.data.position.size.height);
        }

        self.data.position.size.height = cur_y;

        let _used_top = au(0);
        let _used_bot = au(0);
        
        do self.with_block_box |box| {
            box.data.position.origin.y = au(0);
            box.data.position.size.height = cur_y;
            let (_used_top, _used_bot) = box.get_used_height();
        }
    }

    fn build_display_list_block(builder: &dl::DisplayListBuilder, dirty: &Rect<au>, 
                                offset: &Point2D<au>, list: &dl::DisplayList) {

        assert self.starts_block_flow();
        
        // add box that starts block context
        do self.with_block_box |box| {
            box.build_display_list(builder, dirty, offset, list)
        }

        // TODO: handle any out-of-flow elements

        // go deeper into the flow tree
        for FlowTree.each_child(self) |child| {
            self.build_display_list_for_child(builder, child, dirty, offset, list)
        }
    }
}
