/*!

A task that takes a URL and streams back the binary data

*/

export ControlMsg, Load, Exit;
export ProgressMsg, Payload, Done;
export ResourceTask, ResourceManager, LoaderTaskFactory;

use comm::{Chan, Port};
use task::{spawn, spawn_listener};
use std::net::url;
use std::net::url::{Url, to_str};

enum ControlMsg {
    /// Request the data associated with a particular URL
    Load(Url, Chan<ProgressMsg>),
    Exit
}

/// Messages sent in response to a `Load` message
enum ProgressMsg {
    /// Binary data - there may be multiple of these
    Payload(~[u8]),
    /// Indicates loading is complete, either successfully or not
    Done(Result<(), ()>)
}

impl ProgressMsg: cmp::Eq {
    pure fn eq(other: &ProgressMsg) -> bool {
        match (copy self, copy *other) {
          (Payload(a), Payload(b)) => a == b,
          (Done(a), Done(b)) => a == b,

          (Payload(*), _)
          | (Done(*), _) => false
        }
    }
    pure fn ne(other: &ProgressMsg) -> bool {
        return !self.eq(other);
    }
}

/// Handle to a resource task
type ResourceTask = Chan<ControlMsg>;

/**
Creates a task to load a specific resource

The ResourceManager delegates loading to a different type of loader task for
each URL scheme
*/
type LoaderTaskFactory = fn~(+url: Url, Chan<ProgressMsg>);

/// Create a ResourceTask with the default loaders
fn ResourceTask() -> ResourceTask {
    let loaders = ~[
        (~"file", file_loader::factory),
        (~"http", http_loader::factory)
    ];
    create_resource_task_with_loaders(loaders)
}

fn create_resource_task_with_loaders(+loaders: ~[(~str, LoaderTaskFactory)]) -> ResourceTask {
    do spawn_listener |from_client| {
        // TODO: change copy to move once we can move into closures
        ResourceManager(from_client, copy loaders).start()
    }
}

struct ResourceManager {
    from_client: Port<ControlMsg>,
    /// Per-scheme resource loaders
    loaders: ~[(~str, LoaderTaskFactory)],
}


fn ResourceManager(from_client: Port<ControlMsg>, 
                   loaders: ~[(~str, LoaderTaskFactory)]) -> ResourceManager {
    ResourceManager {
        from_client : from_client,
        loaders : loaders,
    }
}


impl ResourceManager {
    fn start() {
        loop {
            match self.from_client.recv() {
              Load(url, progress_chan) => {
                self.load(copy url, progress_chan)
              }
              Exit => {
                break
              }
            }
        }
    }

    fn load(+url: Url, progress_chan: Chan<ProgressMsg>) {

        match self.get_loader_factory(url) {
          Some(loader_factory) => {
            #debug("resource_task: loading url: %s", to_str(copy url));
            loader_factory(url, progress_chan);
          }
          None => {
            #debug("resource_task: no loader for scheme %s", url.scheme);
            progress_chan.send(Done(Err(())));
          }
        }
    }

    fn get_loader_factory(url: Url) -> Option<LoaderTaskFactory> {
        for self.loaders.each |scheme_loader| {
            let (scheme, loader_factory) = copy *scheme_loader;
            if scheme == url.scheme {
                return Some(loader_factory);
            }
        }
        return None;
    }
}

#[test]
fn test_exit() {
    let resource_task = ResourceTask();
    resource_task.send(Exit);
}

#[test]
#[allow(non_implicitly_copyable_typarams)]
fn test_bad_scheme() {
    let resource_task = ResourceTask();
    let progress = Port();
    resource_task.send(Load(url::from_str(~"bogus://whatever").get(), progress.chan()));
    match progress.recv() {
      Done(result) => { assert result.is_err() }
      _ => fail
    }
    resource_task.send(Exit);
}

#[test]
#[allow(non_implicitly_copyable_typarams)]
fn should_delegate_to_scheme_loader() {
    let payload = ~[1, 2, 3];
    let loader_factory = fn~(+_url: Url, progress_chan: Chan<ProgressMsg>, copy payload) {
        progress_chan.send(Payload(copy payload));
        progress_chan.send(Done(Ok(())));
    };
    let loader_factories = ~[(~"snicklefritz", loader_factory)];
    let resource_task = create_resource_task_with_loaders(loader_factories);
    let progress = Port();
    resource_task.send(Load(url::from_str(~"snicklefritz://heya").get(), progress.chan()));
    assert progress.recv() == Payload(payload);
    assert progress.recv() == Done(Ok(()));
    resource_task.send(Exit);
}
