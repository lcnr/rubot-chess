use std::io;
use std::time::Duration;

use rubot::{Bot, Game};

macro_rules! log {
    ($($src:tt)*) => {
        {
            use std::io::Write;
            writeln!(std::fs::OpenOptions::new().create(true).append(true).open("./log").unwrap(), $($src)*).unwrap();
        }
    };
}

use shakmaty::{fen::Fen, uci::Uci, Color, Move, MoveList, Outcome, Position, Role, Setup};

/// this example requires a newtype due to orphan rules, as both shakmaty::Chess and rubot::Game
/// are from a different crate
#[derive(Debug, Clone, Default)]
struct Chess(shakmaty::Chess);

impl Game for Chess {
    type Player = Color;
    type Action = Move;
    type Actions = MoveList;
    type Fitness = i32;

    fn actions(&self, player: &Self::Player) -> (bool, Self::Actions) {
        (*player == self.0.turn(), self.0.legals())
    }

    fn execute(&mut self, action: &Self::Action, player: &Self::Player) -> Self::Fitness {
        self.0.play_unchecked(action);

        if let Some(outcome) = self.0.outcome() {
            match outcome {
                Outcome::Draw => 0,
                Outcome::Decisive { winner } => {
                    if winner == *player {
                        std::i32::MAX
                    } else {
                        std::i32::MIN
                    }
                }
            }
        } else {
            let mut fitness = 0;
            for (_square, piece) in self.0.board().pieces() {
                // values based on https://medium.freecodecamp.org/simple-chess-ai-step-by-step-1d55a9266977
                let value = match piece.role {
                    Role::Pawn => 10,
                    Role::Knight => 30,
                    Role::Bishop => 30,
                    Role::Rook => 50,
                    Role::Queen => 90,
                    Role::King => 900,
                };

                if piece.color == *player {
                    fitness += value;
                } else {
                    fitness -= value;
                }
            }
            fitness
        }
    }
}

use vampirc_uci::{UciMessage, UciTimeControl};

fn respond(msg: UciMessage) {
    println!("{}", msg);
}

fn main() {
    let mut game = Chess::default();
    let mut bot = Bot::new(Color::Black);
    loop {
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");
        for message in vampirc_uci::parse(&input) {
            let clone = message.clone();
            match message {
                UciMessage::Uci => {
                    respond(UciMessage::Id {
                        name: Some("rubot".to_owned()),
                        author: Some("lncr/Bastian Kauschke".to_owned()),
                    });
                    respond(UciMessage::UciOk);
                }
                UciMessage::IsReady => {
                    respond(UciMessage::ReadyOk);
                }
                UciMessage::UciNewGame => {
                    game = Chess(shakmaty::Chess::default());
                }
                UciMessage::Position {
                    startpos,
                    fen,
                    moves,
                } => {
                    if startpos {
                        game = Chess(shakmaty::Chess::default());
                    } else if let Some(fen) = fen {
                        game = Chess(
                            shakmaty::Chess::from_setup(
                                &Fen::from_ascii(fen.as_str().as_bytes()).unwrap(),
                            )
                            .unwrap(),
                        );
                    }

                    for mov in moves {
                        let mov = &Uci::from_ascii(mov.to_string().as_bytes())
                            .unwrap()
                            .to_move(&game.0)
                            .unwrap();
                        game.0.play_unchecked(mov);
                    }

                    bot = rubot::Bot::new(game.0.turn());
                }
                UciMessage::Go {
                    time_control,
                    search_control,
                } => {
                    let mut move_time = 5000;
                    if let Some(time_control) = time_control {
                        match time_control {
                            UciTimeControl::Ponder | UciTimeControl::Infinite => {
                                log!("ERROR: can't handle this right now: {}", clone)
                            }
                            UciTimeControl::TimeLeft {
                                white_time,
                                black_time,
                                white_increment,
                                black_increment,
                                moves_to_go,
                            } => {
                                if moves_to_go.is_some() {
                                    log!("ERROR: can't handle this right now: {}", clone)
                                }

                                if game.0.turn() == Color::Black {
                                    if let (Some(bt), Some(bi)) = (black_time, black_increment) {
                                        move_time = std::cmp::min(bi / 2 + bt / 20, 7000 + bi);
                                    }
                                } else {
                                    if let (Some(wt), Some(wi)) = (white_time, white_increment) {
                                        move_time = std::cmp::min(wi / 2 + wt / 20, 7000 + wi);
                                    }
                                }
                            }
                            UciTimeControl::MoveTime(time) => move_time = time,
                        }
                    }

                    if let Some(search_control) = search_control {
                        if !search_control.search_moves.is_empty()
                            || search_control.mate.is_some()
                            || search_control.depth.is_some()
                            || search_control.nodes.is_some()
                        {
                            log!("ERROR: can't handle this right now: {}", clone)
                        }
                    }

                    println!(
                        "bestmove {}",
                        Uci::from_move(
                            &game.0,
                            &bot.select(&game, Duration::from_millis(move_time)).unwrap()
                        )
                    );
                }
                UciMessage::Quit => {
                    std::process::exit(0);
                }
                _ => log!("ERROR: can't handle this right now: {}", clone),
            }
        }
    }
}
