/** Creates CSS boxes from a DOM. */
use au = gfx::geometry;
use core::dvec::DVec;
use css::styles::{SpecifiedStyle, empty_style_for_node_kind};
use css::values::{CSSDisplay, DisplayBlock, DisplayInline, DisplayInlineBlock, DisplayNone};
use css::values::{Inherit, Initial, Specified};
use dom::element::*;
use dom::node::{Comment, Doctype, Element, Text, Node, NodeTree, LayoutData};
use image::holder::ImageHolder;
use layout::flow::{FlowContext, FlowContextData, BlockFlow, InlineFlow, InlineBlockFlow, RootFlow, FlowTree};
use layout::box::{RenderBox, BoxData, GenericBox, ImageBox, TextBox, RenderBoxTree};
use layout::block::BlockFlowData;
use layout::context::LayoutContext;
use layout::inline::InlineFlowData;
use layout::root::RootFlowData;
use layout::text::TextBoxData;
use option::is_none;
use servo_text::font_cache::FontCache;
use servo_text::text_run::TextRun;
use util::tree;

export LayoutTreeBuilder;

struct LayoutTreeBuilder {
    mut root_ctx: Option<@FlowContext>,
    mut next_bid: int,
    mut next_cid: int
}

fn LayoutTreeBuilder() -> LayoutTreeBuilder {
    LayoutTreeBuilder {
        root_ctx: None,
        next_bid: -1,
        next_cid: -1
    }
}

impl LayoutTreeBuilder {
    /* Debug-only ids */
    fn next_box_id() -> int { self.next_bid += 1; self.next_bid }
    fn next_ctx_id() -> int { self.next_cid += 1; self.next_cid }

    /** Creates necessary box(es) and flow context(s) for the current DOM node,
    and recurses on its children. */
    fn construct_recursively(layout_ctx: &LayoutContext, cur_node: Node, 
                             parent_ctx: @FlowContext, parent_box: Option<@RenderBox>) {

        let style = cur_node.style();
        
        // DEBUG
        let n_str = fmt!("%?", cur_node.read(|n| copy n.kind ));
        debug!("Considering node: %?", n_str);

        // TODO: remove this once UA styles work
        // TODO: handle interactions with 'float', 'position' (CSS 2.1, Section 9.7)
        let simulated_display = match self.simulate_UA_display_rules(cur_node, style) {
            DisplayNone => return, // tree ends here if 'display: none'
            v => v
        };

        // first, create the proper box kind, based on node characteristics
        let box_data = self.create_box_data(layout_ctx, cur_node, simulated_display);

        // then, figure out its proper context, possibly reorganizing.
        let next_ctx: @FlowContext = match box_data {
            /* Text box is always an inline flow. create implicit inline
            flow ctx if we aren't inside one already. */
            TextBox(*) => {
                if (parent_ctx.starts_inline_flow()) {
                    parent_ctx
                } else {
                    self.make_ctx(InlineFlow(InlineFlowData()), tree::empty())
                }
            },
            ImageBox(*) | GenericBox => {
                match simulated_display {
                    DisplayInline | DisplayInlineBlock => {
                        /* if inline, try to put into inline context,
                        making a new one if necessary */
                        if (parent_ctx.starts_inline_flow()) {
                            parent_ctx
                        } else {
                            self.make_ctx(InlineFlow(InlineFlowData()), tree::empty())
                        }
                    },
                    /* block boxes always create a new context */
                    DisplayBlock => {
                        self.make_ctx(BlockFlow(BlockFlowData()), tree::empty())
                    },
                    _ => fail fmt!("unsupported display type in box generation: %?", simulated_display)
                }
            }
        };

        // store reference to the flow context which contains any boxes
        // that correspond to cur_node
        assert cur_node.has_aux();
        do cur_node.aux |data| { data.flow = Some(next_ctx) }

        // make box, add box to any context-specific list.
        let mut new_box = self.make_box(cur_node, parent_ctx, box_data);
        debug!("Assign ^box to flow: %?", next_ctx.debug_str());

        match next_ctx.kind {
            InlineFlow(d) => {
                d.boxes.push(new_box);

                if (parent_box.is_some()) {
                    let parent = parent_box.get();

                    // connect the box to its parent box
                    debug!("In inline flow f%?, set child b%? of parent b%?", next_ctx.id, parent.id, new_box.id);
                    RenderBoxTree.add_child(parent, new_box);
                }
            }
            BlockFlow(d) => { d.box = Some(new_box) }
            _ => {} // TODO: float lists, etc.
        };

    
        if (!next_ctx.eq(&parent_ctx)) {
            debug!("Adding child flow f%? of f%?", parent_ctx.id, next_ctx.id);
            FlowTree.add_child(parent_ctx, next_ctx);
        }
        // recurse
        // TODO: don't set parent box unless this is an inline flow?
        do NodeTree.each_child(cur_node) |child_node| {
            self.construct_recursively(layout_ctx, child_node, next_ctx, Some(new_box)); true
        }

        // Fixup any irregularities, such as split inlines (CSS 2.1 Section 9.2.1.1)
        if (next_ctx.starts_inline_flow()) {
            let mut found_child_inline = false;
            let mut found_child_block = false;

            do FlowTree.each_child(next_ctx) |child_ctx| {
                match child_ctx.kind {
                    InlineFlow(*) | InlineBlockFlow => found_child_inline = true,
                    BlockFlow(*) => found_child_block = true,
                    _ => {}
                }; true
            }

            if found_child_block && found_child_inline {
                self.fixup_split_inline(next_ctx)
            }
        }
    }

    fn fixup_split_inline(_foo: @FlowContext) {
        // TODO: finish me. 
        fail ~"TODO: handle case where an inline is split by a block"
    }

    priv fn simulate_UA_display_rules(node: Node, style: SpecifiedStyle) -> CSSDisplay {
        let resolved = match style.display_type {
            Inherit | Initial => DisplayInline, // TODO: remove once resolve works
            Specified(v) => v
        };

        if (resolved == DisplayNone) { return resolved; }

        do node.read |n| {
            match n.kind {
                ~Doctype(*) | ~Comment(*) => DisplayNone,
                ~Text(*) => DisplayInline,
                ~Element(e) => match e.kind {
                    ~HTMLHeadElement(*) => DisplayNone,
                    ~HTMLScriptElement(*) => DisplayNone,
                    _ => resolved
                }
            }
        }
    }

    /** entry point for box creation. Should only be 
    called on root DOM element. */
    fn construct_trees(layout_ctx: &LayoutContext, root: Node) -> Result<@FlowContext, ()> {
        self.root_ctx = Some(self.make_ctx(RootFlow(RootFlowData()), tree::empty()));

        self.construct_recursively(layout_ctx, root, self.root_ctx.get(), None);
        return Ok(self.root_ctx.get())
    }

    fn make_ctx(kind : FlowContextData, tree: tree::Tree<@FlowContext>) -> @FlowContext {
        let ret = @FlowContext(self.next_ctx_id(), kind, tree);
        debug!("Created context: %s", ret.debug_str());
        ret
    }

    fn make_box(node : Node, ctx: @FlowContext, data: BoxData) -> @RenderBox {
        let ret = @RenderBox(self.next_box_id(), node, ctx, data);
        debug!("Created box: %s", ret.debug_str());
        ret
    }

    /* Based on the DOM node type, create a specific type of box */
    fn create_box_data(layout_ctx: &LayoutContext, node: Node, display: CSSDisplay) -> BoxData {
        // TODO: handle more types of nodes.
        do node.read |n| {
            match n.kind {
                ~Doctype(*) | ~Comment(*) => fail ~"Hey, doctypes and comments shouldn't get here! They are display:none!",
                ~Text(string) => {
                    // TODO: clean this up. Fonts should not be created here.
                    let font = layout_ctx.font_cache.get_test_font();
                    let run = TextRun(font, string);
                    TextBox(TextBoxData(copy string, ~[move run]))
                }
                ~Element(element) => {
                    match (element.kind, display) {
                        (~HTMLImageElement(d), _) if d.image.is_some() => {
                            let holder = ImageHolder(&d.image.get(), 
                                                     layout_ctx.image_cache,
                                                     copy layout_ctx.reflow_cb);
                            ImageBox(holder)
                        },
//                      (_, Specified(_)) => GenericBox,
                        (_, _) => GenericBox // TODO: replace this with the commented lines
//                      (_, _) => fail ~"Can't create box for Node with non-specified 'display' type"
                    }
                }
            }
        }
    }
}

