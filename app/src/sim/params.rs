//! Horse and race parameters (HorseTypes.ts, RaceParameters.ts) and the stat adjustments
//! RaceSolverBuilder applies before a race.

use super::course::Course;

/// Running style codes: 1 Nige (Front), 2 Senkou (Pace), 3 Sasi (Late), 4 Oikomi (End), 5 Oonige (Runaway).
pub type Strategy = i32;
pub const NIGE: Strategy = 1;
pub const SENKOU: Strategy = 2;
pub const SASI: Strategy = 3;
pub const OIKOMI: Strategy = 4;
pub const OONIGE: Strategy = 5;

/// Nige and Oonige count as the same style for skill conditions.
pub fn strategy_matches(a: Strategy, b: Strategy) -> bool {
    a == b || (a == NIGE && b == OONIGE) || (a == OONIGE && b == NIGE)
}

pub fn strategy_name(s: Strategy) -> &'static str {
    match s {
        NIGE => "Front Runner",
        SENKOU => "Pace Chaser",
        SASI => "Late Surger",
        OIKOMI => "End Closer",
        _ => "Runaway",
    }
}

/// Aptitude index: 0 = S, 1 = A … 7 = G.
pub fn parse_aptitude(g: &str) -> usize {
    match g.trim().to_ascii_uppercase().as_str() {
        "S" => 0,
        "A" => 1,
        "B" => 2,
        "C" => 3,
        "D" => 4,
        "E" => 5,
        "F" => 6,
        _ => 7,
    }
}

pub const APTITUDES: [&str; 8] = ["S", "A", "B", "C", "D", "E", "F", "G"];

/// Stats as the race sees them.
#[derive(Clone, Copy, Debug)]
pub struct Horse {
    pub speed: f64,
    pub stamina: f64,
    pub power: f64,
    pub guts: f64,
    pub wisdom: f64,
    pub strategy: Strategy,
    pub distance_apt: usize,
    pub surface_apt: usize,
    pub strategy_apt: usize,
    pub raw_stamina: f64,
    /// Wisdom before the style-aptitude multiplier; used for skill wit checks.
    pub raw_wisdom: f64,
}

/// A build as entered: displayed stats, aptitudes, style and mood (HorseDesc).
#[derive(Clone, Debug, PartialEq)]
pub struct HorseDesc {
    pub speed: f64,
    pub stamina: f64,
    pub power: f64,
    pub guts: f64,
    pub wisdom: f64,
    pub strategy: Strategy,
    pub distance_apt: usize,
    pub surface_apt: usize,
    pub strategy_apt: usize,
    /// -2 (awful) to 2 (great).
    pub mood: i32,
}

fn adjust_overcap(stat: f64) -> f64 {
    if stat > 1200.0 { 1200.0 + ((stat - 1200.0) / 2.0).floor() } else { stat }
}

/// Stats past 1200 count half, then mood scales everything by 2% per step.
pub fn base_stats(h: &HorseDesc) -> Horse {
    let motiv = 1.0 + 0.02 * f64::from(h.mood);
    Horse {
        speed: adjust_overcap(h.speed) * motiv,
        stamina: adjust_overcap(h.stamina) * motiv,
        power: adjust_overcap(h.power) * motiv,
        guts: adjust_overcap(h.guts) * motiv,
        wisdom: adjust_overcap(h.wisdom) * motiv,
        strategy: h.strategy,
        distance_apt: h.distance_apt,
        surface_apt: h.surface_apt,
        strategy_apt: h.strategy_apt,
        raw_stamina: h.stamina * motiv,
        raw_wisdom: adjust_overcap(h.wisdom) * motiv,
    }
}

const GROUND_SPEED: [[f64; 5]; 2] = [[0.0, 0.0, 0.0, 0.0, -50.0], [0.0, 0.0, 0.0, 0.0, -50.0]];
const GROUND_POWER: [[f64; 5]; 2] = [[0.0, 0.0, -50.0, -50.0, -50.0], [0.0, -100.0, -50.0, -100.0, -100.0]];
const STRATEGY_PROFICIENCY: [f64; 8] = [1.1, 1.0, 0.85, 0.75, 0.6, 0.4, 0.2, 0.1];

/// Course stat bonus, ground penalties and the style-aptitude wit multiplier.
pub fn adjusted_stats(b: &Horse, course: &Course, ground: i32) -> Horse {
    let surface = (course.surface.clamp(1, 2) - 1) as usize;
    let g = ground.clamp(0, 4) as usize;
    let modifier = course.speed_modifier([b.speed, b.stamina, b.power, b.guts, b.wisdom]);
    Horse {
        speed: (b.speed * modifier + GROUND_SPEED[surface][g]).max(1.0),
        power: (b.power + GROUND_POWER[surface][g]).max(1.0),
        wisdom: b.wisdom * STRATEGY_PROFICIENCY[b.strategy_apt.min(7)],
        ..*b
    }
}

/// Race conditions. Codes: ground 1 Firm … 4 Heavy; weather 1 Sunny … 4 Snowy; season
/// 1 Spring … 4 Winter (5 Sakura); time 2 Midday, 3 Evening, 4 Night.
#[derive(Clone, Debug, PartialEq)]
pub struct RaceParams {
    pub mood: i32,
    pub ground: i32,
    pub weather: i32,
    pub season: i32,
    pub time: i32,
    pub grade: i32,
    pub popularity: i32,
    pub order_range: Option<(i32, i32)>,
    pub num_umas: Option<i32>,
}

impl Default for RaceParams {
    fn default() -> Self {
        RaceParams { mood: 2, ground: 1, weather: 1, season: 1, time: 2, grade: 100, popularity: 1, order_range: None, num_umas: None }
    }
}

/// The expected field position for a style, used by the skill chart for order conditions.
pub fn order_range_for(strategy: Strategy) -> (i32, i32) {
    match strategy {
        NIGE | OONIGE => (1, 1),
        SENKOU => (2, 4),
        _ => (5, 9),
    }
}
