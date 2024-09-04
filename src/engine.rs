use cozy_chess::{self, util, Board, Color, GameStatus, Move, Piece, PieceMoves};
use rand::{seq::SliceRandom, thread_rng};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub struct GoInfo {
    pub wtime: Option<u32>,
    pub btime: Option<u32>,
    pub winc: Option<u32>,
    pub binc: Option<u32>,
    pub moves_to_go: Option<u32>,
    pub depth: Option<u32>,
    pub nodes: Option<u32>,
    pub mate: Option<u32>,
    pub movetime: Option<u32>,
    pub infinite: bool,
}
macro_rules! find_arg {
    ($split : ident , $x: expr, $y : ty) => {
        if $split.contains(&$x) {
            let x = $split.iter().position(|&r| r == $x).unwrap() + 1;
            Some($split[x].parse::<$y>().unwrap())
        } else {
            None
        }
    };
}

impl GoInfo {
    pub fn new(input: String) -> Self {
        let split: Vec<&str> = input.split(' ').collect();
        let out = Self {
            wtime: find_arg!(split, "wtime", u32),
            btime: find_arg!(split, "btime", u32),
            winc: find_arg!(split, "winc", u32),
            binc: find_arg!(split, "binc", u32),
            moves_to_go: find_arg!(split, "movestogo", u32),
            depth: find_arg!(split, "depth", u32),
            nodes: find_arg!(split, "nodes", u32),
            mate: find_arg!(split, "mate", u32),
            movetime: find_arg!(split, "movetime", u32),
            infinite: {
                if split.contains(&"infinite") {
                    true
                } else {
                    false
                }
            },
        };
        out
    }
}

#[derive(Clone)]
pub struct Engine {
    search_info: SearchInfo,
    shared: Arc<Mutex<Shared>>,
    board: Board,
    my_side: Color,
    search_stack: Vec<SearchStack>,
}

#[derive(Clone, Copy)]
pub struct SearchStack {
    board_hash: u64,
}
impl Default for SearchStack {
    fn default() -> Self {
        Self {
            board_hash: Default::default(),
        }
    }
}
#[derive(Clone, Copy)]
struct SearchInfo {
    nodes: u64,
    tt_hits: u64,
    cutoffs: u64,
}

impl SearchInfo {
    pub fn new() -> Self {
        Self {
            nodes: 0,
            tt_hits: 0,
            cutoffs: 0,
        }
    }
    pub fn reset(&mut self) {
        self.nodes = 0;
        self.tt_hits = 0;
        self.cutoffs = 0;
    }
}
pub struct Shared {
    pub stop: bool,
}
pub enum EndCondition {
    Time(Instant),
    Nodes(u64),
    Depth(u8),
    Infinite,
}

impl EndCondition {
    pub fn met(&self, nodes: u64, depth: u8) -> bool {
        match self {
            EndCondition::Time(end_time) => Instant::now() >= *end_time, // did we hit the time condition?
            EndCondition::Nodes(node_count) => nodes >= *node_count, // did we hit the node condition?
            EndCondition::Depth(depth_to_reach) => depth == *depth_to_reach, // did we hit the depth condition?
            EndCondition::Infinite => false, // you can never meet the end condition for infinity >:)
        }
    }
}
impl Engine {
    pub fn new(shared: Arc<Mutex<Shared>>) -> Engine {
        Engine {
            search_info: SearchInfo::new(),
            shared,
            board: Board::startpos(),
            my_side: Color::White,
            search_stack: vec![],
        }
    }

    pub fn setup_newgame(&mut self) {
        self.board = Board::startpos();
        self.search_info.reset();
    }

    pub fn set_position(&mut self, input: String) {
        let is_startpos = input.contains("startpos");
        if is_startpos {
            self.board = Board::startpos();
        } else {
            let end_index = if input.contains("moves") {
                input.find("moves").unwrap()
            } else {
                input.len()
            };
            let start_index = input.find("fen").unwrap() + 4;
            self.board = Board::from_fen(&input[start_index..end_index], false).unwrap();
        }
        if input.contains("moves") {
            let begin_index = input.find("moves").unwrap() + 6;
            let moves: Vec<&str> = input[begin_index..input.len()].split(' ').collect();
            // println!("{:?}", moves);
            for mov in moves {
                // println!("|{mov}|");
                self.board
                    .play(util::parse_uci_move(&self.board, mov).unwrap());
            }
        }

        self.my_side = self.board.side_to_move();
    }
    pub fn find_best_move(&mut self, info: &GoInfo) -> Move {
        self.search_info.reset();
        // find run mode amongst : {infinite, time, depth, nodes, movetime}
        let end_cond;
        let t1 = Instant::now();
        if info.infinite {
            end_cond = EndCondition::Infinite;
        } else if let Some(nodes) = info.nodes {
            end_cond = EndCondition::Nodes(nodes as u64);
        } else if let Some(depth) = info.depth {
            end_cond = EndCondition::Depth(depth as u8);
        } else if let Some(movetime) = info.movetime {
            end_cond = EndCondition::Time(t1 + Duration::from_millis(movetime.into()));
        } else if let (Some(btime), Some(wtime)) = (info.btime, info.wtime) {
            let (binc, winc) = if let (Some(binc), Some(winc)) = (info.binc, info.winc) {
                (binc, winc)
            } else {
                (0, 0)
            };
            let my_time;
            let other_time;
            match self.my_side {
                Color::Black => {
                    my_time = btime;
                    other_time = wtime;
                }
                Color::White => {
                    my_time = wtime;
                    other_time = btime;
                }
            };
            let time_left = if other_time < my_time {
                (((my_time - other_time) * 3) / 4) + my_time / 10
            } else {
                my_time / 10
            };
            end_cond = EndCondition::Time(Instant::now() + Duration::from_millis(time_left.into()));
        } else {
            panic!("No end condition findable!");
        }
        self.search_stack = vec![Default::default(); 16];
        let (out, score) = self.negamax(&self.board.clone(), &end_cond, 3);

        let time = t1.elapsed().as_millis();
        let nps = self.search_info.nodes * 1000 / (time as u64 + 1);
        println!(
            "info nodes {} time {} score {} nps {}",
            self.search_info.nodes, time, score, nps,
        );

        return out.unwrap();
    }

    fn negamax(
        &mut self,
        board: &Board,
        end_condition: &EndCondition,
        depth: u8,
    ) -> (Option<Move>, i32) {
        // end_condition.met(self.search_info.nodes, 0) ||
        if self.shared.lock().unwrap().stop {
            return (None, 0);
        }
        let cur_hash = board.hash();
        self.search_stack[depth as usize].board_hash = cur_hash;
        if depth == 0 || board.status() != GameStatus::Ongoing {
            return match board.status() {
                GameStatus::Drawn => (None, 0),
                GameStatus::Won => {
                    if board.side_to_move() == Color::White {
                        (None, 1000)
                    } else {
                        (None, -1000)
                    }
                }
                GameStatus::Ongoing => (None, Self::evaluate(&board)),
            };
        // }
        } else if self.search_stack[(depth as usize + 1)..]
            .iter()
            .any(|item| item.board_hash == cur_hash)
        {
            return (None, 0); // detect repeated position in current search line, return draw if found
        }

        let mut max_score = -1000;
        let mut best_move = None;
        let mut moves = vec![];
        board.generate_moves(|mves| {
            for mv in mves {
                moves.push(mv);
            }
            false
        });
        for mv in moves {
            self.search_info.nodes += 1;
            let mut board = board.clone();
            board.play_unchecked(mv);
            let (_, score) = self.negamax(&board, end_condition, depth - 1);
            let score = -score;
            if score > max_score {
                max_score = score;
                best_move = Some(mv);
                // alpha = alpha.max(score);
            }
        }
        (best_move, max_score)
    }

    fn evaluate(board: &Board) -> i32 {
        let who_movin = if (board.side_to_move() == Color::White) {
            1
        } else {
            -1
        };
        let white_material = board.colors(Color::White).len() as i32;
        let black_material = board.colors(Color::Black).len() as i32;
        if (white_material != black_material) {
            // println!("GHEHE");
        }
        return (white_material - black_material) * who_movin;
    }
}

#[allow(dead_code)]
fn perft(board: &Board, depth: u8) -> u64 {
    if depth == 0 {
        1
    } else {
        let mut nodes = 0;
        board.generate_moves(|moves| {
            for mv in moves {
                let mut board = board.clone();
                board.play_unchecked(mv);
                nodes += perft(&board, depth - 1);
            }
            false
        });
        nodes
    }
}

#[cfg(test)]
mod tests {
    use cozy_chess::Board;

    use super::perft;

    #[test]
    fn perft_all() {
        let tests = [
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                vec![1_u64, 20, 400, 8902, 197281, 4865609],
            ),
            (
                "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
                vec![1, 48, 2039, 97862, 4085603],
            ),
        ];
        for (fen, counts) in tests {
            for (depth, count) in counts.iter().enumerate() {
                let mut board = Board::from_fen(fen, false).unwrap();
                let nodes = perft(&mut board, depth as u8);
                println!("{}, depth {}, nodes {}", fen, depth, nodes);
                assert_eq!(nodes, *count);
            }
        }
    }
}
