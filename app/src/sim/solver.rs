//! The race itself: one uma's position, speed and HP integrated frame by frame at 15 fps, with
//! skills, position keep, rushing, downhill mode, lane movement, dueling and spot struggle
//! (RaceSolver.ts). Several solvers share a `[Solver]` slice so they can see each other.

use std::collections::HashSet;
use std::sync::Arc;

use super::conditions::{Dyn, RaceView};
use super::course::{Course, base_speed, phase_start};
use super::hp::{Hp, HpState, PK_NONE, PK_OVERTAKE, PK_PACE_DOWN, PK_PACE_UP, PK_SPEED_UP};
use super::params::{Horse, NIGE, OIKOMI, OONIGE, SASI, SENKOU, Strategy, strategy_matches};
use super::region::Region;
use super::rng::Rng;

/// Pseudo skill ids for the conserve-power and stamina-duel effects the builder adds.
pub const ASITAME: u32 = 1;
pub const STAMINA_SYOUBU: u32 = 2;

// Skill effect types (SkillType in RaceSolver.ts).
pub const NOOP: i32 = 0;
pub const SPEED_UP: i32 = 1;
pub const STAMINA_UP: i32 = 2;
pub const POWER_UP: i32 = 3;
pub const GUTS_UP: i32 = 4;
pub const WISDOM_UP: i32 = 5;
pub const RECOVERY: i32 = 9;
pub const MULTIPLY_START_DELAY: i32 = 10;
pub const SET_START_DELAY: i32 = 14;
pub const CURRENT_SPEED: i32 = 21;
pub const CURRENT_SPEED_DECEL: i32 = 22;
pub const TARGET_SPEED: i32 = 27;
pub const LANE_MOVEMENT_SPEED: i32 = 28;
pub const ACCEL: i32 = 31;
pub const CHANGE_LANE: i32 = 35;
pub const ACTIVATE_RANDOM_GOLD: i32 = 37;
pub const EXTEND_EVOLVED_DURATION: i32 = 42;

pub const KNOWN_EFFECTS: [i32; 17] = [
    NOOP,
    SPEED_UP,
    STAMINA_UP,
    POWER_UP,
    GUTS_UP,
    WISDOM_UP,
    RECOVERY,
    MULTIPLY_START_DELAY,
    SET_START_DELAY,
    CURRENT_SPEED,
    CURRENT_SPEED_DECEL,
    TARGET_SPEED,
    LANE_MOVEMENT_SPEED,
    ACCEL,
    CHANGE_LANE,
    ACTIVATE_RANDOM_GOLD,
    EXTEND_EVOLVED_DURATION,
];

// Skill rarities as the solver sees them (white, gold, unique, evolved).
pub const RARITY_GOLD: i32 = 2;
pub const RARITY_UNIQUE: i32 = 3;
pub const RARITY_EVOLUTION: i32 = 6;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Perspective {
    /// The uma's own skill.
    Mine,
    /// Another uma's skill as it affects this one (debuffs; everything else is a no-op that
    /// still uses up wit-check rolls so two compared umas stay in sync).
    Other,
}

#[derive(Clone, Debug)]
pub struct Effect {
    pub kind: i32,
    pub base_duration: f64,
    pub modifier: f64,
    pub duration_scaling: i32,
    pub value_scaling: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct PendingSkill {
    pub id: u32,
    pub perspective: Perspective,
    pub rarity: i32,
    pub trigger: Region,
    pub cond: Arc<Dyn>,
    pub effects: Arc<Vec<Effect>>,
    pub origin_wisdom: Option<f64>,
}

#[derive(Clone, Copy)]
struct Active {
    id: u32,
    timer: usize,
    modifier: f64,
}

/// One activation of one of the uma's own skills.
#[derive(Clone, Copy, Debug)]
pub struct Activation {
    pub id: u32,
    pub start: f64,
    /// -1 for skills without a duration.
    pub end: f64,
    pub time: f64,
    pub duration: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PosKeepMode {
    None,
    Approximate,
}

/// Dueling chance per style, in percent: runaway, front, pace, late, end.
pub type DuelingRates = [f64; 5];

pub struct SolverParams {
    pub horse: Horse,
    pub course: Arc<Course>,
    pub rng: Rng,
    pub skills: Vec<PendingSkill>,
    pub hp: Hp,
    pub pos_keep_mode: PosKeepMode,
    /// Compare mode keeps position keep on for 10 sections; chart mode for 3.
    pub compare_mode: bool,
    pub is_pacer: bool,
    pub wit_check: bool,
    pub rushed: bool,
    pub compete_fight: bool,
    pub lead_competition: bool,
    pub dueling: Option<DuelingRates>,
    pub lane_movement: bool,
}

#[derive(Clone, Copy, Default)]
struct Compensated {
    acc: f64,
    err: f64,
}

impl Compensated {
    fn add(&mut self, n: f64) {
        let t = self.acc + n;
        if self.acc.abs() >= n.abs() {
            self.err += (self.acc - t) + n;
        } else {
            self.err += (n - t) + self.acc;
        }
        self.acc = t;
    }
    fn value(&self) -> f64 {
        self.acc + self.err
    }
}

const STRATEGY_PHASE_SPEED: [[f64; 3]; 6] = [
    [0.0; 3],
    [1.0, 0.98, 0.962],
    [0.978, 0.991, 0.975],
    [0.938, 0.998, 0.994],
    [0.931, 1.0, 1.0],
    [1.063, 0.962, 0.95],
];
const DISTANCE_SPEED: [f64; 8] = [1.05, 1.0, 0.9, 0.8, 0.6, 0.4, 0.2, 0.1];
const STRATEGY_PHASE_ACCEL: [[f64; 3]; 6] = [
    [0.0; 3],
    [1.0, 1.0, 0.996],
    [0.985, 1.0, 0.996],
    [0.975, 1.0, 1.0],
    [0.945, 1.0, 0.997],
    [1.17, 0.94, 0.956],
];
const SURFACE_ACCEL: [f64; 8] = [1.05, 1.0, 0.9, 0.8, 0.7, 0.5, 0.3, 0.1];
const DISTANCE_ACCEL: [f64; 8] = [1.0, 1.0, 1.0, 1.0, 1.0, 0.6, 0.5, 0.4];
const PHASE_DECEL: [f64; 3] = [-1.2, -0.8, -1.0];
const PK_MIN: [f64; 6] = [0.0, 0.0, 3.0, 6.5, 7.5, 0.0];
const PK_MAX: [f64; 6] = [0.0, 0.0, 5.0, 7.0, 8.0, 0.0];

fn base_target_speed(h: &Horse, c: &Course, phase: usize) -> f64 {
    base_speed(c.distance) * STRATEGY_PHASE_SPEED[h.strategy.clamp(1, 5) as usize][phase]
        + if phase == 2 {
            (500.0 * h.speed).sqrt() * DISTANCE_SPEED[h.distance_apt.min(7)] * 0.002
        } else {
            0.0
        }
}

fn last_spurt_speed(h: &Horse, c: &Course) -> f64 {
    let v = (base_target_speed(h, c, 2) + 0.01 * base_speed(c.distance)) * 1.05
        + (500.0 * h.speed).sqrt() * DISTANCE_SPEED[h.distance_apt.min(7)] * 0.002;
    v + (450.0 * h.guts).powf(0.597) * 0.0001
}

/// How an aptitude grade (index 0 = S … 7 = G) holds a build back, by the simulator's
/// coefficients: the share of last-spurt speed lost to a distance grade below A on a race of
/// `distance` metres, the share of acceleration lost to a surface grade, and the share of Wit a
/// style grade takes away. Each is 0 at A.
pub fn aptitude_losses(
    desc: &super::params::HorseDesc,
    distance: f64,
    distance_apt: usize,
    surface_apt: usize,
    style_apt: usize,
) -> [f64; 3] {
    let spurt = |apt: usize| {
        let mut d = desc.clone();
        d.distance_apt = apt;
        let h = super::params::base_stats(&d);
        // As last_spurt_speed: the speed term is in the phase's target speed and added again.
        let from_speed = (500.0 * h.speed).sqrt() * DISTANCE_SPEED[apt.min(7)] * 0.002;
        let v = (base_speed(distance) * STRATEGY_PHASE_SPEED[h.strategy.clamp(1, 5) as usize][2]
            + from_speed
            + 0.01 * base_speed(distance))
            * 1.05;
        v + from_speed + (450.0 * h.guts).powf(0.597) * 0.0001
    };
    let a = 1usize;
    [
        (1.0 - spurt(distance_apt) / spurt(a)).max(0.0),
        (1.0 - SURFACE_ACCEL[surface_apt.min(7)] / SURFACE_ACCEL[a]).max(0.0),
        (1.0 - super::params::strategy_proficiency(style_apt)
            / super::params::strategy_proficiency(a))
        .max(0.0),
    ]
}

fn base_accel(base: f64, h: &Horse, phase: usize) -> f64 {
    base * (500.0 * h.power).sqrt()
        * STRATEGY_PHASE_ACCEL[h.strategy.clamp(1, 5) as usize][phase]
        * SURFACE_ACCEL[h.surface_apt.min(7)]
        * DISTANCE_ACCEL[h.distance_apt.min(7)]
}

// Fixed timer slots; skill timers are appended after these.
const T_CONDITION: usize = 0;
const T_ACC: usize = 1;
const T_PK_NEXT: usize = 2;
const T_RUSHED: usize = 3;
const T_COMPETE: usize = 4;
const T_LEAD: usize = 5;
const T_DOWNHILL: usize = 6;

pub struct Solver {
    pub horse: Horse,
    pub course: Arc<Course>,
    pub hp: Hp,
    rng: Rng,
    sync_rng: Rng,
    gorosi_rng: Rng,
    rushed_rng: Rng,
    wisdom_rng: Rng,
    pos_keep_rng: Rng,
    lane_rng: Rng,
    special_rng: Rng,
    compete_rng: Rng,
    downhill_rng: Vec<Rng>,
    timers: Vec<f64>,

    pub pos: f64,
    pub current_speed: f64,
    pub target_speed: f64,
    pub accel: f64,
    min_speed: f64,
    base_target: [f64; 3],
    pub last_spurt_speed: f64,
    pub last_spurt_transition: f64,
    section_modifier: Vec<f64>,
    base_accel: [f64; 6],
    start_dash: bool,
    pub start_delay: f64,
    start_delay_acc: f64,
    pub gate_roll: f64,
    pub random_lot: f64,
    pub is_last_spurt: bool,
    pub phase: i32,
    next_phase: f64,

    active_target: Vec<Active>,
    active_current: Vec<Active>,
    active_accel: Vec<Active>,
    active_lane: Vec<Active>,
    active_change_lane: Vec<Active>,
    pub pending: Vec<PendingSkill>,
    pending_removal: HashSet<u32>,
    pub used: HashSet<u32>,
    n_hills: usize,
    hill_idx: i64,
    slope_per: f64,
    hill_start: Vec<f64>,
    hill_end: Vec<f64>,
    pub activate_count: [f64; 3],
    pub activate_count_heal: f64,
    pub activate_count_later_half: f64,
    section_length: f64,

    /// Indices into the shared solver slice: the other umas, then this one.
    pub umas: Vec<usize>,
    pub pacer: Option<usize>,
    pub is_pacer: bool,
    pub pacer_override: bool,
    pk_min: f64,
    pk_max: f64,
    pk_exit_position: f64,
    pk_exit_distance: f64,
    pk_end: f64,
    pub position_keep: i32,
    pk_mode: PosKeepMode,
    pk_speed_coef: f64,
    pub pos_keep_strategy: Strategy,
    wit_check: bool,

    pub is_rushed: bool,
    has_been_rushed: bool,
    rushed_section: i64,
    rushed_enter: f64,
    rushed_max: f64,
    /// (start, end) of the rushed stretch; end is -1 while it lasts.
    pub rushed_span: Option<(f64, f64)>,

    pub is_downhill: bool,

    can_compete: Option<bool>,
    pub compete_fight: bool,
    pub compete_start: Option<f64>,
    pub compete_end: Option<f64>,
    compete_enabled: bool,
    dueling: Option<DuelingRates>,

    lead_enabled: bool,
    pub lead_competition: bool,
    pub lead_start: Option<f64>,
    pub lead_end: Option<f64>,

    lane_enabled: bool,
    pub current_lane: f64,
    target_lane: f64,
    lane_change_speed: f64,
    extra_move_lane: f64,

    pub first_in_late_race: bool,
    pub hp_died: bool,
    pub hp_died_position: Option<f64>,
    pub full_spurt: bool,
    pub non_full_velocity_diff: Option<f64>,
    pub non_full_delay: Option<f64>,

    mod_target: Compensated,
    mod_current: Compensated,
    mod_accel: Compensated,
    special_duration_scaling: f64,

    cond_blocked_side: i32,
    cond_overtake: i32,

    /// Own skill activations, for the results (when `record` is on).
    pub record: bool,
    pub activations: Vec<Activation>,
}

impl RaceView for Solver {
    fn accumulate_time(&self) -> f64 {
        self.timers[T_ACC]
    }
    fn activate_count(&self) -> [f64; 3] {
        self.activate_count
    }
    fn activate_count_heal(&self) -> f64 {
        self.activate_count_heal
    }
    fn activate_count_later_half(&self) -> f64 {
        self.activate_count_later_half
    }
    fn hp_ratio(&self) -> f64 {
        self.hp.ratio()
    }
    fn has_hp(&self) -> bool {
        self.hp.has_remaining()
    }
    fn used(&self, id: u32) -> bool {
        self.used.contains(&id)
    }
    fn any_used(&self) -> bool {
        !self.used.is_empty()
    }
    fn start_delay(&self) -> f64 {
        self.start_delay
    }
    fn is_last_spurt(&self) -> bool {
        self.is_last_spurt
    }
    fn last_spurt_transition(&self) -> f64 {
        self.last_spurt_transition
    }
    fn gate_roll(&self) -> f64 {
        self.gate_roll
    }
    fn random_lot(&self) -> f64 {
        self.random_lot
    }
}

impl Solver {
    pub fn new(p: SolverParams) -> Solver {
        let mut rng = p.rng;
        let course = p.course;
        let mut sub = || Rng::new(rng.int32());
        let (
            sync_rng,
            gorosi_rng,
            rushed_rng,
            wisdom_rng,
            pos_keep_rng,
            lane_rng,
            special_rng,
            compete_rng,
        ) = (sub(), sub(), sub(), sub(), sub(), sub(), sub(), sub());
        let gate_roll = rng.uniform(12_252_240.0);
        let random_lot = rng.uniform(100.0);
        let d = course.distance;
        let section_length = d / 24.0;
        let strategy = p.horse.strategy;
        let course_factor = 0.0008 * (d - 1000.0) + 1.0;
        let s_idx = strategy.clamp(0, 5) as usize;
        let gate_raw = (gate_roll as i64) % 9;
        let gate = if gate_raw < 9 {
            gate_raw
        } else {
            1 + (24 - gate_raw) % 8
        };
        let lane = gate as f64 * course.horse_lane;
        let mut s = Solver {
            horse: p.horse,
            hp: p.hp,
            sync_rng,
            gorosi_rng,
            rushed_rng,
            wisdom_rng,
            pos_keep_rng,
            lane_rng,
            special_rng,
            compete_rng,
            downhill_rng: Vec::new(),
            timers: vec![-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            pos: 0.0,
            current_speed: 3.0,
            target_speed: 0.85 * base_speed(d),
            accel: 0.0,
            min_speed: 0.0,
            base_target: [0.0; 3],
            last_spurt_speed: 0.0,
            last_spurt_transition: -1.0,
            section_modifier: Vec::new(),
            base_accel: [0.0; 6],
            start_dash: true,
            start_delay: 0.0,
            start_delay_acc: 0.0,
            gate_roll,
            random_lot,
            is_last_spurt: false,
            phase: 0,
            next_phase: phase_start(d, 1),
            active_target: Vec::new(),
            active_current: Vec::new(),
            active_accel: Vec::new(),
            active_lane: Vec::new(),
            active_change_lane: Vec::new(),
            pending: p.skills,
            pending_removal: HashSet::new(),
            used: HashSet::new(),
            n_hills: 0,
            hill_idx: -1,
            slope_per: 0.0,
            hill_start: Vec::new(),
            hill_end: Vec::new(),
            activate_count: [0.0; 3],
            activate_count_heal: 0.0,
            activate_count_later_half: 0.0,
            section_length,
            umas: Vec::new(),
            pacer: None,
            is_pacer: p.is_pacer,
            pacer_override: false,
            pk_min: PK_MIN[s_idx]
                * if strategy == SENKOU {
                    1.0
                } else {
                    course_factor
                },
            pk_max: PK_MAX[s_idx] * course_factor,
            pk_exit_position: 0.0,
            pk_exit_distance: 0.0,
            pk_end: section_length * if p.compare_mode { 10.0 } else { 3.0 },
            position_keep: PK_NONE,
            pk_mode: p.pos_keep_mode,
            pk_speed_coef: 1.0,
            pos_keep_strategy: strategy,
            wit_check: p.wit_check,
            is_rushed: false,
            has_been_rushed: false,
            rushed_section: -1,
            rushed_enter: -1.0,
            rushed_max: 12.0,
            rushed_span: None,
            is_downhill: false,
            can_compete: None,
            compete_fight: false,
            compete_start: None,
            compete_end: None,
            compete_enabled: p.compete_fight,
            dueling: p.dueling,
            lead_enabled: p.lead_competition,
            lead_competition: false,
            lead_start: None,
            lead_end: None,
            lane_enabled: p.lane_movement,
            current_lane: lane,
            target_lane: lane,
            lane_change_speed: 0.0,
            extra_move_lane: -1.0,
            first_in_late_race: false,
            hp_died: false,
            hp_died_position: None,
            full_spurt: false,
            non_full_velocity_diff: None,
            non_full_delay: None,
            mod_target: Compensated::default(),
            mod_current: Compensated::default(),
            mod_accel: Compensated::default(),
            special_duration_scaling: 1.0,
            cond_blocked_side: 1,
            cond_overtake: 0,
            record: false,
            activations: Vec::new(),
            course: course.clone(),
            rng: Rng::new(1.0),
        };
        if p.rushed {
            s.init_rushed();
        }
        s.start_delay = 0.1 * rng.random();
        s.rng = rng;
        // Gate skills activate before the minimum speed is set, since greens can change guts.
        s.process_skills();
        s.min_speed = 0.85 * base_speed(d) + (200.0 * s.horse.guts).sqrt() * 0.001;
        s.mod_accel.add(24.0); // start dash
        s.init_hills();
        s.start_delay_acc = s.start_delay;
        s.base_target = [0, 1, 2].map(|ph| base_target_speed(&s.horse, &course, ph));
        s.last_spurt_speed = last_spurt_speed(&s.horse, &course);
        let w = s.horse.wisdom;
        let bs = base_speed(d);
        s.section_modifier = (0..24)
            .map(|_| {
                let max = w / 5500.0 * (w * 0.1).log10();
                bs * ((max - 0.65 + s.wisdom_rng.random() * 0.65) / 100.0)
            })
            .collect();
        s.section_modifier.push(0.0);
        let horse = s.horse;
        s.hp.init(&horse);
        for (i, ph) in [0usize, 1, 2, 0, 1, 2].iter().enumerate() {
            s.base_accel[i] = base_accel(if i > 2 { 0.0004 } else { 0.0006 }, &horse, *ph);
        }
        s
    }

    fn new_timer(&mut self, t: f64) -> usize {
        self.timers.push(t);
        self.timers.len() - 1
    }

    pub fn time(&self) -> f64 {
        self.timers[T_ACC]
    }

    /// Speed including current-speed skill boosts, as shown on charts.
    pub fn displayed_speed(&self) -> f64 {
        self.current_speed + self.mod_current.value()
    }

    fn init_hills(&mut self) {
        let slopes = self.course.slopes.clone();
        self.n_hills = slopes.len();
        self.hill_start = slopes.iter().map(|s| s.start).rev().collect();
        self.hill_end = slopes.iter().map(|s| s.start + s.length).rev().collect();
        self.hill_idx = -1;
        self.downhill_rng = slopes.iter().map(|_| Rng::new(self.rng.int32())).collect();
        if self.hill_start.last() == Some(&0.0) {
            self.hill_idx = 0;
            self.slope_per = slopes[0].slope;
            self.timers[T_DOWNHILL] = 0.0;
            let roll = self.downhill_rng[0].random();
            self.downhill_check(roll);
            self.hill_start.pop();
        } else {
            self.slope_per = 0.0;
        }
    }

    fn init_rushed(&mut self) {
        let chance = (6.5 / (0.1 * self.horse.wisdom + 1.0).log10()).powi(2) / 100.0;
        // Self-Control (202161) lowers the chance by a flat 3%.
        let self_control = self.pending.iter().any(|s| s.id == 202_161);
        let chance = (chance - if self_control { 0.03 } else { 0.0 }).max(0.0);
        if self.rushed_rng.random() < chance {
            self.rushed_section = 2 + self.rushed_rng.uniform(8.0) as i64;
            self.rushed_enter = self.section_length * self.rushed_section as f64;
        }
    }

    fn update_rushed(&mut self) {
        if self.rushed_section >= 0
            && !self.is_rushed
            && !self.has_been_rushed
            && self.pos >= self.rushed_enter
        {
            self.is_rushed = true;
            self.has_been_rushed = true;
            self.timers[T_RUSHED] = 0.0;
            self.rushed_span = Some((self.pos, -1.0));
        }
        if self.is_rushed {
            let t = self.timers[T_RUSHED];
            // A 55% chance to calm down every 3 seconds, and 12 s at most.
            if t > 0.0
                && (t / 3.0).floor() > ((t - 0.017) / 3.0).floor()
                && self.rushed_rng.random() < 0.55
            {
                self.end_rushed();
                return;
            }
            if t >= self.rushed_max {
                self.end_rushed();
            }
        }
    }

    fn end_rushed(&mut self) {
        self.is_rushed = false;
        if let Some((s, -1.0)) = self.rushed_span {
            self.rushed_span = Some((s, self.pos));
        }
    }

    fn max_start_dash_speed(&self) -> f64 {
        self.target_speed
            .min(0.85 * base_speed(self.course.distance))
    }

    fn tick_conditions(&mut self) {
        // blocked_side: outer lane in sections 1–3 never blocks; otherwise phase-based odds.
        let section = (self.pos / self.section_length).floor();
        let (start, cont) =
            if (1.0..=3.0).contains(&section) && self.current_lane > 3.0 * self.course.horse_lane {
                (0.0, 0.0)
            } else if self.phase == 0 {
                (0.1, 0.85)
            } else if self.phase == 1 {
                (0.08, 0.75)
            } else {
                (0.07, 0.50)
            };
        let r = self.special_rng.random();
        self.cond_blocked_side = i32::from(
            r < if self.cond_blocked_side == 0 {
                start
            } else {
                cont
            },
        );
        let (start, cont) = match self.horse.strategy {
            NIGE => (0.05, 0.50),
            SENKOU => (0.15, 0.55),
            _ => (0.20, 0.60),
        };
        let r = self.special_rng.random();
        self.cond_overtake = i32::from(r < if self.cond_overtake == 0 { start } else { cont });
    }

    fn hp_state(&self) -> HpState {
        HpState {
            phase: self.phase,
            pos_keep: self.position_keep,
            rushed: self.is_rushed,
            downhill: self.is_downhill,
            lead_competition: self.lead_competition,
            pos_keep_strategy: self.pos_keep_strategy,
            speed: self.current_speed,
            pos: self.pos,
        }
    }

    fn apply_lane_movement(&mut self) {
        let lane = self.current_lane;
        let side_blocked = self.cond_blocked_side == 1;
        let overtake = self.cond_overtake == 1;
        let c = self.course.clone();
        if self.extra_move_lane < 0.0 && self.after_final_corner_or_in_final_straight() {
            self.extra_move_lane =
                (lane / 0.1).min(c.max_lane_distance) * 0.5 + self.lane_rng.random() * 0.1;
        }
        self.target_lane = if !self.active_change_lane.is_empty() {
            9.5 * c.horse_lane
        } else if overtake {
            self.target_lane.max(c.horse_lane).max(self.extra_move_lane)
        } else if !self.hp.has_remaining() {
            lane
        } else if self.position_keep == PK_PACE_DOWN {
            0.18
        } else if self.extra_move_lane > lane {
            self.extra_move_lane
        } else if self.phase <= 1 && !side_blocked {
            (lane - 0.05).max(0.0)
        } else {
            lane
        };
        if (side_blocked && self.target_lane < lane) || (self.target_lane - lane).abs() < 0.00001 {
            self.lane_change_speed = 0.0;
        } else {
            let mut target = 0.02 * (0.3 + 0.001 * self.horse.power);
            if self.pos < c.move_lane_point {
                target *= 1.0 + lane / c.max_lane_distance * 0.05;
            }
            self.lane_change_speed =
                (self.lane_change_speed + c.lane_change_acceleration_per_frame).min(target);
            let skill: f64 = self.active_lane.iter().map(|s| s.modifier).sum();
            let actual = (self.lane_change_speed + skill).min(0.6);
            if self.target_lane > lane {
                self.current_lane = self.target_lane.min(lane + actual);
            } else {
                self.current_lane = self.target_lane.max(lane - actual * (1.0 + lane));
            }
        }
    }

    fn on_final_straight(&self) -> bool {
        self.course
            .straights
            .last()
            .is_some_and(|s| self.pos >= s.start && self.pos <= s.end)
    }

    fn after_final_corner_or_in_final_straight(&self) -> bool {
        self.course
            .corners
            .last()
            .is_some_and(|c| self.pos >= c.start)
            || self.on_final_straight()
    }

    fn speed_up_wit_check(&mut self) -> bool {
        self.pos_keep_rng.random() < 0.2 * (0.1 * self.horse.wisdom).log10()
    }

    fn pace_up_wit_check(&mut self) -> bool {
        self.pos_keep_rng.random() < 0.15 * (0.1 * self.horse.wisdom).log10()
    }

    /// Sort `umas` by position, farthest first (stable, like Array.sort).
    fn sort_umas(&mut self, pos: &[(f64, Strategy)]) {
        self.umas.sort_by(|a, b| pos[*b].0.total_cmp(&pos[*a].0));
    }

    fn end_pos_keep(&mut self) {
        self.position_keep = PK_NONE;
        self.timers[T_PK_NEXT] = -3.0;
    }

    fn apply_position_keep(&mut self, me: usize, snap: &[(f64, Strategy)]) {
        if self.pos >= self.pk_end || self.pk_mode == PosKeepMode::None {
            self.position_keep = PK_NONE;
            return;
        }
        let Some(pacer) = self.pacer else { return };
        let behind = snap[pacer].0 - self.pos;
        let strategy = self.pos_keep_strategy;
        let second_gap = |s: &mut Solver, lead_pos: f64| -> f64 {
            s.sort_umas(snap);
            let second = s.umas.get(1).copied().unwrap_or(me);
            lead_pos - snap[second].0
        };
        match self.position_keep {
            PK_NONE => {
                if self.timers[T_PK_NEXT] < 0.0 {
                    return;
                }
                if strategy_matches(strategy, NIGE) {
                    if pacer == me {
                        let ahead = second_gap(self, snap[pacer].0);
                        let threshold = if strategy == OONIGE { 17.5 } else { 4.5 };
                        if ahead < threshold && self.speed_up_wit_check() {
                            self.position_keep = PK_SPEED_UP;
                            self.pk_exit_position = self.pos + self.section_length.floor();
                        }
                    } else if self.speed_up_wit_check() {
                        self.position_keep = PK_OVERTAKE;
                    }
                } else if behind > self.pk_max {
                    if self.pace_up_wit_check() {
                        self.position_keep = PK_PACE_UP;
                        self.pk_exit_distance =
                            self.pos_keep_rng.random() * (self.pk_max - self.pk_min) + self.pk_min;
                    }
                } else if behind < self.pk_min
                    && self.active_target.is_empty()
                    && self.active_current.is_empty()
                {
                    self.position_keep = PK_PACE_DOWN;
                    self.pk_exit_distance =
                        self.pos_keep_rng.random() * (self.pk_max - self.pk_min) + self.pk_min;
                }
                if self.position_keep == PK_NONE {
                    self.timers[T_PK_NEXT] = -2.0;
                } else {
                    self.pk_exit_position = self.pos + self.section_length.floor();
                }
            }
            PK_SPEED_UP => {
                if self.pos >= self.pk_exit_position {
                    self.end_pos_keep();
                } else if pacer == me {
                    let ahead = second_gap(self, snap[pacer].0);
                    if ahead >= if strategy == OONIGE { 17.5 } else { 4.5 } {
                        self.end_pos_keep();
                    }
                }
            }
            PK_OVERTAKE => {
                if self.pos >= self.pk_exit_position {
                    self.end_pos_keep();
                } else if pacer == me {
                    let ahead = second_gap(self, self.pos);
                    if ahead >= if strategy == OONIGE { 27.5 } else { 10.0 } {
                        self.end_pos_keep();
                    }
                }
            }
            PK_PACE_UP => {
                if self.pos >= self.pk_exit_position || behind < self.pk_exit_distance {
                    self.end_pos_keep();
                }
            }
            PK_PACE_DOWN => {
                if self.pos >= self.pk_exit_position
                    || behind > self.pk_exit_distance
                    || !self.active_target.is_empty()
                    || !self.active_current.is_empty()
                {
                    self.end_pos_keep();
                }
            }
            _ => {}
        }
    }

    fn update_pos_keep_coef(&mut self) {
        self.pk_speed_coef = match self.position_keep {
            PK_SPEED_UP | PK_PACE_UP => 1.04,
            PK_OVERTAKE => 1.05,
            PK_PACE_DOWN => 0.915,
            _ => 1.0,
        };
    }

    fn update_compete_fight(&mut self) {
        if !self.compete_enabled {
            return;
        }
        if self.compete_fight {
            if self.hp.ratio() <= 0.05 {
                self.compete_fight = false;
                self.compete_end = Some(self.pos);
            }
            return;
        }
        if self.hp.ratio() < 0.15 || !self.on_final_straight() {
            return;
        }
        if self.can_compete.is_none() {
            match self.dueling {
                Some(r) => {
                    let rate = match self.pos_keep_strategy {
                        OONIGE => r[0],
                        NIGE => r[1],
                        SENKOU => r[2],
                        SASI => r[3],
                        OIKOMI => r[4],
                        _ => 0.0,
                    };
                    self.can_compete = Some(self.compete_rng.random() < rate / 100.0);
                    self.timers[T_COMPETE] = 0.0;
                }
                None => self.can_compete = Some(false),
            }
        }
        if self.can_compete != Some(true) {
            return;
        }
        // A 40% chance each second to start dueling.
        if self.timers[T_COMPETE] >= 1.0 {
            if self.compete_rng.random() <= 0.4 {
                self.compete_fight = true;
                self.compete_start = Some(self.pos);
            } else {
                self.timers[T_COMPETE] = 0.0;
            }
        }
    }

    /// Spot struggle. Returns the other umas that start one with this uma.
    fn update_lead_competition(&mut self, me: usize, snap: &[(f64, Strategy)]) -> Vec<usize> {
        if !self.lead_enabled {
            return Vec::new();
        }
        if self.lead_competition {
            let duration = (700.0 * self.horse.guts).sqrt() * 0.012;
            if self.timers[T_LEAD] >= duration || self.pos >= self.lead_end.unwrap_or(f64::INFINITY)
            {
                self.lead_competition = false;
                self.lead_end = Some(self.pos);
            }
        }
        if self.lead_start.is_some() {
            return Vec::new();
        }
        if self.pos >= 150.0
            && self.pos <= (self.section_length * 5.0).floor()
            && strategy_matches(self.pos_keep_strategy, NIGE)
        {
            let gap = if self.pos_keep_strategy == NIGE {
                3.75
            } else {
                5.0
            };
            let within: Vec<usize> = self
                .umas
                .iter()
                .copied()
                .filter(|&u| {
                    snap[u].1 == self.pos_keep_strategy && (snap[u].0 - self.pos).abs() <= gap
                })
                .collect();
            if within.len() >= 2 {
                if within.contains(&me) {
                    self.start_lead_competition(self.section_length);
                }
                return within.into_iter().filter(|u| *u != me).collect();
            }
        }
        Vec::new()
    }

    pub fn start_lead_competition(&mut self, section_length: f64) {
        self.timers[T_LEAD] = 0.0;
        self.lead_competition = true;
        self.lead_start = Some(self.pos);
        self.lead_end = Some(self.pos + (section_length * 8.0).floor());
    }

    fn update_last_spurt(&mut self, force: bool) {
        if self.is_last_spurt || self.phase < 2 {
            return;
        }
        if self.last_spurt_transition == -1.0 || force {
            let initial = self.last_spurt_speed;
            let st = self.hp_state();
            let (transition, speed) =
                self.hp
                    .last_spurt_pair(&st, self.last_spurt_speed, self.base_target[2]);
            self.last_spurt_transition = transition;
            self.last_spurt_speed = speed;
            if self.hp.max_spurt() {
                self.full_spurt = true;
            } else {
                self.non_full_velocity_diff = Some(self.last_spurt_speed - initial);
                self.non_full_delay =
                    (transition >= 0.0).then(|| transition - self.course.distance * 2.0 / 3.0);
            }
        }
        if self.pos >= self.last_spurt_transition {
            self.is_last_spurt = true;
        }
    }

    fn update_target_speed(&mut self) {
        if !self.hp.has_remaining() {
            self.target_speed = self.min_speed;
        } else if self.is_last_spurt {
            self.target_speed = self.last_spurt_speed;
        } else {
            self.target_speed = self.base_target[self.phase as usize] * self.pk_speed_coef;
            let section = ((self.pos / self.section_length).floor() as usize)
                .min(self.section_modifier.len() - 1);
            self.target_speed += self.section_modifier[section];
        }
        self.target_speed += self.mod_target.value();
        if self.is_downhill {
            self.target_speed += 0.3 + self.slope_per / 100_000.0;
        } else if self.hill_idx != -1 && self.slope_per > 0.0 {
            self.target_speed -= self.slope_per / 10_000.0 * 200.0 / self.horse.power;
            self.target_speed = self.target_speed.max(self.min_speed);
        }
        if self.compete_fight {
            self.target_speed += (200.0 * self.horse.guts).powf(0.708) * 0.0001;
        }
        if self.lead_competition {
            self.target_speed += (500.0 * self.horse.guts).powf(0.6) * 0.0001;
        }
        if self.lane_change_speed > 0.0 && !self.active_lane.is_empty() {
            self.target_speed += (0.0002 * self.horse.power).sqrt();
        }
    }

    fn apply_forces(&mut self) {
        if !self.hp.has_remaining() {
            self.accel = -1.2;
            return;
        }
        if self.current_speed > self.target_speed {
            self.accel = if self.position_keep == PK_PACE_DOWN {
                -0.5
            } else {
                PHASE_DECEL[self.phase as usize]
            };
            return;
        }
        self.accel = self.base_accel[usize::from(self.slope_per > 0.0) * 3 + self.phase as usize];
        self.accel += self.mod_accel.value();
        if self.compete_fight {
            self.accel += (160.0 * self.horse.guts).powf(0.59) * 0.0001;
        }
    }

    fn downhill_check(&mut self, roll: f64) {
        if self.slope_per < 0.0 && roll < self.horse.wisdom * 0.0004 {
            self.is_downhill = true;
        }
    }

    fn update_hills(&mut self) {
        if self.hill_idx == -1 && self.hill_start.last().is_some_and(|s| self.pos >= *s) {
            self.hill_idx = (self.n_hills - self.hill_start.len()) as i64;
            self.slope_per = self.course.slopes[self.hill_idx as usize].slope;
            self.timers[T_DOWNHILL] = 0.0;
            let roll = self.downhill_rng[self.hill_idx as usize].random();
            self.downhill_check(roll);
            self.hill_start.pop();
        } else if self.hill_idx != -1 && self.hill_end.last().is_some_and(|e| self.pos > *e) {
            self.hill_idx = -1;
            self.slope_per = 0.0;
            self.hill_end.pop();
            self.is_downhill = false;
        }
        if self.timers[T_DOWNHILL] >= 1.0 && self.hill_idx != -1 {
            let roll = self.downhill_rng[self.hill_idx as usize].random();
            if self.is_downhill && roll > 0.8 {
                self.is_downhill = false;
            } else if !self.is_downhill {
                self.downhill_check(roll);
            }
            self.timers[T_DOWNHILL] = 0.0;
        }
    }

    fn update_phase(&mut self) {
        // Phase 3 behaves like phase 2 for speed and skill counts, so the solver stops at 2.
        if self.pos >= self.next_phase && self.phase < 2 {
            self.phase += 1;
            self.next_phase = phase_start(self.course.distance, self.phase + 1);
        }
    }

    fn deactivate(&mut self, id: u32) {
        if !self.record || id == ASITAME || id == STAMINA_SYOUBU {
            return;
        }
        let (pos, t, d) = (self.pos, self.time(), self.course.distance);
        if let Some(a) = self
            .activations
            .iter_mut()
            .find(|a| a.id == id && a.end == -1.0)
        {
            a.end = pos.min(d);
            a.duration = t - a.time;
        }
    }

    fn expire(list: &mut Vec<Active>, timers: &[f64]) -> Vec<Active> {
        let mut out = Vec::new();
        let mut i = list.len();
        while i > 0 {
            i -= 1;
            if timers[list[i].timer] >= 0.0 {
                out.push(list.remove(i));
            }
        }
        out
    }

    fn process_skills(&mut self) {
        for s in Self::expire(&mut self.active_target, &self.timers) {
            self.mod_target.add(-s.modifier);
            self.deactivate(s.id);
        }
        for s in Self::expire(&mut self.active_current, &self.timers) {
            self.mod_current.add(-s.modifier);
            self.deactivate(s.id);
        }
        for s in Self::expire(&mut self.active_accel, &self.timers) {
            self.mod_accel.add(-s.modifier);
            self.deactivate(s.id);
        }
        for s in Self::expire(&mut self.active_lane, &self.timers) {
            self.deactivate(s.id);
        }
        for s in Self::expire(&mut self.active_change_lane, &self.timers) {
            self.deactivate(s.id);
        }
        let mut i = self.pending.len();
        while i > 0 {
            i -= 1;
            if i >= self.pending.len() {
                continue;
            }
            let (trigger, id) = (self.pending[i].trigger, self.pending[i].id);
            // Regions are half-open: at pos == end the uma has left the trigger.
            if self.pos >= trigger.end || self.pending_removal.contains(&id) {
                self.pending.remove(i);
                self.pending_removal.remove(&id);
            } else if self.pos >= trigger.start && self.pending[i].cond.eval(self) {
                let s = self.pending[i].clone();
                if !self.skip_wit_check(&s) && !self.wit_check_passes(&s) {
                    self.pending.remove(i);
                } else {
                    self.activate(&s);
                    // Activation never removes pending skills, so `i` still points at this one.
                    self.pending.remove(i);
                }
            }
        }
    }

    fn wit_check_passes(&mut self, s: &PendingSkill) -> bool {
        let roll = self.wisdom_rng.random();
        let wisdom = match (s.perspective, s.origin_wisdom) {
            (Perspective::Other, Some(w)) => w,
            _ => self.horse.raw_wisdom,
        };
        roll <= (100.0 - 9000.0 / wisdom).max(20.0) * 0.01
    }

    fn skip_wit_check(&self, s: &PendingSkill) -> bool {
        !self.wit_check
            || s.effects
                .first()
                .is_some_and(|e| (SPEED_UP..=WISDOM_UP).contains(&e.kind))
            || s.rarity == RARITY_UNIQUE
    }

    fn duration_multiplier(&self, scaling: i32) -> f64 {
        match scaling {
            3 => {
                let hp = self.hp.current().unwrap_or(f64::INFINITY);
                [
                    (3500.0, 4.0),
                    (3200.0, 3.5),
                    (3000.0, 3.0),
                    (2800.0, 2.5),
                    (2600.0, 2.2),
                    (2400.0, 2.0),
                    (2000.0, 1.5),
                ]
                .iter()
                .find(|(t, _)| hp >= *t)
                .map_or(1.0, |(_, m)| *m)
            }
            7 => {
                let hp = self.hp.current().unwrap_or(f64::INFINITY);
                [(2100.0, 3.0), (2000.0, 2.5), (1800.0, 2.0), (1500.0, 1.5)]
                    .iter()
                    .find(|(t, _)| hp >= *t)
                    .map_or(1.0, |(_, m)| *m)
            }
            // Distance from the leader isn't tracked; assume mid-pack (~5 lengths).
            2 => (0.8f64 + 12.5 / 62.5).min(1.6),
            _ => 1.0,
        }
    }

    fn activate(&mut self, s: &PendingSkill) {
        let d = self.course.distance;
        for ef in s.effects.iter() {
            let mult = if ef.duration_scaling != 1 {
                self.duration_multiplier(ef.duration_scaling)
            } else {
                1.0
            };
            let dur = ef.base_duration
                * (d / 1000.0)
                * mult
                * if s.rarity == RARITY_EVOLUTION {
                    self.special_duration_scaling
                } else {
                    1.0
                };
            let mut modifier = ef.modifier;
            if matches!(ef.value_scaling, Some(8 | 9)) {
                let roll = self.rng.random();
                modifier *= if roll < 0.6 {
                    0.0
                } else if roll < 0.9 {
                    0.02
                } else {
                    0.04
                };
            }
            match ef.kind {
                SPEED_UP => self.horse.speed = (self.horse.speed + modifier).max(1.0),
                STAMINA_UP => {
                    self.horse.stamina = (self.horse.stamina + modifier).max(1.0);
                    self.horse.raw_stamina = (self.horse.raw_stamina + modifier).max(1.0);
                }
                POWER_UP => self.horse.power = (self.horse.power + modifier).max(1.0),
                GUTS_UP => self.horse.guts = (self.horse.guts + modifier).max(1.0),
                WISDOM_UP => self.horse.wisdom = (self.horse.wisdom + modifier).max(1.0),
                MULTIPLY_START_DELAY => self.start_delay *= modifier,
                SET_START_DELAY => self.start_delay = modifier,
                TARGET_SPEED => {
                    self.mod_target.add(modifier);
                    let timer = self.new_timer(-dur);
                    self.active_target.push(Active {
                        id: s.id,
                        timer,
                        modifier,
                    });
                }
                ACCEL => {
                    self.mod_accel.add(modifier);
                    let timer = self.new_timer(-dur);
                    self.active_accel.push(Active {
                        id: s.id,
                        timer,
                        modifier,
                    });
                }
                LANE_MOVEMENT_SPEED => {
                    let timer = self.new_timer(-dur);
                    self.active_lane.push(Active {
                        id: s.id,
                        timer,
                        modifier,
                    });
                }
                CURRENT_SPEED | CURRENT_SPEED_DECEL => {
                    self.mod_current.add(modifier);
                    let timer = self.new_timer(-dur);
                    self.active_current.push(Active {
                        id: s.id,
                        timer,
                        modifier,
                    });
                }
                RECOVERY => {
                    // Only real heals count toward activate_count_heal; negative ones are stamina costs.
                    if s.perspective == Perspective::Mine && modifier > 0.0 {
                        self.activate_count_heal += 1.0;
                    }
                    self.hp.recover(modifier);
                    if self.phase >= 2 && !self.is_last_spurt {
                        self.update_last_spurt(true);
                    }
                }
                ACTIVATE_RANDOM_GOLD => self.activate_random_gold(modifier),
                EXTEND_EVOLVED_DURATION => self.special_duration_scaling = modifier,
                CHANGE_LANE => {
                    let timer = self.new_timer(-dur);
                    self.active_change_lane.push(Active {
                        id: s.id,
                        timer,
                        modifier,
                    });
                }
                _ => {}
            }
        }
        if s.perspective == Perspective::Mine {
            self.activate_count[self.phase as usize] += 1.0;
            if self.pos >= d / 2.0 {
                self.activate_count_later_half += 1.0;
            }
            if self.record && s.id != ASITAME && s.id != STAMINA_SYOUBU {
                self.activations.push(Activation {
                    id: s.id,
                    start: self.pos,
                    end: -1.0,
                    time: self.time(),
                    duration: 0.0,
                });
            }
        }
        self.used.insert(s.id);
    }

    fn activate_random_gold(&mut self, n: f64) {
        let mut golds: Vec<usize> = (0..self.pending.len())
            .filter(|&i| {
                let p = &self.pending[i];
                (p.rarity == RARITY_GOLD || p.rarity == RARITY_EVOLUTION)
                    && p.effects.iter().all(|e| e.kind > WISDOM_UP)
            })
            .collect();
        let mut i = golds.len();
        while i > 0 {
            i -= 1;
            let j = self.gorosi_rng.uniform((i + 1) as f64) as usize;
            golds.swap(i, j.min(i));
        }
        for &g in golds.iter().take(n.max(0.0) as usize) {
            let s = self.pending[g].clone();
            self.activate(&s);
            // Removed on the next pass, since the caller is iterating over `pending`.
            self.pending_removal.insert(s.id);
        }
    }

    /// End-of-race: close the activation records of skills still running.
    pub fn cleanup(&mut self) {
        let ids: Vec<u32> = self
            .active_target
            .iter()
            .chain(&self.active_current)
            .chain(&self.active_accel)
            .chain(&self.active_lane)
            .chain(&self.active_change_lane)
            .map(|a| a.id)
            .collect();
        for id in ids {
            self.deactivate(id);
        }
    }
}

/// Advance solver `i` by `dt` seconds.
pub fn step(solvers: &mut [Solver], i: usize, dt: f64) {
    let snap: Vec<(f64, Strategy)> = solvers
        .iter()
        .map(|s| (s.pos, s.pos_keep_strategy))
        .collect();
    let started = step_one(&mut solvers[i], i, dt, &snap);
    for j in started {
        let len = solvers[j].section_length;
        solvers[j].start_lead_competition(len);
    }
}

fn step_one(s: &mut Solver, me: usize, dt: f64, snap: &[(f64, Strategy)]) -> Vec<usize> {
    let mut dt_after_delay = dt;
    for t in &mut s.timers {
        *t += dt;
    }
    if s.timers[T_CONDITION] >= 0.0 {
        s.tick_conditions();
        s.timers[T_CONDITION] = -1.0;
    }
    if s.start_delay_acc > 0.0 {
        s.start_delay_acc -= dt;
        if s.start_delay_acc > 0.0 {
            return Vec::new();
        }
    }
    s.update_hills();
    s.update_phase();
    s.update_rushed();
    s.process_skills();
    s.apply_position_keep(me, snap);
    s.update_pos_keep_coef();
    s.update_compete_fight();
    let started = s.update_lead_competition(me, snap);
    s.update_last_spurt(false);
    s.update_target_speed();
    s.apply_forces();
    if s.lane_enabled {
        s.apply_lane_movement();
    }
    let mut new_speed = if s.current_speed <= s.target_speed {
        (s.current_speed + s.accel * dt).min(s.target_speed)
    } else {
        (s.current_speed + s.accel * dt).max(s.target_speed)
    };
    if s.start_dash && new_speed > s.max_start_dash_speed() {
        new_speed = s.max_start_dash_speed();
    }
    if !s.start_dash && s.current_speed < s.min_speed {
        new_speed = s.min_speed;
    }
    s.current_speed = new_speed;
    let displacement = s.current_speed + s.mod_current.value();
    if s.start_delay_acc < 0.0 {
        dt_after_delay = s.start_delay_acc.abs();
        s.start_delay_acc = 0.0;
    }
    s.pos += displacement * dt_after_delay;
    let st = s.hp_state();
    s.hp.tick(&st, dt);
    if !s.hp.has_remaining() && !s.hp_died {
        s.hp_died = true;
        s.hp_died_position = Some(s.course.distance - s.pos);
    }
    if s.start_dash && s.current_speed >= 0.85 * base_speed(s.course.distance) {
        s.start_dash = false;
        s.mod_accel.add(-24.0);
    }
    started
}

/// The pacemaker everyone keeps position against: the farthest front runner, else whoever
/// was picked before, else the farthest uma of the next style (who then runs as a front runner).
pub fn get_pacer(solvers: &mut [Solver], caller: usize) -> Option<usize> {
    let umas = solvers[caller].umas.clone();
    let farthest = |solvers: &[Solver], pred: &dyn Fn(&Solver) -> bool| -> Option<usize> {
        let mut best: Option<usize> = None;
        for &u in &umas {
            if pred(&solvers[u]) && best.is_none_or(|b| solvers[u].pos > solvers[b].pos) {
                best = Some(u);
            }
        }
        best
    };
    for strategy in [OONIGE, NIGE] {
        if let Some(u) = farthest(solvers, &|s: &Solver| s.pos_keep_strategy == strategy) {
            return Some(u);
        }
    }
    if let Some(&u) = umas.iter().find(|&&u| solvers[u].pacer_override) {
        return Some(u);
    }
    for strategy in [SENKOU, SASI, OIKOMI] {
        if let Some(u) = farthest(solvers, &|s: &Solver| {
            strategy_matches(s.pos_keep_strategy, strategy)
        }) {
            solvers[u].pacer_override = true;
            solvers[u].pos_keep_strategy = NIGE;
            return Some(u);
        }
    }
    let u = umas.iter().copied().find(|&u| solvers[u].is_pacer)?;
    solvers[u].pos_keep_strategy = NIGE;
    Some(u)
}

/// Mark the first uma to reach the late race (ties broken with the synced rng).
pub fn update_first_in_late_race(solvers: &mut [Solver], caller: usize) {
    let umas = solvers[caller].umas.clone();
    if umas.iter().any(|&u| solvers[u].first_in_late_race) {
        return;
    }
    let snap: Vec<(f64, Strategy)> = solvers
        .iter()
        .map(|s| (s.pos, s.pos_keep_strategy))
        .collect();
    solvers[caller].sort_umas(&snap);
    let sorted = solvers[caller].umas.clone();
    let Some(&first) = sorted.first() else { return };
    if solvers[first].pos < solvers[caller].course.distance * 2.0 / 3.0 {
        return;
    }
    let round = |p: f64| (p * 100.0 + 0.5).floor() / 100.0;
    let lead = round(solvers[first].pos);
    let tied: Vec<usize> = sorted
        .iter()
        .copied()
        .take_while(|&u| round(solvers[u].pos) == lead)
        .collect();
    let pick = solvers[caller].sync_rng.uniform(tied.len() as f64) as usize;
    let chosen = tied[pick.min(tied.len() - 1)];
    solvers[chosen].first_in_late_race = true;
}
