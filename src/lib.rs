// Interface between the IRC bot, and its plugins

extern crate nanomsg;
extern crate rustc_serialize;  // serialization into/from JSON

use std::io::{Read,Write};
use std::sync::Arc;
use std::vec::Vec;
use nanomsg::{Socket,Protocol,Endpoint};
use rustc_serialize::json;
use rustc_serialize::json::Json;

/// The Error type for Bender
#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Nano(nanomsg::Error),
    Serialize(String), // printed error, don't bother with subtleties
}

impl std::fmt::Display for Error {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter
    ) -> std::result::Result<(), std::fmt::Error> {
        match *self {
            Error::IO(ref e) => write!(out, "IO error: {}", e),
            Error::Nano(ref e) => write!(out, "nanomsg error: {}", e),
            Error::Serialize(ref e) => write!(out, "serialize error: {}", e),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error { Error::IO(e) }
}
impl From<nanomsg::Error> for Error {
    fn from(e: nanomsg::Error) -> Error { Error::Nano(e) }
}
impl From<json::DecoderError> for Error {
    fn from(e: json::DecoderError) -> Error {
        Error::Serialize(e.to_string()) }
}
impl From<json::EncoderError> for Error {
    fn from(e: json::EncoderError) -> Error {
        Error::Serialize(e.to_string()) }
}
impl From<json::ParserError> for Error {
    fn from(e: json::ParserError) -> Error {
        Error::Serialize(e.to_string()) }
}

/// Make `Error` a proper error type
impl std::error::Error for Error {
    fn description(&self) -> &str {
        match *self{
            Error::IO(ref e) => e.description(),
            Error::Nano(ref e) => e.description(),
            Error::Serialize(ref e) => e,
        }
    }
    fn cause(&self) -> Option<&std::error::Error> {
        match *self {
            Error::IO(ref e) => Some(e),
            Error::Nano(ref e) => Some(e),
            Error::Serialize(_) => None,
        }
    }
}

/// `Result`, specialized for `bender::Error`.
pub type Result<T> = std::result::Result<T, Error>;

/// An event witnessed by Bender, transmitted to the plugins
#[derive(RustcDecodable, RustcEncodable,Clone)]
pub enum Event {
    Privmsg {from: String, content: String},
    Joined {chan: String},
}

/// A command sent by a plugin, to Bender
#[derive(RustcDecodable, RustcEncodable,Clone,Debug)]
pub enum Command {
    Privmsg {to: String, content: String}, // send a message
    Join {chan: String},  // join channel
    Part {chan: String},  // quit chan
    Reconnect, // reconnect to IRC
    Exit, // disconnect
}

/// A (connection to a) plugin
pub struct Plugin {
    buf: String, // buffer for reading
    pull: Socket, // Get commands
    endpoint: Endpoint,
    path: String, // path of the plugin program
    subproc: std::process::Output, // the subprocess
}

impl Plugin {
    /// Spawn the plugin at this given
    pub fn spawn(p: &str) -> Result<Plugin> {
        use std::process::Command;
        let subproc = try!(Command::new(p).output());
        let mut pull = try!(Socket::new(Protocol::Pull));
        let mut endpoint = try!(pull.bind("ipc:///tmp/bender.ipc"));
        Ok(Plugin {
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
}

/// A Set of plugins
pub struct PluginSet {
    push: Socket, // broadcast
    endpoint: Endpoint,
    plugins: Vec<Plugin>,
}

impl PluginSet {
    /// Create an empty set of plugins.
    pub fn new() -> Result<PluginSet> {
        let mut push = try!(Socket::new(Protocol::Push));
        let mut endpoint = try!(push.bind("ipc:///tmp/bender.ipc"));
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

// ## Communication from Bender to Plugin





