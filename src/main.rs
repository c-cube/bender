
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
    buf: String, // buffer for reading
    pull: Socket, // Get commands
    endpoint: Endpoint,
    path: String, // path of the plugin program
    subproc: std::process::Output, // the subprocess
}

impl PluginConn {
    /// Spawn the plugin at this given
    pub fn spawn(p: &str) -> Result<PluginConn> {
        use std::process::Command;
        let subproc = try!(Command::new(p).output());
        let mut pull = try!(Socket::new(Protocol::Pull));
        let endpoint = try!(pull.bind("ipc:///tmp/plugin2bender.ipc"));
        Ok(PluginConn {
            buf: String::with_capacity(256),
            pull: pull,
            endpoint: endpoint,
            path: p.to_string(),
            subproc: subproc,
        })
    }

    /// Read a command sent by the plugin
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

impl Drop for PluginConn {
    fn drop(&mut self) { self.endpoint.shutdown().unwrap(); }
}

/// A Set of plugins
pub struct PluginSet {
    push: Socket, // broadcast
    endpoint: Endpoint,
    plugins: Vec<PluginConn>,
}

impl PluginSet {
    /// Create an empty set of plugins.
    pub fn new() -> Result<PluginSet> {
        let mut push = try!(Socket::new(Protocol::Push));
        let endpoint = try!(push.bind("ipc:///tmp/bender2plugin.ipc"));
        Ok(PluginSet {
            plugins: Vec::new(),
            push: push,
            endpoint: endpoint,
        })
    }

    /// Transmit an event to the plugin.
    pub fn send_event(&mut self, msg: Event) -> Result<()> {
        let json = json::encode(&msg).unwrap();
        try!(self.push.write(json.as_bytes()));
        Ok(())
    }
}

impl Drop for PluginSet {
    fn drop(&mut self) { self.endpoint.shutdown().unwrap(); }
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
    let event = match msg.command.as_ref() {
        "PRIVMSG" => {
            Event::Privmsg {
                from: IrcEndPoint::from_string(msg.args[0].clone()),
                content: msg.suffix.expect("empty private message!!")
            }
        },
        "JOIN" => {
            Event::Joined { chan: msg.suffix.expect("empty joined message!!") }
        },
        _ => return Ok(())
    };
    println!("event {:?}", event);
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
    let mut plugins = try!(PluginSet::new());
    for msg in conn.iter() {
        let msg = try!(msg);
        try!(handle_msg(&conn, &mut plugins, msg));
    }
    g.join().unwrap(); // wait for thread
    Ok(())
}

fn main() {
    main_loop().unwrap_or_else(|e| {
        println!("error: {}", e);
        std::process::exit(1);
    });
}
