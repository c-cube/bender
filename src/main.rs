
extern crate irc;
extern crate nanomsg;
extern crate rustc_serialize;
extern crate clap;

extern crate bender; // interface to plugins

use bender::*;

use std::default::Default;
use std::path::{Path, PathBuf};
use irc::client::prelude as client;
use irc::client::server::Server;
use irc::client::server::utils::ServerExt;
use std::time::Duration;
use std::sync::Arc;
use std::io::{Read,Write};
use rustc_serialize::json;
use nanomsg::{Socket,Protocol,Endpoint};

pub type Message = irc::client::data::Message;

// TODO: parse config from a file, if asked on the command-line?

/// A connection to a plugin
pub struct PluginConn {
    path: PathBuf, // path of the plugin program
    subproc: std::process::Child, // the subprocess
}

impl PluginConn {
    /// Spawn the plugin at this given
    pub fn spawn(p: &Path) -> Result<PluginConn> {
        use std::process::Command;
        let subproc = try!(Command::new(p).spawn());
        Ok(PluginConn {
            path: p.to_path_buf(),
            subproc: subproc,
        })
    }
}

pub struct PluginSetPull {
    buf: String, // buffer for reading
    pull: Socket, // Get commands
    endpoint: Endpoint,
}

impl PluginSetPull {
    fn new() -> Result<PluginSetPull> {
        let mut pull = try!(Socket::new(Protocol::Pull));
        let p2b = "ipc:///tmp/plugin2bender.ipc";
        let endpoint = try!(pull.bind(p2b));
        try!(set_777(&p2b[6..]));
        Ok(PluginSetPull {
            buf: String::with_capacity(256),
            pull: pull,
            endpoint: endpoint,
        })
    }

    /// Read a command sent by some plugin
    pub fn recv_command(&mut self) -> Result<Command> {
        self.buf.clear();
        try!(self.pull.read_to_string(&mut self.buf));
        let cmd: Command = try!(json::decode(&self.buf));
        Ok(cmd)
    }

    /// read commands, and give every command to `f`
    fn listen<F>(mut self, f: F) where F: Fn(Command) + 'static {
        loop {
            match self.recv_command() {
                Err(ref e) => println!("error on command: {:?}", e),
                Ok(c) => f(c),
            }
        }
    }

    /// Spawn a new thread that listens on the socket
    pub fn spawn_listen<F>(self, f: F) -> std::thread::JoinHandle<()>
    where F: Fn(Command) + Sync + Send + 'static
    {
        std::thread::spawn(move || { self.listen(f) })
    }
}

impl Drop for PluginSetPull {
    fn drop(&mut self) { self.endpoint.shutdown().unwrap(); }
}

pub struct PluginSetPush {
    push: Socket, // broadcast
    endpoint: Endpoint,
}

impl PluginSetPush {
    /// Create an empty set of plugins.
    pub fn new() -> Result<PluginSetPush> {
        let mut push = try!(Socket::new(Protocol::Pub));
        let b2p = "ipc:///tmp/bender2plugin.ipc";
        let endpoint = try!(push.bind(b2p));
        try!(set_777(&b2p[6..]));
        Ok(PluginSetPush {
            push: push,
            endpoint: endpoint,
        })
    }

    /// Transmit an event to the plugin.
    pub fn send_event(&mut self, msg: Event) -> Result<()> {
        let json = json::encode(&msg).unwrap();
        println!("sending event {:?}", msg);
        try!(self.push.write(json.as_bytes()));
        try!(self.push.flush());
        println!("sent event");
        Ok(())
    }
}

impl Drop for PluginSetPush {
    fn drop(&mut self) { self.endpoint.shutdown().unwrap(); }
}

/// A Set of plugins
pub struct PluginSet {
    plugins: Vec<PluginConn>,
    push: PluginSetPush,
}

impl PluginSet {
    /// Create an empty set of plugins.
    pub fn new() -> Result<(PluginSet, PluginSetPull)> {
        let push = try!(PluginSetPush::new());
        let pull = try!(PluginSetPull::new());
        Ok((PluginSet {
            plugins: Vec::new(),
            push: push,
        }, pull))
    }

    /// Start the plugin at path and add it to
    /// the set of monitored plugins
    pub fn start<P: AsRef<Path>>(&mut self, p: P) -> Result<()> {
        let p = p.as_ref();
        let plugin_conn = try!(PluginConn::spawn(p));
        self.plugins.push(plugin_conn);
        Ok(())
    }
}

/// Create the configuration
pub fn mk_config() -> client::Config {
    client::Config {
        nickname: Some("bender".to_string()),
        realname: Some("bender".to_string()),
        use_ssl: Some(true),
        server: Some("irc.rezosup.net".to_string()),
        .. Default::default()
    }
}

/// Handle a received message, dispatching it to plugins
pub fn handle_msg(
    plugins: &mut PluginSet,
    msg: Message
) -> Result<()> {
    // TODO
    match Event::from_message(msg) {
        Ok(event) => try!(plugins.push.send_event(event)),
        Err(msg) => println!("unhandled message {:?}", msg)
    }
    Ok(())
}

/// Main listening loop
pub fn main_loop(conf: Option<&str>,
                 plugin_path: Option<&str>)
                 -> Result<()> {
    let c = match conf {
        None => mk_config(),
        Some(_) => unimplemented!()
    };
    let conn = Arc::new(try!(client::IrcServer::from_config(c)));
    try!(conn.identify());
    // spawn thread to join chan after 2s
    let g = {
        let conn2 = conn.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::new(2, 0));
            // FIXME: looks like on some conditions we don't sleep long
            // enough and are unable to join the channel, but hard to reproduce
            println!("join #sac");
            conn2.send_join("#sac").unwrap();
        })
    };

    let conn3 = conn.clone();
    let (mut plugins, pull) = try!(PluginSet::new());

    if let Some(path) = plugin_path {
        try!(plugins.start(&path));
    }

    // listen for commands from plugins
    let g_listen = pull.spawn_listen(move |c:Command| {
        println!("bender: received command {:?}", c);
        match c {
            Command::Privmsg {to, content} =>
                conn3.send_privmsg(to.as_str(), &content).unwrap(),
            _ => (), // TODO
        }
    });

    for msg in conn.iter() {
        let msg = try!(msg);
        try!(handle_msg(&mut plugins, msg));
    }
    g.join().unwrap(); // wait for thread
    g_listen.join().unwrap();
    Ok(())
}

fn main() {
    let matches = clap::App::new("bender")
        .version("0.1")
        .author("<simon.cruanes@m4x.org> and <shuba@melix.net>")
        .about("irc bot multiplexer")
        .args_from_usage(
            "-r --run-plugin=[RUN_PLUGIN] 'Launch the specified plugin'
             -c --config=[CONFIG] 'Path to the configuration file'")
        .get_matches();

    main_loop(matches.value_of("CONFIG"),
              matches.value_of("RUN_PLUGIN")).unwrap_or_else(|e| {
        println!("error: {}", e);
        std::process::exit(1);
    });
}
