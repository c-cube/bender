// Interface between the IRC bot, and its plugins

extern crate nanomsg;
extern crate irc;
extern crate rustc_serialize;  // serialization into/from JSON

use std::io::{Read,Write};
use nanomsg::{Socket,Protocol,Endpoint};
use rustc_serialize::json;

pub type IrcMessage = irc::client::data::Message;

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

/// An irc endpoint is either a user or a channel,
/// it can be the source of messages, or the sink of messages
#[derive(RustcDecodable, RustcEncodable, Clone, Debug)]
pub enum IrcEndPoint {
    Chan(String),
    User(String),
}

impl IrcEndPoint {
    /// str representation of self
    pub fn as_str(& self) -> &str {
        match *self {
            IrcEndPoint::Chan(ref s) => s,
            IrcEndPoint::User(ref s) => s,
        }
    }

    /// Create an endpoint by parsing a string
    /// channels are identified by the leading #
    pub fn from_strings(irc_arg: String, prefix: String) -> IrcEndPoint {
        if irc_arg.starts_with("#") {
            IrcEndPoint::Chan(irc_arg)
        }
        else {
            let from_user = prefix.split("!").next()
                                  .expect("privmsg prefixes should contain !")
                                  .to_string();
            IrcEndPoint::User(from_user)
        }
    }
}

/// An event witnessed by Bender, transmitted to the plugins
#[derive(RustcDecodable, RustcEncodable, Debug, Clone)]
pub enum Event {
    Privmsg {from: IrcEndPoint, content: String},
    Joined {chan: String},
}

impl Event {
    pub fn from_message(msg: IrcMessage)
    -> std::result::Result<Self, IrcMessage>
    {
        match msg {
            IrcMessage {
                tags,
                prefix: Some(prefix),
                command,
                mut args,
                suffix: Some(suffix),
            } => {
                println!("prefix: {:?}", prefix);
                match command.as_ref() {
                    "PRIVMSG" if args.len() > 0 => {
                        let arg0 = args.swap_remove(0);
                        Ok(Event::Privmsg {
                            from: IrcEndPoint::from_strings(arg0, prefix),
                            content: suffix
                        })
                    },
                    "JOIN" => {
                        Ok(Event::Joined {
                            chan: suffix
                        })
                    },
                    _ => Err(IrcMessage {
                        tags: tags,
                        prefix: Some(prefix),
                        command: command,
                        args: args,
                        suffix: Some(suffix)
                    })
                }
            },
            _ => Err(msg)
        }
    }
}

unsafe impl Send for Event {}

/// A command sent by a plugin, to Bender
#[derive(RustcDecodable, RustcEncodable,Clone,Debug)]
pub enum Command {
    Privmsg {to: IrcEndPoint, content: String}, // send a message
    Join {chan: String},  // join channel
    Part {chan: String},  // quit chan
    Reconnect, // reconnect to IRC
    Exit, // disconnect
}

unsafe impl Send for Command {}

pub struct ServerConnPush {
    push : Socket,
    push_endpoint: Endpoint,
}

impl ServerConnPush {
    /// Send command
    pub fn send_command(&mut self, c: Command) -> Result<()> {
        println!("send command {:?}", c);
        let json = json::encode(&c).unwrap();
        try!(self.push.write(json.as_bytes()));
        try!(self.push.flush());
        println!("sent command");
        Ok(())
    }
}

impl Drop for ServerConnPush {
    fn drop(&mut self) { self.push_endpoint.shutdown().unwrap(); }
}

pub struct ServerConnPull {
    buf: String,
    pull: Socket,
    pull_endpoint: Endpoint,
}

impl ServerConnPull {
    /// Read next event
    pub fn recv_event(&mut self) -> Result<Event> {
        self.buf.clear();
        try!(self.pull.read_to_string(&mut self.buf));
        let e: Event = try!(json::decode(&self.buf));
        Ok(e)
    }
}

impl Iterator for ServerConnPull {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        self.recv_event().ok()
    }
}

impl Drop for ServerConnPull {
    fn drop(&mut self) {
        self.pull_endpoint.shutdown().unwrap();
    }
}

/// Connection to the server.
///
/// A plugin might be simply written as
/// ```
/// fn plugin() {
///   let (push, pull) = connect_server();
///   for e in pull {
///     match e {
///       Privmsg(from, body) =>
///         push.send_command(Privmsg {to: from, body: body }).unwrap(),
///       _ => (),
///     }
///   }
/// }
/// ```
pub fn connect_server() -> Result<(ServerConnPush, ServerConnPull)> {
    let mut push = try!(Socket::new(Protocol::Push));
    let push_endpoint = try!(push.connect("ipc:///tmp/plugin2bender.ipc"));
    let mut pull = try!(Socket::new(Protocol::Sub));
    let pull_endpoint = try!(pull.connect("ipc:///tmp/bender2plugin.ipc"));
    try!(pull.subscribe(""));
    Ok((ServerConnPush {
            push: push,
            push_endpoint: push_endpoint,
        }, ServerConnPull {
            buf: String::with_capacity(256),
            pull: pull,
            pull_endpoint: pull_endpoint,
        }
    ))
}



