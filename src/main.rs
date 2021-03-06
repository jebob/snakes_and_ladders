mod dice;

use crate::boards::Board;
use crate::sim::Sim;
use crate::BadRouteError::BadRoute;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{fmt, fs};

mod boards {
    use crate::{BadRoute, BadRouteError};
    use std::collections::HashMap;

    #[derive(Debug, Clone)]
    pub struct Board {
        pub size: usize,
        pub routes: HashMap<usize, usize>, // Snakes AND Ladders in Source: Destination order
    }

    impl Board {
        pub fn new(size: usize, routes: HashMap<usize, usize>) -> Result<Board, BadRouteError> {
            // Do some validation first
            for (&from, &to) in routes.iter() {
                if (from == 0) | (from >= size) {
                    return Err(BadRoute(format!(
                        "Illegal snake/ladder start position: {}",
                        from
                    )));
                }
                if to > size {
                    return Err(BadRoute(format!(
                        "Illegal snake/ladder end position: {}",
                        to
                    )));
                }
                if from == to {
                    return Err(BadRoute(format!(
                        "Snake or ladder links to itself on square {}",
                        from
                    )));
                }
            }

            Ok(Board { size, routes })
        }
    }

    #[allow(dead_code)]
    pub(crate) fn blank(size: usize) -> Board {
        Board::new(size, HashMap::new()).unwrap()
    }

    /// Returns the board from the prompt
    #[allow(dead_code)]
    pub(crate) fn canon_board() -> Board {
        Board::new(
            100,
            HashMap::from([
                // snakes go down
                (27, 5),
                (40, 3),
                (43, 18),
                (54, 31),
                (66, 45),
                (76, 58),
                (89, 53),
                (99, 41),
                // ladders go up
                (4, 25),
                (13, 46),
                (33, 49),
                (42, 63),
                (50, 69),
                (62, 81),
                (74, 92),
            ]),
        )
        .unwrap()
    }

    #[cfg(test)]
    mod test {
        use crate::boards::*;

        #[test]
        fn test_blank() {
            let _ = blank(100);
        }
        #[test]
        fn test_canon_board() {
            let _ = canon_board();
        }
    }
}

#[derive(Debug)]
pub enum BadRouteError {
    BadRoute(String),
}
impl std::error::Error for BadRouteError {}

impl fmt::Display for BadRouteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BadRouteError::BadRoute(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ConfigFile {
    iterations: usize,
    size: usize,
    snakes: Vec<(usize, usize)>,
    ladders: Vec<(usize, usize)>,
}

fn load_cfg(file: &str) -> Result<(Board, usize), Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(file)?;
    let v: ConfigFile = serde_json::from_str(&contents)?;
    if v.snakes.iter().any(|el| el.0 < el.1) {
        return Err(Box::new(BadRoute(
            "Some snake(s) are going upwards!".to_string(),
        )));
    };
    if v.ladders.iter().any(|el| el.0 > el.1) {
        return Err(Box::new(BadRoute(
            "Some ladders(s) are going downwards!".to_string(),
        )));
    };
    let mut routes_vec = v.snakes.clone();
    routes_vec.extend(v.ladders.clone());

    let mut routes = HashMap::new();
    for (from, to) in routes_vec {
        if routes.contains_key(&from) {
            return Err(Box::new(BadRoute(format!(
                "Duplicate snake or ladder from square {}",
                from
            ))));
        }
        routes.insert(from, to);
    }
    Ok((Board::new(v.size, routes)?, v.iterations))
}

mod sim {
    use crate::dice::{Roll, DIE_SIZE};
    use crate::Board;
    use std::cmp::{max, Ordering};
    use std::collections::HashSet;

    pub struct Sim {
        board: Board,
        position: usize,
        rng: Box<dyn Roll>,
        lucky_spaces: HashSet<usize>,
        unlucky_spaces: HashSet<usize>,
        // stats
        pub turn_count: usize,
        pub roll_count: usize,
        pub climb_count: usize,
        pub slide_count: usize,
        pub climb_distance: usize,
        pub slide_distance: usize,
        pub biggest_climb: usize,
        pub biggest_slide: usize,
        pub longest_turn: Vec<usize>,
        pub lucky_rolls: usize,
        pub unlucky_rolls: usize,
    }

    struct RollResult {
        die_value: usize,
        climb_distance: usize,
        slide_distance: usize,
    }

    /// Implementation detail for (un)lucky rolls
    /// Currently a roll is (un)lucky iff it lands on an (un)lucky space
    fn calc_lucky_spaces(board: &Board) -> (HashSet<usize>, HashSet<usize>) {
        let mut lucky_spaces: HashSet<usize> = HashSet::new();
        let mut unlucky_spaces: HashSet<usize> = HashSet::new();
        for i in 0..board.size {
            // lucky or unlucky if ladder or snake.
            match board.routes.get(&i).unwrap_or(&i).cmp(&i) {
                Ordering::Greater => {
                    lucky_spaces.insert(i);
                }
                Ordering::Less => {
                    unlucky_spaces.insert(i);
                }
                Ordering::Equal => {}
            }
            // Check for snake near-miss
            for delta in [-2, -1, 1, 2] {
                let other_i = i as isize + delta;
                if other_i <= 0 {
                    continue; // Underflow, so ignore
                }
                let other_i = other_i as usize;
                let route_outcome = *board.routes.get(&other_i).unwrap_or(&other_i);
                if route_outcome < other_i {
                    // Rolled onto a position that was next to a snake leading downwards
                    lucky_spaces.insert(i);
                    break;
                }
            }
        }
        // Finally, the winning space is lucky
        lucky_spaces.insert(board.size);
        (lucky_spaces, unlucky_spaces)
    }

    impl Sim {
        pub fn new(board: Board, rng: Box<dyn Roll>) -> Sim {
            // Pre-calculate (un)lucky spaces
            let (lucky_spaces, unlucky_spaces) = calc_lucky_spaces(&board);

            Sim {
                board,
                position: 0,
                rng,
                lucky_spaces,
                unlucky_spaces,
                turn_count: 0,
                roll_count: 0,
                climb_count: 0,
                slide_count: 0,
                climb_distance: 0,
                slide_distance: 0,
                biggest_climb: 0,
                biggest_slide: 0,
                longest_turn: vec![],
                lucky_rolls: 0,
                unlucky_rolls: 0,
            }
        }

        fn has_won(&self) -> bool {
            self.position == self.board.size
        }

        /// Take turns until has_won()
        pub fn run(&mut self) {
            // Add a max_turns constraint? Not all possible boards are winnable.
            while !self.has_won() {
                self.turn()
            }
        }

        /// Roll once, and keep rolling if we get DIE_SIZE. Stop immediately if we've won.
        fn turn(&mut self) {
            self.turn_count += 1;
            let mut turn_climb = 0;
            let mut turn_slide = 0;
            let mut die_rolls: Vec<usize> = vec![];
            while !self.has_won() {
                let result = self.roll();
                turn_climb += result.climb_distance;
                turn_slide += result.slide_distance;
                die_rolls.push(result.die_value);
                if result.die_value < DIE_SIZE {
                    break;
                };
            }
            // Store turn stats
            self.biggest_climb = max(self.biggest_climb, turn_climb);
            self.biggest_slide = max(self.biggest_slide, turn_slide);
            if die_rolls > self.longest_turn {
                self.longest_turn = die_rolls
            };
        }

        /// Roll the die once and resolve the consequences
        fn roll(&mut self) -> RollResult {
            // Not the same as Roll::roll
            let die_value = self.rng.roll();
            self.roll_resolve(die_value)
        }

        /// Try to move forwards some spaces
        fn roll_resolve(&mut self, die_value: usize) -> RollResult {
            self.roll_count += 1;
            let rolled_position = self.position + die_value;
            if rolled_position > self.board.size {
                // Illegal move!
                return RollResult {
                    die_value,
                    climb_distance: 0,
                    slide_distance: 0,
                };
            }

            // Try to follow any routes (snake or ladder)
            self.position = rolled_position;
            self.follow_routes();
            let (climb_distance, slide_distance) = if self.position > rolled_position {
                (self.position - rolled_position, 0)
            } else {
                (0, rolled_position - self.position)
            };

            if self.is_unlucky_roll(&rolled_position) {
                // Note "unlucky" trumps lucky.
                // If you miss a snake (lucky) and land on another (unlucky) that feels unlucky
                self.unlucky_rolls += 1
            } else if self.is_lucky_roll(&rolled_position) {
                self.lucky_rolls += 1
            }
            RollResult {
                die_value,
                climb_distance,
                slide_distance,
            }
        }

        /// Follow snakes and ladders from the current position
        /// Can follow multiple snakes/ladders
        fn follow_routes(&mut self) {
            let mut new_position = self.position;
            while let Some(p) = self.board.routes.get(&new_position) {
                if *p > new_position {
                    // ladder
                    let delta = *p - new_position;
                    self.climb_count += 1;
                    self.climb_distance += delta;
                } else {
                    // snake
                    let delta = new_position - *p;
                    self.slide_count += 1;
                    self.slide_distance += delta;
                }
                new_position = *p
            }
            self.position = new_position;
            // Don't return value to ensure sliding/climbing is an atomic action
        }

        fn is_lucky_roll(&self, rolled_position: &usize) -> bool {
            // We are lucky if we land on a lucky position i.e.
            // a ladder
            // or just missing a snake
            // or winning
            self.lucky_spaces.contains(rolled_position)
        }

        fn is_unlucky_roll(&self, rolled_position: &usize) -> bool {
            // We are unlucky if we land on an unlucky position i.e.
            // a snake
            self.unlucky_spaces.contains(rolled_position)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::boards::{blank, canon_board};
        use crate::dice::{MockDie, Unrollable};
        use std::collections::{HashMap, HashSet};

        /// Check can move forwards
        #[test]
        fn test_roll_movement() {
            let mut sim = Sim::new(blank(20), Box::new(Unrollable {}));
            assert_eq!(sim.position, 0, "Should start at zero");
            sim.roll_resolve(5);
            assert_eq!(sim.position, 5);
            sim.roll_resolve(1);
            assert_eq!(sim.position, 6);
        }

        /// Over-rolling should not result in movement or winning
        #[test]
        fn test_roll_over_rolling() {
            let mut sim = Sim::new(blank(20), Box::new(Unrollable {}));
            sim.roll_resolve(9999);
            assert_eq!(sim.position, 0);
            assert!(!sim.has_won(), "Not on the victory space, but has_won?");
        }

        #[test]
        fn test_roll_winning() {
            let mut sim = Sim::new(blank(20), Box::new(Unrollable {}));
            sim.roll_resolve(20); // Perfect roll!
            assert_eq!(sim.position, 20); // End of board
            assert!(sim.has_won(), "At winning position, but not has_won?");

            // test post-victory behaviour
            sim.roll_resolve(1);
            assert_eq!(sim.position, 20, "Moved after winning, illegal");
        }

        /// Check can generate a plausible random move
        #[test]
        fn test_random_roll() {
            let max_rolls = 10; // 10 times is good enough
            let board = blank(max_rolls * DIE_SIZE); // Make a big enough board
            let mut sim = Sim::new(board.clone(), Box::new(rand::thread_rng()));
            for _ in 0..max_rolls {
                let old_position = sim.position;
                let result = sim.roll();
                println!("Rolled a {}", result.die_value); // Maybe useful for debugging
                assert!(
                    1 <= result.die_value,
                    "Die value {} is < 1",
                    result.die_value
                );
                assert!(
                    result.die_value <= DIE_SIZE,
                    "Die value {} is > {}",
                    result.die_value,
                    DIE_SIZE
                );
                assert_eq!(sim.position, old_position + result.die_value);
            }
        }

        #[test]
        fn test_lucky_spaces() {
            // If rules for luck changes, should replace this with checking rolls.
            let board = Board::new(20, HashMap::from([(5, 8), (14, 2)])).unwrap();
            let sim = Sim::new(board, Box::new(Unrollable {}));
            assert_eq!(
                sim.lucky_spaces,
                HashSet::from([
                    5, // Ladders up
                    12, 13, 15, 16, // near a snake
                    20  // Winning square
                ])
            );
            assert_eq!(sim.unlucky_spaces, HashSet::from([14]));
        }

        #[test]
        fn test_canon_board_speedrun() {
            // More fun than useful!
            // Can probably be deleted if the canon_board changes
            let b = canon_board();
            let rng = Box::new(MockDie {
                queued_results: vec![2, 6, 5, 1, 2, 6, 4],
            });
            let mut sim = Sim::new(b, rng);
            sim.run();
            assert_eq!(sim.roll_count, 7);
            assert_eq!(sim.turn_count, 5);
            assert_eq!(sim.climb_count, 4);
            assert_eq!(sim.slide_count, 0);
            assert_eq!(sim.climb_distance, 74);
            assert_eq!(sim.slide_distance, 0);
            assert_eq!(sim.biggest_climb, 21);
            assert_eq!(sim.biggest_slide, 0);
            assert_eq!(sim.lucky_rolls, 6);
            assert_eq!(sim.unlucky_rolls, 0);
            assert!(sim.has_won());
        }

        #[test]
        fn test_chained_slides() {
            // Take one step forwards and fall down a chain of snakes
            // then re-roll and go down another snake
            let b = Board::new(100, HashMap::from([(99, 60), (60, 30), (30, 2), (5, 1)])).unwrap();
            let rng = Box::new(MockDie {
                queued_results: vec![3, 6],
            });
            let mut sim = Sim::new(b, rng);
            sim.position = 93; // Override position
            sim.turn();
            assert_eq!(sim.roll_count, 2);
            assert_eq!(sim.turn_count, 1);
            assert_eq!(sim.climb_count, 0);
            assert_eq!(sim.slide_count, 4);
            assert_eq!(sim.climb_distance, 0);
            assert_eq!(sim.slide_distance, 101);
            assert_eq!(sim.biggest_climb, 0);
            assert_eq!(sim.biggest_slide, 101);
            assert_eq!(sim.lucky_rolls, 0);
            assert_eq!(sim.unlucky_rolls, 2);
            assert_eq!(sim.longest_turn, vec![6, 3]);
            assert!(!sim.has_won());
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
struct MultiSimResult {
    min_rolls: usize,
    avg_rolls: f64,
    max_rolls: usize,
    min_climb: usize, // Total distance
    avg_climb: f64,
    max_climb: usize,
    min_slide: usize,
    avg_slide: f64,
    max_slide: usize,
    biggest_turn_climb: usize, // Greatest climb in a single turn, INCLUDING re-rolls and chains
    biggest_turn_slide: usize, // Greatest slide in a single turn, INCLUDING re-rolls and chains
    longest_turn: Vec<usize>,  // Longest die rolls e.g. [6,5] < [6,6,2] < [6,6,3]
    min_lucky_rolls: usize,
    avg_lucky_rolls: f64,
    max_lucky_rolls: usize,
    min_unlucky_rolls: usize,
    avg_unlucky_rolls: f64,
    max_unlucky_rolls: usize,
}

impl MultiSimResult {
    fn from_sims(sims: &[Sim]) -> MultiSimResult {
        let (min_rolls, avg_rolls, max_rolls) =
            min_avg_max(sims.iter().map(|s| s.roll_count).collect()).unwrap();
        let (min_climb, avg_climb, max_climb) =
            min_avg_max(sims.iter().map(|s| s.climb_distance).collect()).unwrap();
        let (min_slide, avg_slide, max_slide) =
            min_avg_max(sims.iter().map(|s| s.slide_distance).collect()).unwrap();
        let (min_lucky_rolls, avg_lucky_rolls, max_lucky_rolls) =
            min_avg_max(sims.iter().map(|s| s.lucky_rolls).collect()).unwrap();
        let (min_unlucky_rolls, avg_unlucky_rolls, max_unlucky_rolls) =
            min_avg_max(sims.iter().map(|s| s.unlucky_rolls).collect()).unwrap();
        MultiSimResult {
            min_rolls,
            avg_rolls,
            max_rolls,
            min_climb,
            avg_climb,
            max_climb,
            min_slide,
            avg_slide,
            max_slide,
            biggest_turn_climb: sims.iter().map(|s| s.biggest_climb).max().unwrap(),
            biggest_turn_slide: sims.iter().map(|s| s.biggest_slide).max().unwrap(),
            longest_turn: sims.iter().map(|s| s.longest_turn.clone()).max().unwrap(),
            min_lucky_rolls,
            avg_lucky_rolls,
            max_lucky_rolls,
            min_unlucky_rolls,
            avg_unlucky_rolls,
            max_unlucky_rolls,
        }
    }
}

fn min_avg_max(sequence: Vec<usize>) -> Option<(usize, f64, usize)> {
    if sequence.is_empty() {
        None
    } else {
        Some((
            *sequence.iter().min().unwrap(),
            sequence.iter().sum::<usize>() as f64 / sequence.len() as f64,
            *sequence.iter().max().unwrap(),
        ))
    }
}

#[cfg(test)]
mod tests_stats {
    use super::*;
    use crate::boards::canon_board;
    use crate::dice::Unrollable;
    #[test]
    fn test_min_max_average_empty() {
        assert!(min_avg_max(vec![]).is_none());
    }
    #[test]
    fn test_min_max_average_singleton() {
        assert_eq!(min_avg_max(vec![5]).unwrap(), (5, 5.0, 5));
    }
    #[test]
    fn test_min_max_average_fraction() {
        assert_eq!(min_avg_max(vec![8, 0, 3]).unwrap(), (0, 11.0 / 3.0, 8));
    }
    #[test]
    fn test_empty_multi_sim_result() {
        let b = boards::blank(100);
        let rng = Box::new(Unrollable {});
        let sim = Sim::new(b, rng);
        let result: MultiSimResult = MultiSimResult::from_sims(&vec![sim]);
        assert_eq!(
            result,
            MultiSimResult {
                min_rolls: 0,
                avg_rolls: 0.0,
                max_rolls: 0,
                min_climb: 0,
                avg_climb: 0.0,
                max_climb: 0,
                min_slide: 0,
                avg_slide: 0.0,
                max_slide: 0,
                biggest_turn_climb: 0,
                biggest_turn_slide: 0,
                longest_turn: vec![],
                min_lucky_rolls: 0,
                avg_lucky_rolls: 0.0,
                max_lucky_rolls: 0,
                min_unlucky_rolls: 0,
                avg_unlucky_rolls: 0.0,
                max_unlucky_rolls: 0
            }
        )
    }
    #[test]
    fn test_sim_batch() {
        let results = run_sim_batch(canon_board(), 10);
        assert!(results.min_rolls > 0); // Must roll once in order to win
        assert!(results.min_lucky_rolls >= 1); // Winning is a lucky roll
    }
}

fn run_sim_batch(board: Board, count: usize) -> MultiSimResult {
    let mut sims: Vec<Sim> = vec![];
    for _ in 0..count {
        let mut sim = Sim::new(board.clone(), Box::new(rand::thread_rng()));
        sim.run();
        //println!("Turns: {}, Rolls: {}", sim.turn_count, sim.roll_count);
        sims.push(sim);
    }
    MultiSimResult::from_sims(&sims)
}

fn main() {
    let (b, max_ites) = load_cfg("config.json").unwrap();
    println!("Loaded board");
    let results = run_sim_batch(b, max_ites);
    println!("{:?}", results);
}
