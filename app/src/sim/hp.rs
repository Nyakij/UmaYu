//! HP (stamina) during the race: drain per second, recovery, and the last-spurt decision
//! (HpPolicy.ts), plus the two variants the Stamina mode uses (hpcalc.ts).

use super::course::{Course, phase_start};
use super::params::{Horse, OONIGE, Strategy};
use super::rng::Rng;

const STRATEGY_COEF: [f64; 6] = [0.0, 0.95, 0.89, 1.0, 0.995, 0.86];
const GROUND_MOD: [[f64; 5]; 2] = [[0.0, 1.0, 1.0, 1.02, 1.02], [0.0, 1.0, 1.0, 1.01, 1.02]];

/// Position keep states (RaceSolver.ts).
pub const PK_NONE: i32 = 0;
pub const PK_PACE_UP: i32 = 1;
pub const PK_PACE_DOWN: i32 = 2;
pub const PK_SPEED_UP: i32 = 3;
pub const PK_OVERTAKE: i32 = 4;

/// The parts of the race state HP drain depends on.
#[derive(Clone, Copy)]
pub struct HpState {
    pub phase: i32,
    pub pos_keep: i32,
    pub rushed: bool,
    pub downhill: bool,
    pub lead_competition: bool,
    pub pos_keep_strategy: Strategy,
    pub speed: f64,
    pub pos: f64,
}

#[derive(Clone)]
pub struct GameHp {
    pub distance: f64,
    base_speed: f64,
    pub max_hp: f64,
    pub hp: f64,
    ground_mod: f64,
    guts_mod: f64,
    subpar_accept: f64,
    rng: Rng,
    pub max_spurt: bool,
}

impl GameHp {
    pub fn new(course: &Course, ground: i32, rng: Rng) -> Self {
        let surface = (course.surface.clamp(1, 2) - 1) as usize;
        GameHp {
            distance: course.distance,
            base_speed: 20.0 - (course.distance - 2000.0) / 1000.0,
            max_hp: 1.0,
            hp: 1.0,
            ground_mod: GROUND_MOD[surface][ground.clamp(0, 4) as usize],
            guts_mod: 1.0,
            subpar_accept: 0.0,
            rng,
            max_spurt: false,
        }
    }

    fn init(&mut self, h: &Horse) {
        self.max_hp =
            0.8 * STRATEGY_COEF[h.strategy.clamp(1, 5) as usize] * h.stamina + self.distance;
        self.hp = self.max_hp;
        self.guts_mod = 1.0 + 200.0 / (600.0 * h.guts).sqrt();
        self.subpar_accept = ((15.0 + 0.05 * h.wisdom) * 1000.0 + 0.5).floor();
        self.max_spurt = false;
    }

    fn status_mod(s: &HpState) -> f64 {
        let mut m = 1.0;
        if s.downhill {
            m *= 0.4;
        }
        let oonige = s.pos_keep_strategy == OONIGE;
        if s.lead_competition {
            m *= match (s.rushed, oonige) {
                (true, true) => 7.7,
                (true, false) => 3.6,
                (false, true) => 3.5,
                (false, false) => 1.4,
            };
        } else if s.rushed {
            m *= 1.6;
        }
        if s.pos_keep == PK_PACE_DOWN {
            m *= 0.6;
        }
        m
    }

    pub fn hp_per_second(&self, s: &HpState, velocity: f64) -> f64 {
        let guts = if s.phase >= 2 { self.guts_mod } else { 1.0 };
        20.0 * (velocity - self.base_speed + 12.0).powi(2) / 144.0
            * Self::status_mod(s)
            * self.ground_mod
            * guts
    }

    fn last_leg(s: &HpState) -> HpState {
        HpState {
            phase: 2,
            pos_keep: PK_NONE,
            rushed: false,
            downhill: false,
            lead_competition: false,
            ..*s
        }
    }

    fn last_spurt_pair(&mut self, s: &HpState, max_speed: f64, bts2: f64) -> (f64, f64) {
        let max_dist = self.distance - phase_start(self.distance, 2);
        let secs = (max_dist - 60.0) / max_speed;
        let leg = Self::last_leg(s);
        if self.hp >= self.hp_per_second(&leg, max_speed) * secs {
            self.max_spurt = true;
            return (-1.0, max_speed);
        }
        let remain = self.distance - 60.0 - s.pos;
        let mut candidates: Vec<(f64, f64)> = Vec::new();
        let mut speed = max_speed - 0.1;
        let base_drain = self.hp_per_second(&leg, bts2);
        while speed >= bts2 {
            let drain = self.hp_per_second(&leg, speed);
            let spurt_secs = (remain / speed).min(
                ((bts2 * self.hp - base_drain * remain) / (bts2 * drain - base_drain * speed))
                    .max(0.0),
            );
            candidates.push((self.distance - spurt_secs * speed - 60.0, speed));
            speed -= 0.1;
        }
        let time = |c: &(f64, f64)| (c.0 - s.pos) / bts2 + (self.distance - c.0) / c.1;
        candidates.sort_by(|a, b| time(a).total_cmp(&time(b)));
        for c in &candidates {
            if self.rng.uniform(100_000.0) <= self.subpar_accept {
                return *c;
            }
        }
        candidates.last().copied().unwrap_or((-1.0, bts2))
    }
}

pub enum Hp {
    /// Pacemakers never run out.
    Noop,
    Game(GameHp),
    /// Stamina mode: optionally forces a full spurt and tracks HP saved on downhills.
    ForceFull {
        hp: GameHp,
        force: bool,
        balance: f64,
        was_full: bool,
        downhill_save: f64,
    },
    /// Stamina mode's second solver: counts HP used up to the start of the last spurt.
    CalcRequired(GameHp),
}

impl Hp {
    pub fn init(&mut self, h: &Horse) {
        match self {
            Hp::Noop => {}
            Hp::Game(g) | Hp::ForceFull { hp: g, .. } => g.init(h),
            Hp::CalcRequired(g) => {
                g.init(h);
                g.hp = 0.0;
            }
        }
    }

    pub fn tick(&mut self, s: &HpState, dt: f64) {
        match self {
            Hp::Noop => {}
            Hp::Game(g) | Hp::CalcRequired(g) => g.hp -= g.hp_per_second(s, s.speed) * dt,
            Hp::ForceFull {
                hp, downhill_save, ..
            } => {
                if s.downhill {
                    let flat = HpState {
                        downhill: false,
                        ..*s
                    };
                    *downhill_save +=
                        hp.hp_per_second(&flat, s.speed) * dt - hp.hp_per_second(s, s.speed) * dt;
                }
                hp.hp -= hp.hp_per_second(s, s.speed) * dt;
            }
        }
    }

    pub fn has_remaining(&self) -> bool {
        match self {
            Hp::Noop | Hp::CalcRequired(_) => true,
            Hp::Game(g) | Hp::ForceFull { hp: g, .. } => g.hp > 0.0,
        }
    }

    pub fn ratio(&self) -> f64 {
        match self {
            Hp::Noop | Hp::CalcRequired(_) => 1.0,
            Hp::Game(g) | Hp::ForceFull { hp: g, .. } => (g.hp / g.max_hp).max(0.0),
        }
    }

    pub fn recover(&mut self, m: f64) {
        match self {
            Hp::Noop => {}
            Hp::Game(g) | Hp::ForceFull { hp: g, .. } | Hp::CalcRequired(g) => {
                g.hp = g.max_hp.min(g.hp + g.max_hp * m)
            }
        }
    }

    /// The HP a duration-scaling skill reads; `None` reads as infinite (best case).
    pub fn current(&self) -> Option<f64> {
        match self {
            Hp::Game(g) => Some(g.hp),
            _ => None,
        }
    }

    /// HP for charts.
    pub fn value(&self) -> f64 {
        match self {
            Hp::Noop => 0.0,
            Hp::Game(g) | Hp::ForceFull { hp: g, .. } | Hp::CalcRequired(g) => g.hp,
        }
    }

    pub fn max_spurt(&self) -> bool {
        matches!(self, Hp::Game(g) if g.max_spurt)
    }

    pub fn last_spurt_pair(&mut self, s: &HpState, max_speed: f64, bts2: f64) -> (f64, f64) {
        match self {
            Hp::Noop => (-1.0, max_speed),
            Hp::Game(g) => g.last_spurt_pair(s, max_speed, bts2),
            Hp::ForceFull {
                hp,
                force,
                balance,
                was_full,
                ..
            } => {
                let max_dist = hp.distance - phase_start(hp.distance, 2);
                let secs = (max_dist - 60.0) / max_speed;
                *balance = hp.hp - hp.hp_per_second(&GameHp::last_leg(s), max_speed) * secs;
                *was_full = *balance >= 0.0;
                if *force {
                    (-1.0, max_speed)
                } else {
                    hp.last_spurt_pair(s, max_speed, bts2)
                }
            }
            Hp::CalcRequired(g) => {
                let max_dist = g.distance - phase_start(g.distance, 2);
                let secs = (max_dist - 60.0) / max_speed;
                g.hp -= g.hp_per_second(&GameHp::last_leg(s), max_speed) * secs;
                (-1.0, max_speed)
            }
        }
    }
}
