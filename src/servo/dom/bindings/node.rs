use js::rust::{bare_compartment, methods, jsobj};
use js::{JS_ARGV, JSCLASS_HAS_RESERVED_SLOTS, JSPROP_ENUMERATE, JSPROP_SHARED, JSVAL_NULL,
            JS_THIS_OBJECT, JS_SET_RVAL, JSPROP_NATIVE_ACCESSORS};
use js::jsapi::{JSContext, jsval, JSObject, JSBool, jsid, JSClass, JSFreeOp, JSPropertySpec};
use js::jsapi::bindgen::{JS_ValueToString, JS_GetStringCharsZAndLength, JS_ReportError,
                            JS_GetReservedSlot, JS_SetReservedSlot, JS_NewStringCopyN,
                            JS_DefineFunctions, JS_DefineProperty, JS_GetContextPrivate};
use js::jsapi::bindgen::*;
use js::glue::bindgen::*;
use js::crust::{JS_PropertyStub, JS_StrictPropertyStub, JS_EnumerateStub, JS_ConvertStub};

use dom::node::{Node, NodeScope, Text, Doctype, Comment, Element};
use utils::{rust_box, squirrel_away_unique, get_compartment, domstring_to_jsval, str};
use libc::c_uint;
use ptr::null;

fn init(compartment: bare_compartment) {
    let obj = utils::define_empty_prototype(~"Node", None, compartment);

    let attrs = @~[
        {name: compartment.add_name(~"firstChild"),
         tinyid: 0,
         flags: (JSPROP_SHARED | JSPROP_ENUMERATE | JSPROP_NATIVE_ACCESSORS) as u8,
         getter: {op: getFirstChild, info: null()},
         setter: {op: null(), info: null()}},

        {name: compartment.add_name(~"nextSibling"),
         tinyid: 0,
         flags: (JSPROP_SHARED | JSPROP_ENUMERATE | JSPROP_NATIVE_ACCESSORS) as u8,
         getter: {op: getNextSibling, info: null()},
         setter: {op: null(), info: null()}},

        {name: compartment.add_name(~"nodeType"),
         tinyid: 0,
         flags: (JSPROP_SHARED | JSPROP_ENUMERATE | JSPROP_NATIVE_ACCESSORS) as u8,
         getter: {op: getNodeType, info: null()},
         setter: {op: null(), info: null()}}];
    vec::push(compartment.global_props, attrs);
    vec::as_imm_buf(*attrs, |specs, _len| {
        JS_DefineProperties(compartment.cx.ptr, obj.ptr, specs);
    });
}

fn create(cx: *JSContext, node: Node, scope: NodeScope) -> jsobj unsafe {
    do scope.write(node) |nd| {
        match nd.kind {
            ~Element(*) => {
              element::create(cx, node, scope)
            }
            ~Text(*) => {
              fail ~"no text node bindings yet";
            }
            ~Comment(*) => {
              fail ~"no comment node bindings yet";
            }
            ~Doctype(*) => {
              fail ~"no doctype node bindings yet";
            }
        }
    }
}

struct NodeBundle {
    node: Node,
    scope: NodeScope,
}

fn NodeBundle(n: Node, s: NodeScope) -> NodeBundle {
    NodeBundle {
        node : n,
        scope : s
    }
}

unsafe fn unwrap(obj: *JSObject) -> *rust_box<NodeBundle> {
    let val = JS_GetReservedSlot(obj, 0);
    cast::reinterpret_cast(&RUST_JSVAL_TO_PRIVATE(val))
}

extern fn getFirstChild(cx: *JSContext, _argc: c_uint, vp: *mut jsval) -> JSBool {
    unsafe {
        let obj = JS_THIS_OBJECT(cx, cast::reinterpret_cast(&vp));
        if obj.is_null() {
            return 0;
        }

        let bundle = unwrap(obj);
        do (*bundle).payload.scope.write((*bundle).payload.node) |nd| {
            match nd.tree.first_child {
              Some(n) => {
                let obj = create(cx, n, (*bundle).payload.scope).ptr;
                *vp = RUST_OBJECT_TO_JSVAL(obj);
              }
              None => {
                *vp = JSVAL_NULL;
              }
            }
        };
    }
    return 1;
}

extern fn getNextSibling(cx: *JSContext, _argc: c_uint, vp: *mut jsval) -> JSBool {
    unsafe {
        let obj = JS_THIS_OBJECT(cx, cast::reinterpret_cast(&vp));
        if obj.is_null() {
            return 0;
        }

        let bundle = unwrap(obj);
        do (*bundle).payload.scope.write((*bundle).payload.node) |nd| {
            match nd.tree.next_sibling {
              Some(n) => {
                let obj = create(cx, n, (*bundle).payload.scope).ptr;
                *vp = RUST_OBJECT_TO_JSVAL(obj);
              }
              None => {
                *vp = JSVAL_NULL;
              }
            }
        };
    }
    return 1;
}

extern fn getNodeType(cx: *JSContext, _argc: c_uint, vp: *mut jsval) -> JSBool {
    unsafe {
        let obj = JS_THIS_OBJECT(cx, cast::reinterpret_cast(&vp));
        if obj.is_null() {
            return 0;
        }

        let bundle = unwrap(obj);
        let nodeType = do (*bundle).payload.node.read |nd| {
            match nd.kind {
              ~Element(*) => 1,
              ~Text(*)    => 3,
              ~Comment(*) => 8,
              ~Doctype(*) => 10
            }
        };
        *vp = RUST_INT_TO_JSVAL(nodeType);
    }
    return 1;
}
