mod engine;

use std::{
    io,
    sync::{mpsc::channel, Arc, Mutex},
    thread,
};

use engine::{Engine, GoInfo, Shared};

fn get_input() -> String {
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("error: unable to read user input");
    input.trim().to_string()
}
fn main() {
    loop {
        if get_input() != "uci" {
            println!("Invalid protocol!");
        } else {
            break;
        }
    }

    println!("id name tunaturk");
    println!("id author BrokenKeyboard");

    println!("uciok");

    let (send, recv) = channel::<SearchMessage>();
    let shared = Arc::new(Mutex::new(Shared { stop: false }));
    let shared_for_thread = Arc::clone(&shared);
    thread::spawn(move || {
        let mut search = Engine::new(Arc::clone(&shared_for_thread));
        while let Ok(message) = recv.recv() {
            match message {
                SearchMessage::NewGame => {
                    shared_for_thread.lock().expect("error").stop = false;
                    search.setup_newgame();
                }
                SearchMessage::Go(things) => {
                    let best_move = search.find_best_move(&things);
                    println!("bestmove {}", best_move);
                }
                SearchMessage::SetPosition(info) => {
                    search.set_position(info);
                }
                SearchMessage::Ready => {
                    println!("readyok");
                }
            }
        }
    });
    // send readyok
    // loop with a match for all the uai commands
    loop {
        let t = get_input();
        let input = t.trim();
        match input.split(' ').next().unwrap() {
            "ucinewgame" => {
                send.send(SearchMessage::NewGame).unwrap();
            }
            "position" => {
                send.send(SearchMessage::SetPosition(String::from(
                    input.get(9..).unwrap(),
                )))
                .unwrap();
            }

            "go" => send
                .send(SearchMessage::Go(GoInfo::new(String::from(
                    input.get(2..).unwrap(),
                ))))
                .unwrap(),
            "stop" => {
                shared.lock().unwrap().stop = true;
            }
            "isready" => {
                send.send(SearchMessage::Ready).unwrap();
            }
            "ponderhit" => todo!(),
            "quit" => {
                break;
            }
            _ => {}
        }
    }
}

enum SearchMessage {
    NewGame,
    SetPosition(String),
    Go(GoInfo),
    Ready,
}
