/*!
The content task is the main task that runs JavaScript and spawns layout
tasks.
*/

export Content, ContentTask;
export ControlMsg, ExecuteMsg, ParseMsg, ExitMsg, Timer;
export PingMsg, PongMsg;
export task_from_context;

use std::arc::{ARC, clone};
use comm::{Port, Chan, listen, select2};
use task::{spawn, spawn_listener};
use io::{read_whole_file, println};

use dom::document::Document;
use dom::node::{Node, NodeScope, define_bindings};
use dom::event::{Event, ResizeEvent, ReflowEvent};
use dom::window::Window;
use gfx::compositor::Compositor;
use html::lexer::spawn_html_lexer_task;
use layout::layout_task;
use layout_task::{LayoutTask, BuildMsg};
use resource::image_cache_task::ImageCacheTask;

use css::styles::Stylesheet;

use jsrt = js::rust::rt;
use js::rust::{cx, methods};
use js::global::{global_class, debug_fns};

use either::{Either, Left, Right};

use dom::bindings::utils::rust_box;
use js::rust::compartment;

use resource::resource_task;
use resource_task::{ResourceTask};

use std::net::url::Url;
use url_to_str = std::net::url::to_str;
use util::url::make_url;
use task::{task, SingleThreaded};

use js::glue::bindgen::RUST_JSVAL_TO_OBJECT;
use js::JSVAL_NULL;
use js::jsapi::{JSContext, jsval};
use js::jsapi::bindgen::{JS_CallFunctionValue, JS_GetContextPrivate};
use ptr::null;

enum ControlMsg {
    ParseMsg(Url),
    ExecuteMsg(Url),
    Timer(~dom::bindings::window::TimerData),
    ExitMsg
}

enum PingMsg {
    PongMsg
}

type ContentTask = Chan<ControlMsg>;

fn ContentTask<S: Compositor Send Copy>(layout_task: LayoutTask,
                                        +compositor: S,
                                        resource_task: ResourceTask,
                                        img_cache_task: ImageCacheTask) -> ContentTask {
    do task().sched_mode(SingleThreaded).spawn_listener::<ControlMsg> |from_master| {
        let content = Content(layout_task, from_master, resource_task, img_cache_task);
        compositor.add_event_listener(content.event_port.chan());
        content.start();
    }
}

struct Content {
    layout_task: LayoutTask,
    image_cache_task: ImageCacheTask,
    from_master: comm::Port<ControlMsg>,
    event_port: comm::Port<Event>,

    scope: NodeScope,
    jsrt: jsrt,
    cx: cx,

    mut document: Option<@Document>,
    mut window:   Option<@Window>,
    mut doc_url: Option<Url>,

    resource_task: ResourceTask,

    compartment: Option<compartment>,
}

fn Content(layout_task: LayoutTask, 
           from_master: Port<ControlMsg>,
           resource_task: ResourceTask,
           img_cache_task: ImageCacheTask) -> Content {
    
    let jsrt = jsrt();
    let cx = jsrt.cx();
    let event_port = Port();

    cx.set_default_options_and_version();
    cx.set_logging_error_reporter();

    let compartment = match cx.new_compartment(global_class) {
          Ok(c) => Some(c),
          Err(()) => None
    };

    let content = Content {
        layout_task : layout_task,
        image_cache_task : img_cache_task,
        from_master : from_master,
        event_port : event_port,

        scope : NodeScope(),
        jsrt : jsrt,
        cx : cx,

        document : None,
        window   : None,
        doc_url  : None,

        resource_task : resource_task,
        compartment : compartment
    };

    cx.set_cx_private(ptr::to_unsafe_ptr(&content) as *());

    content
}

fn task_from_context(cx: *JSContext) -> &Content unsafe {
    cast::reinterpret_cast(&JS_GetContextPrivate(cx))
}

impl Content {

    fn start() {
        while self.handle_msg(select2(self.from_master, self.event_port)) {
            // Go on...
        }
    }

    fn handle_msg(msg: Either<ControlMsg,Event>) -> bool {
        match msg {
            Left(control_msg) => self.handle_control_msg(control_msg),
            Right(event) => self.handle_event(event)
        }
    }

    fn handle_control_msg(control_msg: ControlMsg) -> bool {
        match control_msg {
          ParseMsg(url) => {
            debug!("content: Received url `%s` to parse", url_to_str(copy url));

            // Note: we can parse the next document in parallel
            // with any previous documents.

            let result = html::hubbub_html_parser::parse_html(self.scope,
                                                              url,
                                                              self.resource_task,
                                                              self.image_cache_task);

            let root = result.root;
            let css_rules = result.style_port.recv();
            let js_scripts = result.js_port.recv();

            // Apply the css rules to the dom tree:
            debug!("css_rules: %?", css_rules);

            debug!("js_scripts: %?", js_scripts);

            let document = Document(root, self.scope, css_rules);
            let window   = Window(self.from_master);
            self.relayout(document, &url);
            self.document = Some(@document);
            self.window   = Some(@window);
            self.doc_url = Some(copy url);

            let compartment = option::expect(self.compartment, ~"TODO error checking");
            compartment.define_functions(debug_fns);
            define_bindings(*compartment,
                            option::get(self.document),
                            option::get(self.window));

            for vec::each(js_scripts) |bytes| {
                self.cx.evaluate_script(compartment.global_obj, *bytes, ~"???", 1u);
            }

            return true;
          }

          Timer(timerData) => {
            let compartment = option::expect(self.compartment, ~"TODO error checking");
            let thisValue = if timerData.args.len() > 0 {
                RUST_JSVAL_TO_OBJECT(unsafe { timerData.args.shift() })
            } else {
                compartment.global_obj.ptr
            };
            let _rval = JSVAL_NULL;
            //TODO: support extra args. requires passing a *jsval argv
            JS_CallFunctionValue(self.cx.ptr, thisValue, timerData.funval,
                                 0, null(), ptr::addr_of(_rval));
            self.relayout(*option::get(self.document), &option::get(self.doc_url));
            return true;
          }


          ExecuteMsg(url) => {
            debug!("content: Received url `%s` to execute", url_to_str(copy url));

            match read_whole_file(&Path(url.path)) {
              Err(msg) => {
                println(fmt!("Error opening %s: %s", url_to_str(copy url), msg));
              }
              Ok(bytes) => {
                let compartment = option::expect(self.compartment, ~"TODO error checking");
                compartment.define_functions(debug_fns);
                self.cx.evaluate_script(compartment.global_obj, bytes, url.path, 1u);
              }
            }
            return true;
          }

          ExitMsg => {
            self.layout_task.send(layout_task::ExitMsg);
            return false;
          }
        }
    }

    /**
       Sends a ping to layout and waits for the response (i.e., it has finished any
       pending layout request messages).
    */
    fn join_layout() {
        if self.scope.is_reader_forked() {
            listen(|response_from_layout| {
                self.layout_task.send(layout_task::PingMsg(response_from_layout));
                response_from_layout.recv();
            });
            self.scope.reader_joined();
        }
    }

    /**
       This method will wait until the layout task has completed its current action,
       join the layout task, and then request a new layout run. It won't wait for the
       new layout computation to finish.
    */
    fn relayout(document: Document, doc_url: &Url) {
        debug!("content: performing relayout");

        // Now, join the layout so that they will see the latest
        // changes we have made.
        self.join_layout();

        // Send new document and relevant styles to layout
        // FIXME: Put CSS rules in an arc or something.
        self.layout_task.send(BuildMsg(document.root, clone(&document.css_rules), copy *doc_url, self.event_port.chan()));

        // Indicate that reader was forked so any further
        // changes will be isolated.
        self.scope.reader_forked();
    }

     fn query_layout(query: layout_task::LayoutQuery) -> layout_task::LayoutQueryResponse {
         self.relayout(*self.document.get(), &self.doc_url.get());
         self.join_layout();
         
         let response_port = Port();
         self.layout_task.send(layout_task::QueryMsg(query, response_port.chan()));
         return response_port.recv()
    }

    /**
       This is the main entry point for receiving and dispatching DOM events.
    */
    // TODO: actually perform DOM event dispatch.
    fn handle_event(event: Event) -> bool {
        match event {
          ResizeEvent(new_width, new_height) => {
            debug!("content got resize event: %d, %d", new_width, new_height);
            match copy self.document {
                None => {
                    // Nothing to do.
                }
                Some(document) => {
                    assert self.doc_url.is_some();
                    self.relayout(*document, &self.doc_url.get());
                }
            }
            return true;
          }
          ReflowEvent => {
            debug!("content got reflow event");
            match copy self.document {
                None => {
                    // Nothing to do.
                }
                Some(document) => {
                    assert self.doc_url.is_some();
                    self.relayout(*document, &self.doc_url.get());
                }
            }
            return true;
          }
        }
    }
}
