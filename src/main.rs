mod dice;

use crate::dice::{Rollable, DIE_SIZE};
use rand::rngs::ThreadRng;
use std::cmp::max;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct Board {
    size: usize,
    routes: HashMap<usize, usize>, // Snakes AND Ladders in Source: Destination order
}

fn get_canon_board() -> Board {
    Board {
        size: 100,
        routes: HashMap::from([
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
    }
}

struct Sim<T: Rollable> {
    board: Board,
    position: usize,
    rng: T,
    // stats
    turn_count: usize,
    roll_count: usize,
    climb_count: usize,
    slide_count: usize,
    climb_distance: usize,
    slide_distance: usize,
    biggest_climb: usize,
    biggest_slide: usize,
}

struct RollResult {
    die_value: usize,
    climb_distance: usize,
    slide_distance: usize,
}

impl<T: Rollable> Sim<T> {
    fn new(board: Board, rng: T) -> Sim<T> {
        Sim {
            board,
            position: 0,
            rng,
            turn_count: 0,
            roll_count: 0,
            climb_count: 0,
            slide_count: 0,
            climb_distance: 0,
            slide_distance: 0,
            biggest_climb: 0,
            biggest_slide: 0,
        }
    }

    fn has_won(&self) -> bool {
        self.position == self.board.size
    }

    fn run(&mut self) {
        // Take turns until has_won()
        // Add a max_turns constraint? Not all possible boards are winnable.
        while !self.has_won() {
            self.turn()
        }
    }

    fn turn(&mut self) {
        // Roll once, and keep rolling if we get DIE_SIZE. Stop immediately if we've won.
        self.turn_count += 1;
        let mut turn_climb = 0;
        let mut turn_slide = 0;
        while !self.has_won() {
            let result = self.roll();
            turn_climb += result.climb_distance;
            turn_slide += result.slide_distance;
            if result.die_value < DIE_SIZE {
                break;
            };
        }
        // Store turn stats
        self.biggest_climb = max(self.biggest_climb, turn_climb);
        self.biggest_slide = max(self.biggest_slide, turn_slide);
    }

    fn roll(&mut self) -> RollResult {
        // Roll the die once and resolve the consequences
        let die_value = self.rng.roll();
        self.roll_resolve(die_value)
    }

    fn roll_resolve(&mut self, die_value: usize) -> RollResult {
        // Try to move forwards some spaces
        self.roll_count += 1;
        // Track roll-wise climb/slide distance separately from lifetime climb/slide distance
        let mut climb_distance = 0;
        let mut slide_distance = 0;
        let mut new_position = self.position + die_value;
        if new_position > self.board.size {
            // Illegal move!
            return RollResult {
                die_value,
                climb_distance,
                slide_distance,
            };
        }

        // Try to follow any routes (snake or ladder)
        // Note, in the version of the game from my childhood, snakes can chain!
        while let Some(p) = self.board.routes.get(&new_position) {
            if *p > new_position {
                // ladder
                let delta = *p - new_position;
                self.climb_count += 1;
                climb_distance += delta;
                self.climb_distance += delta;
            } else {
                // snake
                let delta = new_position - *p;
                self.slide_count += 1;
                slide_distance += delta; // flip sign
                self.slide_distance += delta;
            }
            new_position = *p
        }

        self.position = new_position;
        RollResult {
            die_value,
            climb_distance,
            slide_distance,
        }
    }
}

#[cfg(test)]
mod tests_sim {
    use super::*;
    use crate::dice::{MockDie, Unrollable};

    fn blank_board(size: usize) -> Board {
        Board {
            size,
            routes: HashMap::new(),
        }
    }

    #[test]
    fn test_roll() {
        // Check can move forwards
        let mut sim = Sim::new(blank_board(20), Unrollable {});
        assert_eq!(sim.position, 0);
        sim.roll_resolve(5);
        assert_eq!(sim.position, 5);
        sim.roll_resolve(1);
        assert_eq!(sim.position, 6);
        sim.roll_resolve(9999); // Should choke due to hitting end of board
        assert_eq!(sim.position, 6);
        assert!(!sim.has_won());
        sim.roll_resolve(14); // Perfect roll!
        assert_eq!(sim.position, 20); // End of board
        assert!(sim.has_won()); // Won
        sim.roll_resolve(1);
        assert_eq!(sim.position, 20); // No further moves possible
    }

    #[test]
    fn test_random_roll() {
        // Check can generate a random move
        let max_rolls = 10; // 10 times is good enough
        let board = blank_board(max_rolls * DIE_SIZE); // Make a big enough board
        let mut sim = Sim::new(board.clone(), rand::thread_rng());
        for _ in 0..max_rolls {
            let old_position = sim.position;
            let result = sim.roll();
            println!("Rolled a {}", result.die_value); // Maybe useful for debugging
            assert!(1 <= result.die_value, "{}", result.die_value);
            assert!(result.die_value <= DIE_SIZE, "{}", result.die_value);
            assert_eq!(sim.position, old_position + result.die_value);
        }
    }

    #[test]
    fn test_canon_board_speedrun() {
        // More fun than useful!
        // Can probably be deleted if the canon_board changes
        let b = get_canon_board();
        let rng = MockDie {
            queued_results: vec![2, 6, 5, 1, 2, 6, 4],
        };
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
        assert!(sim.has_won());
    }

    #[test]
    fn test_chained_slides() {
        // Take one step forwards and fall down a chain of snakes
        // then re-roll and go down another snake
        let b = Board {
            size: 100,
            routes: HashMap::from([(99, 60), (60, 30), (30, 2), (5, 1)]),
        };
        let rng = MockDie {
            queued_results: vec![3, 6],
        };
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
        assert!(!sim.has_won());
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct MultiSimResult {
    min_rolls: usize,
    avg_rolls: f64,
    max_rolls: usize,
    min_climbs: usize, // Total distance, events or rolls?
    avg_climbs: f64,
    max_climbs: usize,
    min_slides: usize,
    avg_slides: f64,
    max_slides: usize,
    biggest_turn_climb: usize, // Greatest climb in a single turn, INCLUDING re-rolls and chains
    biggest_turn_slide: usize, // Greatest slide in a single turn, INCLUDING re-rolls and chains
                               /* //todo
                               longest_turn: vec<usize>,
                               min_lucky_rolls: usize,
                               avg_lucky_rolls: f64,
                               max_lucky_rolls: usize,
                               min_unlucky_rolls: usize,
                               avg_unlucky_rolls: f64,
                               max_unlucky_rolls: usize,
                                */
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
    #[test]
    fn test_min_max_average() {
        assert!(min_avg_max(vec![]).is_none());
        assert_eq!(min_avg_max(vec![5]).unwrap(), (5, 5.0, 5));
        assert_eq!(min_avg_max(vec![8, 0, 3]).unwrap(), (0, 11.0 / 3.0, 8));
        assert_eq!(min_avg_max(vec![1, 2, 3]).unwrap(), (1, 2.0, 3));
    }
}

fn run_sim_batch(board: Board, count: usize) -> MultiSimResult {
    let mut sims: Vec<Sim<ThreadRng>> = vec![];
    for _ in 0..count {
        let mut sim = Sim::new(board.clone(), rand::thread_rng());
        sim.run();
        //println!("Turns: {}, Rolls: {}", sim.turn_count, sim.roll_count);
        sims.push(sim);
    }
    let (min_rolls, avg_rolls, max_rolls) =
        min_avg_max(sims.iter().map(|s| s.roll_count).collect()).unwrap();
    let (min_climbs, avg_climbs, max_climbs) =
        min_avg_max(sims.iter().map(|s| s.climb_count).collect()).unwrap();
    let (min_slides, avg_slides, max_slides) =
        min_avg_max(sims.iter().map(|s| s.slide_count).collect()).unwrap();
    MultiSimResult {
        min_rolls,
        avg_rolls,
        max_rolls,
        min_climbs,
        avg_climbs,
        max_climbs,
        min_slides,
        avg_slides,
        max_slides,
        biggest_turn_climb: sims.iter().map(|s| s.biggest_climb).max().unwrap(),
        biggest_turn_slide: sims.iter().map(|s| s.biggest_slide).max().unwrap(),
    }
}

fn main() {
    let b = get_canon_board();
    let results = run_sim_batch(b, 1000);
    print!("{:?}", results)
}
