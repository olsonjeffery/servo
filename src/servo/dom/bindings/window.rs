use js::rust::{bare_compartment, methods};
use js::{JS_ARGV, JSCLASS_HAS_RESERVED_SLOTS, JSPROP_ENUMERATE, JSPROP_SHARED, JSVAL_NULL,
            JS_THIS_OBJECT, JS_SET_RVAL};
use js::jsapi::{JSContext, jsval, JSObject, JSBool, jsid, JSClass, JSFreeOp};
use js::jsapi::bindgen::{JS_ValueToString, JS_GetStringCharsZAndLength, JS_ReportError,
                            JS_GetReservedSlot, JS_SetReservedSlot, JS_NewStringCopyN,
    JS_DefineFunctions, JS_DefineProperty, JS_DefineProperties, JS_EncodeString, JS_free};
use js::glue::bindgen::*;
use js::global::jsval_to_rust_str;
use js::crust::{JS_PropertyStub, JS_StrictPropertyStub, JS_EnumerateStub, JS_ConvertStub, JS_ResolveStub};
use js::glue::bindgen::RUST_JSVAL_TO_INT;
use ptr::null;
use libc::c_uint;
use utils::{rust_box, squirrel_away, jsval_to_str};
use bindings::node::create;
use dom::window::{Window, TimerMessage_Fire};
use dom::node::Node;
use dvec::DVec;

extern fn alert(cx: *JSContext, argc: c_uint, vp: *jsval) -> JSBool {
  unsafe {
    let argv = JS_ARGV(cx, vp);
    assert (argc == 1);
    // Abstract this pattern and use it in debug, too?
    let jsstr = JS_ValueToString(cx, *ptr::offset(argv, 0));
    // Right now, just print to the console
    io::println(#fmt("ALERT: %s", jsval_to_rust_str(cx, jsstr)));
    JS_SET_RVAL(cx, vp, JSVAL_NULL);
  }
  1_i32
}

// Holder for the various JS values associated with setTimeout
// (ie. function value to invoke and all arguments to pass
//      to the function when calling it)
struct TimerData {
    funval: jsval,
    args: DVec<jsval>,
}

fn TimerData(argc: c_uint, argv: *jsval) -> TimerData unsafe {
    let data = TimerData {
        funval : *argv,
        args : DVec(),
    };

    let mut i = 2;
    while i < argc as uint {
        data.args.push(*ptr::offset(argv, i));
        i += 1;
    };

    data
}


extern fn setTimeout(cx: *JSContext, argc: c_uint, vp: *jsval) -> JSBool unsafe {
    let argv = JS_ARGV(cx, vp);
    assert (argc >= 2);

    //TODO: don't crash when passed a non-integer value for the timeout

    // Post a delayed message to the per-window timer task; it will dispatch it
    // to the relevant content handler that will deal with it.
    std::timer::delayed_send(std::uv_global_loop::get(),
                             RUST_JSVAL_TO_INT(*ptr::offset(argv, 1)) as uint,
                             (*unwrap(JS_THIS_OBJECT(cx, vp))).payload.timer_chan,
                             TimerMessage_Fire(~TimerData(argc, argv)));

    JS_SET_RVAL(cx, vp, JSVAL_NULL);
    return 1;
}

unsafe fn unwrap(obj: *JSObject) -> *rust_box<Window> {
    let val = JS_GetReservedSlot(obj, 0);
    cast::reinterpret_cast(&RUST_JSVAL_TO_PRIVATE(val))
}

extern fn finalize(_fop: *JSFreeOp, obj: *JSObject) {
    #debug("finalize!");
    unsafe {
        let val = JS_GetReservedSlot(obj, 0);
        let _: @Window = cast::reinterpret_cast(&RUST_JSVAL_TO_PRIVATE(val));
    }
}

fn init(compartment: bare_compartment, win: @Window) {
    let proto = utils::define_empty_prototype(~"Window", None, compartment);
    compartment.register_class(utils::instance_jsclass(~"WindowInstance", finalize));

    let obj = result::unwrap(
                 compartment.new_object_with_proto(~"WindowInstance",
                                                   ~"Window", null()));

    /* Define methods on a window */
    let methods = ~[{name: compartment.add_name(~"alert"),
                     call: {op: alert, info: null()},
                     nargs: 1,
                     flags: 0,
                     selfHostedName: null()},
                    {name: compartment.add_name(~"setTimeout"),
                     call: {op: setTimeout, info: null()},
                     nargs: 2,
                     flags: 0,
                     selfHostedName: null()}];

    vec::as_imm_buf(methods, |fns, _len| {
        JS_DefineFunctions(compartment.cx.ptr, proto.ptr, fns);
    });

    unsafe {
        let raw_ptr: *libc::c_void = cast::reinterpret_cast(&squirrel_away(win));
        JS_SetReservedSlot(obj.ptr, 0, RUST_PRIVATE_TO_JSVAL(raw_ptr));
    }

    //TODO: All properties/methods on Window need to be available on the global
    //      object as well. We probably want a special JSClass with a resolve hook.
    compartment.define_property(~"window", RUST_OBJECT_TO_JSVAL(obj.ptr),
                                JS_PropertyStub, JS_StrictPropertyStub,
                                JSPROP_ENUMERATE);
}
