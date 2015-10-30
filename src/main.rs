
extern crate irc;

use std::default::Default;
use irc::client::prelude as client;
use irc::client::server::Server;
use irc::client::server::utils::ServerExt;
use std::sync::Arc;

pub type NetIrcServer = irc::client::server::NetIrcServer;

pub fn mk_config() -> client::Config {
    let mut c: client::Config = Default::default();
    c.nickname = Some("bender".to_string());
    c.realname = Some("bender".to_string());
    c.use_ssl = Some(true);
    c.server = Some("irc.rezosup.net".to_string());
    c
}

/// Main listening loop
pub fn main_loop() -> std::io::Result<()> {
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
    for msg in conn.iter() {
        let msg = try!(msg);
        println!("received message {:?}", msg);
    }
    g.join(); // wait for thread
    Ok(())
}

fn main() {
    main_loop().unwrap();
}
