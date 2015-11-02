// Example plugin: answers "world" when it receives a privmsg containing "hello"

extern crate bender;

use bender::*;

fn run() -> Result<()> {
    let (mut push, mut pull) = try!(connect_server());
    for e in pull {
        match e {
            Event::Privmsg {from, content} =>
                if content.contains("hello") {
                    let c = Command::Privmsg{to:from, content:"world".to_string()};
                    try!(push.send_command(c));
                },
            _ => (),
        }
    };
    unreachable!();
}

/// Run and listen for events
fn main() {
    match run() {
        Ok(()) => (),
        Err(ref e) => {
            println!("error: {}", e);
            std::process::exit(1)
        },
    }
}


