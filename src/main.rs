
extern crate irc;
extern crate nanomsg;
extern crate rustc_serialize;

extern crate bender; // interface to plugins

use bender::*;

use std::default::Default;
use irc::client::prelude as client;
use irc::client::server::Server;
use irc::client::server::utils::ServerExt;
use std::sync::Arc;
use std::io::{Read,Write};
use rustc_serialize::json;
use nanomsg::{Socket,Protocol,Endpoint};

pub type NetIrcServer = irc::client::server::NetIrcServer;
pub type Message = irc::client::data::Message;

// TODO: parse config from a file, if asked on the command-line?

/// A connection to a plugin
pub struct PluginConn {
    path: String, // path of the plugin program
    subproc: std::process::Output, // the subprocess
}

impl PluginConn {
    /// Spawn the plugin at this given
    pub fn spawn(p: &str) -> Result<PluginConn> {
        use std::process::Command;
        let subproc = try!(Command::new(p).output());
        Ok(PluginConn {
            path: p.to_string(),
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
        let endpoint = try!(pull.bind("ipc:///tmp/plugin2bender.ipc"));
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
                Err(ref e) => (), // TODO: print error?
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
        let endpoint = try!(push.bind("ipc:///tmp/bender2plugin.ipc"));
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
    pull: PluginSetPull,
}

impl PluginSet {
    /// Create an empty set of plugins.
    pub fn new() -> Result<PluginSet> {
        let mut push = try!(PluginSetPush::new());
        let mut pull = try!(PluginSetPull::new());
        Ok(PluginSet {
            plugins: Vec::new(),
            push: push,
            pull: pull,
        })
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
    conn: &NetIrcServer,
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
pub fn main_loop() -> Result<()> {
    let c = mk_config();
    let conn = Arc::new(try!(client::IrcServer::from_config(c)));
    try!(conn.identify());
    // spawn thread to join chan after 2s
    let g = {
        let conn2 = conn.clone();
        std::thread::spawn(move || {
            std::thread::sleep_ms(2000);
            println!("join #sac");
            conn2.send_join("#sac").unwrap();
        })
    };
    let conn3 = conn.clone();
    let mut plugins = try!(PluginSet::new());
    for msg in conn.iter() {
        let msg = try!(msg);
        try!(handle_msg(&conn, &mut plugins, msg));
    }
    // listen for commands from plugins
    let g_listen = plugins.pull.spawn_listen(move |c:Command| {
        println!("bender: received command {:?}", c);
        match c {
            Command::Privmsg {to, content} =>
                conn3.send_privmsg(to.as_str(), &content).unwrap(),
            _ => (), // TODO
        }
    });
    g.join().unwrap(); // wait for thread
    g_listen.join().unwrap();
    Ok(())
}

fn main() {
    main_loop().unwrap_or_else(|e| {
        println!("error: {}", e);
        std::process::exit(1);
    });
}
