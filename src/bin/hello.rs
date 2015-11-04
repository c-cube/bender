// Example plugin: answers "world!" when it receives
// a privmsg containing "hello"

extern crate bender;

use bender::*;

fn run() -> Result<()> {
    let (mut push, pull) = try!(connect_server());
    println!("plugin `hello`: connected");
    for e in pull {
        match e {
            Event::Privmsg {from, content} => {
                println!("plugin `hello`: received privmsg {} from {:?}",
                         content, from);
                if content.contains("hello") {
                    let c = Command::Privmsg{
                        to: from,
                        content: "world!".to_string()
                    };
                    try!(push.send_command(c));
                }
            },
            _ => (),
        }
    };
    unreachable!();
}

/// Run and listen for events
fn main() {
    std::thread::sleep_ms(1000);
    match run() {
        Ok(()) => (),
        Err(ref e) => {
            println!("error: {}", e);
            std::process::exit(1)
        },
    }
}


