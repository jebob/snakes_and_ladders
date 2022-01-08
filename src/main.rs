use rand::rngs::ThreadRng;
use rand::Rng;
use std::collections::HashMap;

const DIE_SIZE: usize = 6; // Must be >= 1

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

struct Sim {
    board: Board,
    position: usize,
    rng: ThreadRng,
}

struct RollResult {
    die_value: usize,
}

impl Sim {
    fn new(board: Board) -> Sim {
        Sim {
            board,
            position: 0,
            rng: rand::thread_rng(),
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
        while !self.has_won() {
            let result = self.roll();
            if result.die_value < DIE_SIZE {
                break;
            };
        }
    }

    fn roll(&mut self) -> RollResult {
        // Roll the die once and resolve the consequences
        let die_value = self.rng.gen_range(1, DIE_SIZE);
        self.roll_resolve(die_value)
    }

    fn roll_resolve(&mut self, die_value: usize) -> RollResult {
        // Try to move forwards some spaces
        let mut new_position = self.position + die_value;
        if new_position > self.board.size {
            // Illegal move!
            return RollResult { die_value };
        }

        // Try to follow any routes (snake or ladder)
        // Note, in the version of the game from my childhood, snakes can chain!
        while let Some(p) = self.board.routes.get(&new_position) {
            new_position = *p
        }

        self.position = new_position;
        RollResult { die_value }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::max;

    fn blank_board(size: usize) -> Board {
        Board {
            size,
            routes: HashMap::new(),
        }
    }

    #[test]
    fn test_roll() {
        // Check can move forwards
        let mut sim = Sim::new(blank_board(20));
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
        // Make a big enough board
        let max_rolls = 10; // 10 times is good enough
        let board = blank_board(max_rolls * DIE_SIZE);
        let mut sim = Sim::new(board.clone());
        for _ in 0..max_rolls {
            let old_position = sim.position;
            let result = sim.roll();
            println!("Rolled a {}", result.die_value); // Maybe useful for debugging
            assert!(1 <= result.die_value, "{}", result.die_value);
            assert!(result.die_value <= DIE_SIZE, "{}", result.die_value);
            assert_eq!(sim.position, old_position + result.die_value);
        }
    }
}

fn main() {
    let b = get_canon_board();
    let mut sim = Sim::new(b);
    sim.run();
    println!("Hello, world!");
}
