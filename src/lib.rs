// Interface between the IRC bot, and its plugins

extern crate nanomsg;
extern crate rustc_serialize;  // serialization into/from JSON

use std::sync::Arc;
use std::vec::Vec;
use nanomsg::{Socket,Protocol,Endpoint};
use rustc_serialize::json;

/// The Error type for Bender
#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Nano(nanomsg::Error),
}

impl std::fmt::Display for Error {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter
    ) -> std::result::Result<(), std::fmt::Error> {
        match *self {
            Error::IO(ref e) => write!(out, "IO error: {}", e),
            Error::Nano(ref e) => write!(out, "nanomsg error: {}", e),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error { Error::IO(e) }
}
impl From<nanomsg::Error> for Error {
    fn from(e: nanomsg::Error) -> Error { Error::Nano(e) }
}

/// Make `Error` a proper error type
impl std::error::Error for Error {
    fn description(&self) -> &str {
        match *self{
            Error::IO(ref e) => e.description(),
            Error::Nano(ref e) => e.description(),
        }
    }
    fn cause(&self) -> Option<&std::error::Error> {
        match *self {
            Error::IO(ref e) => Some(e),
            Error::Nano(ref e) => Some(e),
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
            pull: pull,
            endpoint: endpoint,
            path: p.to_string(),
            subproc: subproc,
        })
    }
}

/// A Set of plugins
pub struct PluginSet {
    push: Socket, // broadcast
    endpoint: Endpoint,
    plugins: Vec<Plugin>,
}

impl PluginSet {
    /// Create an empty set
    pub fn new() -> Result<PluginSet> {
        let mut push = try!(Socket::new(Protocol::Push));
        let mut endpoint = try!(push.bind("ipc:///tmp/bender.ipc"));
        Ok(PluginSet {
            plugins: Vec::new(),
            push: push,
            endpoint: endpoint,
        })
    }

}

// ## Communication from Bender to Plugin





