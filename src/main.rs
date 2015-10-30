
extern crate irc;
extern crate nanomsg;

extern crate bender; // interface to plugins

use bender::*;

use std::default::Default;
use irc::client::prelude as client;
use irc::client::server::Server;
use irc::client::server::utils::ServerExt;
use std::sync::Arc;

pub type NetIrcServer = irc::client::server::NetIrcServer;
pub type Message = irc::client::data::Message;

// TODO: parse config from a file, if asked on the command-line?

/// Create the configuration
pub fn mk_config() -> client::Config {
    let mut c: client::Config = Default::default();
    c.nickname = Some("bender".to_string());
    c.realname = Some("bender".to_string());
    c.use_ssl = Some(true);
    c.server = Some("irc.rezosup.net".to_string());
    c
}

/// Handle a received message, dispatching it to plugins
pub fn handle_msg(
    conn: &NetIrcServer,
    plugins: &mut PluginSet,
    msg: Message
) -> Result<()> {
    // TODO
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
    g.join(); // wait for thread
    Ok(())
}

fn main() {
    main_loop().unwrap_or_else(|e| {
        println!("error: {}", e);
        std::process::exit(1);
    });
}
