//! Skill activation conditions: the parser for strings like `phase>=2&order_rate<=50`, every
//! condition the engine models (ActivationConditions.ts), how each samples its trigger point
//! (ActivationSamplePolicy.ts), and the dynamic checks that run during the race.

use super::course::{Course, base_speed, phase_end, phase_start};
use super::params::{Horse, NIGE, OIKOMI, RaceParams, SASI, SENKOU, Strategy, strategy_matches};
use super::region::{Region, clip, clip_each, union};
use super::rng::Rng;

// ---------------------------------------------------------------------------
// Sample policies

/// How a condition's static regions become one trigger point per simulated run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Policy {
    /// The first region.
    Immediate,
    /// A random point, weighted by region length.
    Random,
    /// A point drawn from a distribution over the regions (modelled conditions).
    Uniform,
    Erlang(f64, f64),
    /// A random straight, then a random point on it.
    StraightRandom,
    AllCornerRandom,
    /// A forced position (user-set skill position).
    Fixed(f64),
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Immediate,
    Random,
    Distribution,
    Straight,
    AllCorner,
    Fixed,
}

impl Policy {
    fn kind(self) -> Kind {
        match self {
            Policy::Immediate => Kind::Immediate,
            Policy::Random => Kind::Random,
            Policy::Uniform | Policy::Erlang(..) => Kind::Distribution,
            Policy::StraightRandom => Kind::Straight,
            Policy::AllCornerRandom => Kind::AllCorner,
            Policy::Fixed(_) => Kind::Fixed,
        }
    }

    /// Which policy wins when two conditions are joined (the double dispatch in ActivationSamplePolicy.ts):
    /// true randomness beats modelled distributions, which beat immediate conditions.
    pub fn reconcile(self, other: Policy) -> Result<Policy, String> {
        use Kind::*;
        let (a, b) = (self.kind(), other.kind());
        if a == Fixed || b == Fixed {
            return Ok(if a == Fixed { self } else { other });
        }
        // `other.reconcileA(self)`: the receiver keeps itself unless it is lower priority.
        Ok(match (a, b) {
            (Immediate, _) => other,
            (_, Immediate) => self,
            (Distribution, Distribution) => other,
            (Distribution, _) => other,
            (Random, Distribution) => self,
            (Random, Random) => self,
            (Random, _) => other,
            (Straight, Distribution | Random) => self,
            (Straight, Straight) => self,
            (AllCorner, Distribution | Random) => self,
            (AllCorner, AllCorner) => other,
            (Straight, AllCorner) | (AllCorner, Straight) => {
                return Err("cannot reconcile straight_random with all_corner_random".into());
            }
            (Fixed, _) | (_, Fixed) => unreachable!(),
        })
    }

    pub fn sample(self, regions: &[Region], n: usize, rng: &mut Rng) -> Vec<Region> {
        match self {
            Policy::Immediate => regions.iter().take(1).copied().collect(),
            Policy::Fixed(pos) => vec![Region::new(pos, pos + 10.0); n],
            Policy::Random => {
                if regions.is_empty() {
                    return Vec::new();
                }
                let mut acc = 0.0;
                let weights: Vec<f64> = regions
                    .iter()
                    .map(|r| {
                        acc += r.end - r.start;
                        acc
                    })
                    .collect();
                (0..n)
                    .map(|_| {
                        let threshold = rng.uniform(acc);
                        let r = regions[weights
                            .iter()
                            .position(|w| *w > threshold)
                            .unwrap_or(regions.len() - 1)];
                        let pos = r.start + rng.uniform(r.end - r.start - 10.0);
                        Region::new(pos, pos + 10.0)
                    })
                    .collect()
            }
            Policy::Uniform | Policy::Erlang(..) => {
                if regions.is_empty() {
                    return Vec::new();
                }
                let range: f64 = regions.iter().map(|r| r.end - r.start).sum();
                let mut rs = regions.to_vec();
                rs.sort_by(|a, b| a.start.total_cmp(&b.start));
                let randoms = self.distribution(range, n, rng);
                let mut out = Vec::with_capacity(n);
                for i in 0..n {
                    let mut pos = randoms[i];
                    for (j, r) in rs.iter().enumerate() {
                        pos += r.start;
                        if pos > r.end && j + 1 < rs.len() {
                            pos -= r.end;
                        } else {
                            out.push(Region::new(pos.min(r.end), r.end));
                            break;
                        }
                    }
                }
                out
            }
            Policy::StraightRandom => {
                if regions.is_empty() {
                    return Vec::new();
                }
                (0..n)
                    .map(|_| {
                        let r = regions
                            [(rng.uniform(regions.len() as f64) as usize).min(regions.len() - 1)];
                        let pos = r.start + rng.uniform(r.end - r.start - 10.0);
                        Region::new(pos, pos + 10.0)
                    })
                    .collect()
            }
            Policy::AllCornerRandom => (0..n)
                .map(|_| place_corner_triggers(regions, rng))
                .collect(),
        }
    }

    /// `n` draws in [0, upper], rescaled from the 0.1th–99.9th percentiles of the distribution.
    fn distribution(self, upper: f64, n: usize, rng: &mut Rng) -> Vec<f64> {
        match self {
            Policy::Erlang(k, lambda) => {
                let nums: Vec<f64> = (0..n)
                    .map(|_| {
                        let mut u = 1.0;
                        let mut j = 0.0;
                        while j < k {
                            u *= rng.random();
                            j += 1.0;
                        }
                        -u.ln() / lambda
                    })
                    .collect();
                let c = 1.0 / (9.0 * k);
                let min = k * (1.0 - c + -3.09023 * c.sqrt()).powi(3) / lambda;
                let max = k * (1.0 - c + 3.09023 * c.sqrt()).powi(3) / lambda;
                nums.iter()
                    .map(|v| (upper * ((v - min).max(0.0) / (max - min)).min(1.0)).floor())
                    .collect()
            }
            _ => (0..n).map(|_| rng.uniform(upper)).collect(),
        }
    }
}

/// all_corner_random: up to four triggers on random corners, each after the last; the first counts.
fn place_corner_triggers(regions: &[Region], rng: &mut Rng) -> Region {
    let mut triggers = Vec::new();
    let mut candidates = regions.to_vec();
    candidates.sort_by(|a, b| a.start.total_cmp(&b.start));
    while triggers.len() < 4 && !candidates.is_empty() {
        let ci = (rng.uniform(candidates.len() as f64) as usize).min(candidates.len() - 1);
        let c = candidates[ci];
        let start = c.start + rng.uniform(c.end - c.start - 10.0);
        if start + 20.0 <= c.end {
            candidates[ci] = Region::new(start + 10.0, c.end);
        } else {
            candidates.remove(ci);
        }
        candidates.drain(0..ci);
        triggers.push(start);
    }
    let t = triggers.first().copied().unwrap_or(9999.0);
    Region::new(t, t + 10.0)
}

// ---------------------------------------------------------------------------
// Dynamic conditions

/// A check that runs every frame once the uma is inside the trigger region.
#[derive(Clone, Debug, PartialEq)]
pub enum Dyn {
    True,
    False,
    And(Box<Dyn>, Box<Dyn>),
    Or(Box<Dyn>, Box<Dyn>),
    AccTimeGte(f64),
    ActCountAllLte(f64),
    ActCountAllGte(f64),
    /// activate_count_start / _middle / _end_after: skills used in phase 0, 1 or 2+.
    ActCountPhaseGte(usize, f64),
    ActCountHealGte(f64),
    ActCountLaterHalfGte(f64),
    HpPerLte(f64),
    HpPerGte(f64),
    UsedSkill(u32),
    AnySkillUsed,
    BadStart(bool),
    HpEmpty,
    IsLastSpurt,
    LastSpurtCase(i64),
    /// (comparison, post, number of umas)
    PostNumber(Cmp, i64, i64),
    RandomLotLt(f64),
}

impl Dyn {
    pub fn and(a: Dyn, b: Dyn) -> Dyn {
        match (a, b) {
            (Dyn::True, x) | (x, Dyn::True) => x,
            (a, b) => Dyn::And(Box::new(a), Box::new(b)),
        }
    }
    pub fn or(a: Dyn, b: Dyn) -> Dyn {
        match (a, b) {
            (Dyn::True, Dyn::True) => Dyn::True,
            (a, b) => Dyn::Or(Box::new(a), Box::new(b)),
        }
    }
}

/// What dynamic conditions read from the running solver.
pub trait RaceView {
    fn accumulate_time(&self) -> f64;
    fn activate_count(&self) -> [f64; 3];
    fn activate_count_heal(&self) -> f64;
    fn activate_count_later_half(&self) -> f64;
    fn hp_ratio(&self) -> f64;
    fn has_hp(&self) -> bool;
    fn used(&self, id: u32) -> bool;
    fn any_used(&self) -> bool;
    fn start_delay(&self) -> f64;
    fn is_last_spurt(&self) -> bool;
    fn last_spurt_transition(&self) -> f64;
    fn gate_roll(&self) -> f64;
    fn random_lot(&self) -> f64;
}

impl Dyn {
    pub fn eval(&self, s: &dyn RaceView) -> bool {
        match self {
            Dyn::True => true,
            Dyn::False => false,
            Dyn::And(a, b) => a.eval(s) && b.eval(s),
            Dyn::Or(a, b) => a.eval(s) || b.eval(s),
            Dyn::AccTimeGte(t) => s.accumulate_time() >= *t,
            Dyn::ActCountAllLte(n) => s.activate_count().iter().sum::<f64>() <= *n,
            Dyn::ActCountAllGte(n) => s.activate_count().iter().sum::<f64>() >= *n,
            Dyn::ActCountPhaseGte(p, n) => s.activate_count()[*p] >= *n,
            Dyn::ActCountHealGte(n) => s.activate_count_heal() >= *n,
            Dyn::ActCountLaterHalfGte(n) => s.activate_count_later_half() >= *n,
            Dyn::HpPerLte(r) => s.hp_ratio() <= *r,
            Dyn::HpPerGte(r) => s.hp_ratio() >= *r,
            Dyn::UsedSkill(id) => s.used(*id),
            Dyn::AnySkillUsed => s.any_used(),
            Dyn::BadStart(flag) => {
                if *flag {
                    s.start_delay() > 0.08
                } else {
                    s.start_delay() <= 0.08
                }
            }
            Dyn::HpEmpty => !s.has_hp(),
            Dyn::IsLastSpurt => s.is_last_spurt(),
            Dyn::LastSpurtCase(c) => match c {
                1 => s.is_last_spurt() && s.last_spurt_transition() != -1.0,
                2 => s.is_last_spurt() && s.last_spurt_transition() == -1.0,
                _ => !s.is_last_spurt(),
            },
            Dyn::PostNumber(cmp, post, num) => {
                let gate = (s.gate_roll() as i64) % num.max(&1);
                let block = if gate < 9 { gate } else { 1 + (24 - gate) % 8 };
                match cmp {
                    Cmp::Lte => block <= *post,
                    Cmp::Gte => block >= *post,
                    _ => block == *post,
                }
            }
            Dyn::RandomLotLt(lot) => s.random_lot() < *lot,
        }
    }
}

// ---------------------------------------------------------------------------
// Parser

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmp {
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Clone, Debug)]
pub enum Op {
    Cmp { cond: String, cmp: Cmp, arg: i64 },
    And(Box<Op>, Box<Op>),
    Or(Box<Op>, Box<Op>),
}

#[derive(Debug)]
pub enum ParseError {
    /// A condition the engine doesn't model; the skill alternative is skipped.
    Unknown,
    Syntax(String),
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Int(i64),
    Id(String),
    Cmp(Cmp),
    And,
    Or,
    Eof,
}

fn tokenize(s: &str) -> Result<Vec<Tok>, ParseError> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    let is_id = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_';
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() {
            let mut n: i64 = 0;
            while i < b.len() && b[i].is_ascii_digit() {
                n = n * 10 + i64::from(b[i] - b'0');
                i += 1;
            }
            out.push(Tok::Int(n));
        } else if is_id(c) {
            let start = i;
            while i < b.len() && is_id(b[i]) {
                i += 1;
            }
            out.push(Tok::Id(s[start..i].to_string()));
        } else {
            let next = b.get(i + 1).copied();
            match c {
                b'=' if next == Some(b'=') => {
                    out.push(Tok::Cmp(Cmp::Eq));
                    i += 2;
                }
                b'!' if next == Some(b'=') => {
                    out.push(Tok::Cmp(Cmp::Neq));
                    i += 2;
                }
                b'<' | b'>' => {
                    let eq = next == Some(b'=');
                    out.push(Tok::Cmp(match (c, eq) {
                        (b'<', true) => Cmp::Lte,
                        (b'<', false) => Cmp::Lt,
                        (_, true) => Cmp::Gte,
                        _ => Cmp::Gt,
                    }));
                    i += if eq { 2 } else { 1 };
                }
                b'@' => {
                    out.push(Tok::Or);
                    i += 1;
                }
                b'&' => {
                    out.push(Tok::And);
                    i += 1;
                }
                b' ' | b'\n' | b'\r' | b'\t' => i += 1,
                _ => return Err(ParseError::Syntax(format!("invalid character in {s:?}"))),
            }
        }
    }
    out.push(Tok::Eof);
    Ok(out)
}

enum Node {
    Int(i64),
    Cond(String),
    Op(Op),
}

struct Parser {
    toks: Vec<Tok>,
    i: usize,
}

fn lbp(t: &Tok) -> u8 {
    match t {
        Tok::Cmp(_) => 30,
        Tok::And => 20,
        Tok::Or => 10,
        _ => 0,
    }
}

impl Parser {
    fn next(&mut self) -> Tok {
        let t = self.toks.get(self.i).cloned().unwrap_or(Tok::Eof);
        self.i += 1;
        t
    }
    fn peek(&self) -> &Tok {
        self.toks.get(self.i).unwrap_or(&Tok::Eof)
    }
    // Pratt parser: Or ::= And '@' Or | And; And ::= Cmp '&' And | Cmp; Cmp ::= cond op int.
    fn expression(&mut self, rbp: u8) -> Result<Node, ParseError> {
        let mut left = match self.next() {
            Tok::Int(n) => Node::Int(n),
            Tok::Id(name) => {
                if policy_of(&name).is_none() {
                    return Err(ParseError::Unknown);
                }
                Node::Cond(name)
            }
            t => return Err(ParseError::Syntax(format!("unexpected {t:?}"))),
        };
        while rbp < lbp(self.peek()) {
            let t = self.next();
            left = match t {
                Tok::Cmp(cmp) => {
                    let Node::Cond(cond) = left else {
                        return Err(ParseError::Syntax(
                            "expected condition before comparison".into(),
                        ));
                    };
                    let Node::Int(arg) = self.expression(30)? else {
                        return Err(ParseError::Syntax(
                            "expected number after comparison".into(),
                        ));
                    };
                    Node::Op(Op::Cmp { cond, cmp, arg })
                }
                Tok::And | Tok::Or => {
                    let Node::Op(l) = left else {
                        return Err(ParseError::Syntax(
                            "expected comparison before operator".into(),
                        ));
                    };
                    let Node::Op(r) = self.expression(lbp(&t))? else {
                        return Err(ParseError::Syntax(
                            "expected comparison after operator".into(),
                        ));
                    };
                    Node::Op(if t == Tok::And {
                        Op::And(Box::new(l), Box::new(r))
                    } else {
                        Op::Or(Box::new(l), Box::new(r))
                    })
                }
                t => return Err(ParseError::Syntax(format!("unexpected {t:?}"))),
            };
        }
        Ok(left)
    }
}

pub fn parse(s: &str) -> Result<Op, ParseError> {
    let mut p = Parser {
        toks: tokenize(s)?,
        i: 0,
    };
    match p.expression(0)? {
        Node::Op(op) if *p.peek() == Tok::Eof => Ok(op),
        _ => Err(ParseError::Syntax(format!(
            "expected a comparison in {s:?}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Conditions

/// Everything a condition filter can look at when a skill is built.
pub struct Ctx<'a> {
    pub course: &'a Course,
    /// Base stats (before course and ground adjustments).
    pub horse: &'a Horse,
    pub race: &'a RaceParams,
    pub skill_id: u32,
}

const NOOP_ERLANG: Policy = Policy::Erlang(3.0, 2.0);

/// The sample policy of each modelled condition; `None` for conditions the engine doesn't know.
pub fn policy_of(name: &str) -> Option<Policy> {
    use Policy::*;
    Some(match name {
        "accumulatetime"
        | "activate_count_all"
        | "activate_count_end_after"
        | "activate_count_heal"
        | "activate_count_later_half"
        | "activate_count_middle"
        | "activate_count_start"
        | "always"
        | "base_power"
        | "base_speed"
        | "base_stamina"
        | "base_guts"
        | "base_wiz"
        | "corner"
        | "corner_count"
        | "course_distance"
        | "distance_diff_rate"
        | "distance_diff_top"
        | "distance_diff_top_float"
        | "distance_rate"
        | "distance_type"
        | "grade"
        | "ground_condition"
        | "ground_type"
        | "hp_per"
        | "is_activate_other_skill_detail"
        | "is_activate_any_skill"
        | "is_other_character_activate_advantage_skill"
        | "is_basis_distance"
        | "is_badstart"
        | "is_behind_in"
        | "is_dirtgrade"
        | "is_finalcorner"
        | "is_finalcorner_laterhalf"
        | "is_hp_empty_onetime"
        | "is_lastspurt"
        | "is_last_straight"
        | "is_last_straight_onetime"
        | "is_temptation"
        | "is_used_skill_id"
        | "lane_type"
        | "lastspurt"
        | "motivation"
        | "order"
        | "order_rate"
        | "order_rate_in20_continue"
        | "order_rate_in40_continue"
        | "order_rate_in50_continue"
        | "order_rate_in80_continue"
        | "order_rate_out20_continue"
        | "order_rate_out40_continue"
        | "order_rate_out50_continue"
        | "order_rate_out70_continue"
        | "phase"
        | "phase_firsthalf"
        | "phase_firstquarter"
        | "phase_laterhalf"
        | "popularity"
        | "post_number"
        | "random_lot"
        | "remain_distance"
        | "rotation"
        | "running_style"
        | "running_style_count_same"
        | "running_style_count_same_rate"
        | "running_style_count_nige_otherself"
        | "running_style_count_senko_otherself"
        | "running_style_count_sashi_otherself"
        | "running_style_count_oikomi_otherself"
        | "running_style_equal_popularity_one"
        | "same_skill_horse_count"
        | "season"
        | "slope"
        | "straight_front_type"
        | "temptation_count"
        | "time"
        | "track_id"
        | "visiblehorse"
        | "weather" => Immediate,
        "all_corner_random" => AllCornerRandom,
        "bashin_diff_behind"
        | "bashin_diff_infront"
        | "behind_near_lane_time"
        | "behind_near_lane_time_set1"
        | "blocked_all_continuetime"
        | "blocked_front"
        | "blocked_front_continuetime"
        | "blocked_side_continuetime"
        | "change_order_onetime"
        | "change_order_up_end_after"
        | "change_order_up_finalcorner_after"
        | "change_order_up_middle"
        | "infront_near_lane_time"
        | "is_surrounded"
        | "overtake_target_no_order_up_time"
        | "overtake_target_time" => NOOP_ERLANG,
        "is_move_lane" => Erlang(5.0, 1.0),
        "is_overtake" => Erlang(1.0, 2.0),
        "near_count" => Erlang(2.0, 2.0),
        "compete_fight_count" => Uniform,
        "corner_random"
        | "distance_rate_after_random"
        | "down_slope_random"
        | "is_finalcorner_random"
        | "phase_corner_random"
        | "phase_firsthalf_random"
        | "phase_firstquarter_random"
        | "phase_laterhalf_random"
        | "phase_random"
        | "running_style_temptation_count_nige"
        | "running_style_temptation_count_senko"
        | "running_style_temptation_count_sashi"
        | "running_style_temptation_count_oikomi"
        | "running_style_temptation_opponent_count_nige"
        | "running_style_temptation_opponent_count_senko"
        | "running_style_temptation_opponent_count_sashi"
        | "running_style_temptation_opponent_count_oikomi"
        | "temptation_count_behind"
        | "temptation_count_infront"
        | "temptation_opponent_count_behind"
        | "temptation_opponent_count_infront"
        | "up_slope_random"
        | "up_slope_random_later_half" => Random,
        "phase_first_half_straight_random"
        | "phase_latter_half_straight_random"
        | "phase_straight_random"
        | "straight_random"
        | "last_straight_random" => StraightRandom,
        _ => return None,
    })
}

/// The corner_random==1@corner_random==2@… skills, which Umalator pins to the first counted corner.
const CORNER_HACK: [u32; 25] = [
    200331, 200332, 200333, 200341, 200342, 200343, 200351, 200352, 200353, 200971, 200972, 201041,
    201042, 201111, 201112, 201181, 201182, 201251, 201252, 201321, 201322, 201391, 201392, 201461,
    201462,
];
/// Skills whose phase condition gets 10 m of slack (phase checks run after skill activation).
const PHASE_FUDGE: [u32; 10] = [
    100591, 900591, 110261, 910261, 110191, 910191, 120451, 920451, 101502121, 100461,
];

fn cmp_f(cmp: Cmp, a: f64, b: f64) -> bool {
    match cmp {
        Cmp::Eq => a == b,
        Cmp::Neq => a != b,
        Cmp::Lt => a < b,
        Cmp::Lte => a <= b,
        Cmp::Gt => a > b,
        Cmp::Gte => a >= b,
    }
}

/// JavaScript's Math.round (halves round up).
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

type Filtered = Result<(Vec<Region>, Dyn), String>;

fn unsupported(name: &str, cmp: Cmp) -> Filtered {
    Err(format!("unsupported comparison {cmp:?} for {name}"))
}

/// Apply one comparison to the regions where the skill could still trigger.
fn filter(name: &str, cmp: Cmp, regions: &[Region], arg: i64, ctx: &Ctx) -> Filtered {
    let c = ctx.course;
    let d = c.distance;
    let a = arg as f64;
    let keep =
        |ok: bool| -> Filtered { Ok((if ok { regions.to_vec() } else { Vec::new() }, Dyn::True)) };
    let stat = |r: Vec<Region>| -> Filtered { Ok((r, Dyn::True)) };
    let dynamic = |dy: Dyn| -> Filtered { Ok((regions.to_vec(), dy)) };
    let only = |want: Cmp, f: &dyn Fn() -> Filtered| -> Filtered {
        if cmp == want {
            f()
        } else {
            unsupported(name, cmp)
        }
    };
    let value = |v: f64| keep(cmp_f(cmp, v, a));
    let section = |s: f64, e: f64| stat(clip(regions, Region::new(s * d / 24.0, e * d / 24.0)));
    let final_corner = || c.corners.last().map(|fc| (fc.start, fc.start + fc.length));
    let phase_bounds = |p: i64| (phase_start(d, p as i32), phase_end(d, p as i32));
    let order_pos = |numerator: bool| -> f64 {
        if numerator {
            a
        } else {
            js_round(f64::from(ctx.race.num_umas.unwrap_or(0)) * (a / 100.0))
        }
    };
    match name {
        "accumulatetime" => only(Cmp::Gte, &|| {
            let rest = Region::new(0.85 * base_speed(d) * a, d);
            Ok((clip(regions, rest), Dyn::AccTimeGte(a)))
        }),
        "activate_count_all" => match cmp {
            Cmp::Lte => dynamic(Dyn::ActCountAllLte(a)),
            Cmp::Gte => dynamic(Dyn::ActCountAllGte(a)),
            _ => unsupported(name, cmp),
        },
        "activate_count_end_after" => only(Cmp::Gte, &|| dynamic(Dyn::ActCountPhaseGte(2, a))),
        "activate_count_heal" => only(Cmp::Gte, &|| dynamic(Dyn::ActCountHealGte(a))),
        "activate_count_later_half" => only(Cmp::Gte, &|| dynamic(Dyn::ActCountLaterHalfGte(a))),
        "activate_count_middle" => only(Cmp::Gte, &|| dynamic(Dyn::ActCountPhaseGte(1, a))),
        "activate_count_start" => only(Cmp::Gte, &|| dynamic(Dyn::ActCountPhaseGte(0, a))),
        "all_corner_random" => only(Cmp::Eq, &|| stat(clip_each(regions, &c.corner_regions()))),
        "always"
        | "distance_diff_rate"
        | "distance_diff_top"
        | "distance_diff_top_float"
        | "is_behind_in"
        | "is_temptation"
        | "lane_type"
        | "popularity"
        | "running_style_count_same"
        | "running_style_count_same_rate"
        | "running_style_equal_popularity_one"
        | "same_skill_horse_count"
        | "temptation_count"
        | "visiblehorse"
        | "bashin_diff_behind"
        | "bashin_diff_infront"
        | "behind_near_lane_time"
        | "behind_near_lane_time_set1"
        | "blocked_all_continuetime"
        | "blocked_front"
        | "change_order_onetime"
        | "infront_near_lane_time"
        | "is_move_lane"
        | "is_overtake"
        | "is_surrounded"
        | "near_count"
        | "overtake_target_no_order_up_time"
        | "overtake_target_time" => keep(true),
        "base_power" => value(ctx.horse.power),
        "base_speed" => value(ctx.horse.speed),
        "base_stamina" => value(ctx.horse.stamina),
        "base_guts" => value(ctx.horse.guts),
        "base_wiz" => value(ctx.horse.wisdom),
        "blocked_front_continuetime" | "blocked_side_continuetime" => only(Cmp::Gte, &|| {
            // Delay activations that would start at 0 m by ~13 m/s × the required time.
            let min_distance = 13.0 * a;
            stat(
                regions
                    .iter()
                    .map(|r| {
                        if r.start == 0.0 {
                            Region::new(r.start + min_distance, r.end)
                        } else {
                            *r
                        }
                    })
                    .collect(),
            )
        }),
        "change_order_up_end_after" => only(Cmp::Gte, &|| {
            stat(clip(regions, Region::new(phase_start(d, 2), d)))
        }),
        "change_order_up_finalcorner_after" => only(Cmp::Gte, &|| match final_corner() {
            Some((s, _)) => stat(clip(regions, Region::new(s, d))),
            None => stat(Vec::new()),
        }),
        "change_order_up_middle" => only(Cmp::Gte, &|| {
            stat(clip(
                regions,
                Region::new(phase_start(d, 1), phase_end(d, 1)),
            ))
        }),
        "compete_fight_count" => only(Cmp::Gt, &|| stat(clip(regions, c.last_straight()))),
        "corner" => match cmp {
            Cmp::Eq if arg == 0 => {
                let mut last_end = 0.0;
                let mut non_corners: Vec<Region> = c
                    .corners
                    .iter()
                    .map(|k| {
                        let r = Region::new(last_end, k.start);
                        last_end = k.start + k.length;
                        r
                    })
                    .collect();
                if last_end != d {
                    non_corners.push(Region::new(last_end, d));
                }
                stat(clip_each(regions, &non_corners))
            }
            Cmp::Eq => {
                let n = c.corners.len() as i64;
                if n + arg >= 5 {
                    let mut corners = Vec::new();
                    let mut idx = n + arg - 5;
                    while idx >= 0 {
                        let k = c.corners[idx as usize];
                        corners.push(Region::new(k.start, k.start + k.length));
                        idx -= 4;
                    }
                    corners.reverse();
                    stat(clip_each(regions, &corners))
                } else {
                    stat(Vec::new())
                }
            }
            Cmp::Neq => stat(clip_each(regions, &c.corner_regions())),
            _ => unsupported(name, cmp),
        },
        "corner_count" => value(c.corners.len() as f64),
        "corner_random" => only(Cmp::Eq, &|| {
            let n = c.corners.len() as i64;
            if CORNER_HACK.contains(&ctx.skill_id) {
                if arg == 1 && n > 0 {
                    let k = c.corners[(n - 4).max(0) as usize];
                    return stat(clip(regions, Region::new(k.start, k.start + k.length)));
                }
                return stat(Vec::new());
            }
            if n + arg >= 5 {
                let k = c.corners[(n + arg - 5) as usize];
                stat(clip(regions, Region::new(k.start, k.start + k.length)))
            } else {
                stat(Vec::new())
            }
        }),
        "course_distance" => value(d),
        "distance_rate" => match cmp {
            Cmp::Lte => stat(clip(regions, Region::new(0.0, d * a / 100.0))),
            Cmp::Gte => stat(clip(regions, Region::new(d * a / 100.0, d))),
            _ => unsupported(name, cmp),
        },
        "distance_rate_after_random" => only(Cmp::Eq, &|| {
            stat(clip(regions, Region::new(d * a / 100.0, d)))
        }),
        "distance_type" => match cmp {
            Cmp::Eq => keep(f64::from(c.distance_type) == a),
            Cmp::Neq => keep(f64::from(c.distance_type) != a),
            _ => unsupported(name, cmp),
        },
        "down_slope_random" => only(Cmp::Eq, &|| {
            let s: Vec<Region> = c
                .slopes
                .iter()
                .filter(|s| s.slope < 0.0)
                .map(|s| Region::new(s.start, s.start + s.length))
                .collect();
            stat(clip_each(regions, &s))
        }),
        "grade" => value(f64::from(ctx.race.grade)),
        "ground_condition" => value(f64::from(ctx.race.ground)),
        "ground_type" => value(f64::from(c.surface)),
        "hp_per" => match cmp {
            Cmp::Lte => dynamic(Dyn::HpPerLte(a / 100.0)),
            Cmp::Gte => dynamic(Dyn::HpPerGte(a / 100.0)),
            _ => unsupported(name, cmp),
        },
        "is_activate_other_skill_detail" => {
            only(Cmp::Eq, &|| dynamic(Dyn::UsedSkill(ctx.skill_id)))
        }
        "is_activate_any_skill" => only(Cmp::Eq, &|| dynamic(Dyn::AnySkillUsed)),
        "is_other_character_activate_advantage_skill" => only(Cmp::Eq, &|| keep(true)),
        "is_basis_distance" => only(Cmp::Eq, &|| keep(((d % 400.0).min(1.0)) != a)),
        "is_badstart" => only(Cmp::Eq, &|| dynamic(Dyn::BadStart(arg != 0))),
        "is_dirtgrade" => {
            let dirt = [10101, 10103, 10104, 10105].contains(&c.race_track_id);
            match cmp {
                Cmp::Eq => keep(dirt),
                Cmp::Neq => keep(!dirt),
                _ => unsupported(name, cmp),
            }
        }
        "is_finalcorner" => only(Cmp::Eq, &|| match final_corner() {
            Some((s, _)) => stat(clip(
                regions,
                if arg != 0 {
                    Region::new(s, d)
                } else {
                    Region::new(0.0, s)
                },
            )),
            None => stat(Vec::new()),
        }),
        "is_finalcorner_laterhalf" => only(Cmp::Eq, &|| match final_corner() {
            Some((s, e)) => stat(clip(regions, Region::new((s + e) / 2.0, e))),
            None => stat(Vec::new()),
        }),
        "is_finalcorner_random" => only(Cmp::Eq, &|| match final_corner() {
            Some((s, e)) => stat(clip(regions, Region::new(s, e))),
            None => stat(Vec::new()),
        }),
        "is_hp_empty_onetime" => only(Cmp::Eq, &|| dynamic(Dyn::HpEmpty)),
        "is_lastspurt" => only(Cmp::Eq, &|| {
            Ok((
                clip(regions, Region::new(phase_start(d, 2), d)),
                Dyn::IsLastSpurt,
            ))
        }),
        "is_last_straight" => only(Cmp::Eq, &|| stat(clip(regions, c.last_straight()))),
        "is_last_straight_onetime" => only(Cmp::Eq, &|| {
            let s = c.last_straight().start;
            stat(clip(regions, Region::new(s, s + 10.0)))
        }),
        "is_used_skill_id" => only(Cmp::Eq, &|| dynamic(Dyn::UsedSkill(arg as u32))),
        "lastspurt" => only(Cmp::Eq, &|| {
            Ok((
                clip(regions, Region::new(phase_start(d, 2), d)),
                Dyn::LastSpurtCase(arg),
            ))
        }),
        "motivation" => value(f64::from(ctx.race.mood + 3)),
        "order" | "order_rate" => {
            let Some((lo, hi)) = ctx.race.order_range else {
                return keep(true);
            };
            let (lo, hi) = (f64::from(lo), f64::from(hi));
            let pos = order_pos(name == "order");
            let late = Region::new(phase_start(d, 2) + 100.0, d);
            match cmp {
                Cmp::Eq => keep(pos >= lo && pos <= hi),
                Cmp::Neq => keep(pos < lo || pos > hi),
                // Forward-order conditions are ignored late in the race (e.g. NY Opera's unique).
                Cmp::Lt => {
                    if lo < pos {
                        keep(true)
                    } else {
                        stat(clip(regions, late))
                    }
                }
                Cmp::Lte => {
                    if lo <= pos {
                        keep(true)
                    } else {
                        stat(clip(regions, late))
                    }
                }
                Cmp::Gt => keep(pos < hi),
                Cmp::Gte => keep(pos <= hi),
            }
        }
        "order_rate_in20_continue"
        | "order_rate_in40_continue"
        | "order_rate_in50_continue"
        | "order_rate_in80_continue" => only(Cmp::Eq, &|| {
            let rate: f64 = name[13..15].parse::<f64>().unwrap_or(50.0) / 100.0;
            match ctx.race.order_range {
                Some((lo, _)) => keep(
                    f64::from(lo) <= js_round(rate * f64::from(ctx.race.num_umas.unwrap_or(0))),
                ),
                None => keep(true),
            }
        }),
        "order_rate_out20_continue"
        | "order_rate_out40_continue"
        | "order_rate_out50_continue"
        | "order_rate_out70_continue" => only(Cmp::Eq, &|| {
            let rate: f64 = name[14..16].parse::<f64>().unwrap_or(50.0) / 100.0;
            match ctx.race.order_range {
                Some((_, hi)) => keep(
                    js_round(rate * f64::from(ctx.race.num_umas.unwrap_or(0))) <= f64::from(hi),
                ),
                None => keep(true),
            }
        }),
        "phase" => {
            if !(0..=3).contains(&arg) {
                return Err(format!("bad phase {arg}"));
            }
            match cmp {
                Cmp::Eq => {
                    let fudge = if PHASE_FUDGE.contains(&ctx.skill_id) {
                        10.0
                    } else {
                        0.0
                    };
                    let (s, e) = phase_bounds(arg);
                    stat(clip(regions, Region::new(s, e + fudge)))
                }
                Cmp::Lt if arg > 0 => {
                    stat(clip(regions, Region::new(0.0, phase_start(d, arg as i32))))
                }
                Cmp::Lte => stat(clip(regions, Region::new(0.0, phase_end(d, arg as i32)))),
                Cmp::Gt if arg < 3 => stat(clip(
                    regions,
                    Region::new(phase_start(d, arg as i32 + 1), d),
                )),
                Cmp::Gte => stat(clip(regions, Region::new(phase_start(d, arg as i32), d))),
                _ => unsupported(name, cmp),
            }
        }
        "phase_corner_random" => only(Cmp::Eq, &|| {
            let (ps, pe) = phase_bounds(arg);
            let corners: Vec<Region> = c
                .corners
                .iter()
                .filter(|k| {
                    (k.start >= ps && k.start < pe)
                        || (k.start + k.length >= ps && k.start + k.length < pe)
                })
                .map(|k| Region::new(k.start.max(ps), (k.start + k.length).min(pe)))
                .collect();
            stat(clip_each(regions, &corners))
        }),
        "phase_firsthalf" | "phase_firsthalf_random" => only(Cmp::Eq, &|| {
            let (s, e) = phase_bounds(arg);
            stat(clip(regions, Region::new(s, s + (e - s) / 2.0)))
        }),
        "phase_firstquarter" | "phase_firstquarter_random" => only(Cmp::Eq, &|| {
            let (s, e) = phase_bounds(arg);
            stat(clip(regions, Region::new(s, s + (e - s) / 4.0)))
        }),
        "phase_laterhalf" | "phase_laterhalf_random" => only(Cmp::Eq, &|| {
            let (s, e) = phase_bounds(arg);
            stat(clip(regions, Region::new((s + e) / 2.0, e)))
        }),
        "phase_random" => only(Cmp::Eq, &|| {
            let (s, e) = phase_bounds(arg);
            stat(clip(regions, Region::new(s, e)))
        }),
        "phase_first_half_straight_random"
        | "phase_latter_half_straight_random"
        | "phase_straight_random" => only(Cmp::Eq, &|| {
            let (s, e) = phase_bounds(arg);
            let bounds = match name {
                "phase_first_half_straight_random" => Region::new(s, (s + e) / 2.0),
                "phase_latter_half_straight_random" => Region::new((s + e) / 2.0, e),
                _ => Region::new(s, e),
            };
            let straights: Vec<Region> = c
                .straights
                .iter()
                .map(|k| Region::new(k.start, k.end))
                .collect();
            stat(clip(&clip_each(regions, &straights), bounds))
        }),
        "post_number" => {
            let num = i64::from(ctx.race.num_umas.unwrap_or(9).max(1));
            match cmp {
                Cmp::Eq | Cmp::Lte | Cmp::Gte => dynamic(Dyn::PostNumber(cmp, arg, num)),
                _ => unsupported(name, cmp),
            }
        }
        "random_lot" => only(Cmp::Eq, &|| dynamic(Dyn::RandomLotLt(a))),
        "remain_distance" => match cmp {
            Cmp::Eq => stat(clip(regions, Region::new(d - a, d - a + 1.0))),
            Cmp::Lte => stat(clip(regions, Region::new(d - a, d))),
            Cmp::Gte => stat(clip(regions, Region::new(0.0, d - a))),
            _ => unsupported(name, cmp),
        },
        "rotation" => value(f64::from(c.turn)),
        "running_style" => only(Cmp::Eq, &|| {
            keep(strategy_matches(ctx.horse.strategy, arg as Strategy))
        }),
        "running_style_count_nige_otherself" => value(f64::from(u8::from(strategy_matches(
            ctx.horse.strategy,
            NIGE,
        )))),
        "running_style_count_senko_otherself" => value(f64::from(u8::from(strategy_matches(
            ctx.horse.strategy,
            SENKOU,
        )))),
        "running_style_count_sashi_otherself" => value(f64::from(u8::from(strategy_matches(
            ctx.horse.strategy,
            SASI,
        )))),
        "running_style_count_oikomi_otherself" => value(f64::from(u8::from(strategy_matches(
            ctx.horse.strategy,
            OIKOMI,
        )))),
        "running_style_temptation_count_nige"
        | "running_style_temptation_count_senko"
        | "running_style_temptation_count_sashi"
        | "running_style_temptation_count_oikomi"
        | "running_style_temptation_opponent_count_nige"
        | "running_style_temptation_opponent_count_senko"
        | "running_style_temptation_opponent_count_sashi"
        | "running_style_temptation_opponent_count_oikomi"
        | "temptation_count_behind"
        | "temptation_count_infront"
        | "temptation_opponent_count_behind"
        | "temptation_opponent_count_infront" => section(2.0, 9.0),
        "season" => value(f64::from(ctx.race.season)),
        "slope" => only(Cmp::Eq, &|| {
            if !(0..=2).contains(&arg) {
                return Err(format!("bad slope type {arg}"));
            }
            let slopes: Vec<_> = c
                .slopes
                .iter()
                .filter(|s| (arg != 2 && s.slope > 0.0) || (arg != 1 && s.slope < 0.0))
                .collect();
            let pieces: Vec<Region> = if arg == 0 {
                // Flat ground: the gaps between slopes.
                let mut last_end = 0.0;
                let mut flat: Vec<Region> = slopes
                    .iter()
                    .map(|s| {
                        let r = Region::new(last_end, s.start);
                        last_end = s.start + s.length;
                        r
                    })
                    .collect();
                if last_end != d {
                    flat.push(Region::new(last_end, d));
                }
                flat
            } else {
                slopes
                    .iter()
                    .map(|s| Region::new(s.start, s.start + s.length))
                    .collect()
            };
            stat(clip_each(regions, &pieces))
        }),
        "straight_front_type" => only(Cmp::Eq, &|| {
            let s: Vec<Region> = c
                .straights
                .iter()
                .filter(|s| i64::from(s.front_type) == arg)
                .map(|s| Region::new(s.start, s.end))
                .collect();
            stat(clip_each(regions, &s))
        }),
        "straight_random" => only(Cmp::Eq, &|| {
            let s: Vec<Region> = c
                .straights
                .iter()
                .map(|s| Region::new(s.start, s.end))
                .collect();
            stat(clip_each(regions, &s))
        }),
        "last_straight_random" => only(Cmp::Eq, &|| stat(clip(regions, c.last_straight()))),
        "time" => value(f64::from(ctx.race.time)),
        "track_id" => value(f64::from(c.race_track_id)),
        "up_slope_random" | "up_slope_random_later_half" => only(Cmp::Eq, &|| {
            let s: Vec<Region> = c
                .slopes
                .iter()
                .filter(|s| s.slope > 0.0)
                .map(|s| Region::new(s.start, s.start + s.length))
                .collect();
            let r = clip_each(regions, &s);
            stat(if name == "up_slope_random_later_half" {
                clip(&r, Region::new(d / 2.0, d))
            } else {
                r
            })
        }),
        "weather" => value(f64::from(ctx.race.weather)),
        _ => Err(format!("unknown condition {name}")),
    }
}

impl Op {
    pub fn policy(&self) -> Result<Policy, String> {
        match self {
            Op::Cmp { cond, .. } => {
                policy_of(cond).ok_or_else(|| format!("unknown condition {cond}"))
            }
            Op::And(a, b) | Op::Or(a, b) => a.policy()?.reconcile(b.policy()?),
        }
    }

    /// Narrow `regions` to where the condition can hold, plus the check left for race time.
    pub fn apply(&self, regions: &[Region], ctx: &Ctx) -> Result<(Vec<Region>, Dyn), String> {
        match self {
            Op::Cmp { cond, cmp, arg } => filter(cond, *cmp, regions, *arg, ctx),
            Op::And(a, b) => {
                let (lv, lc) = a.apply(regions, ctx)?;
                let (rv, rc) = b.apply(&lv, ctx)?;
                Ok((rv, Dyn::and(lc, rc)))
            }
            Op::Or(a, b) => {
                let (lv, lc) = a.apply(regions, ctx)?;
                let (rv, rc) = b.apply(regions, ctx)?;
                Ok((union(&lv, &rv), Dyn::or(lc, rc)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_precedence() {
        let op = parse("phase>=2&order_rate<=50@remain_distance<=200").unwrap();
        assert!(matches!(op, Op::Or(..)), "{op:?}");
        assert!(matches!(
            parse("not_a_condition==1"),
            Err(ParseError::Unknown)
        ));
        assert!(parse("phase>=").is_err());
    }

    #[test]
    fn policies_reconcile() {
        use Policy::*;
        assert_eq!(Immediate.reconcile(Random).unwrap(), Random);
        assert_eq!(Random.reconcile(Immediate).unwrap(), Random);
        assert_eq!(Erlang(3.0, 2.0).reconcile(Random).unwrap(), Random);
        assert_eq!(Random.reconcile(Erlang(3.0, 2.0)).unwrap(), Random);
        assert_eq!(StraightRandom.reconcile(Random).unwrap(), StraightRandom);
        assert!(StraightRandom.reconcile(AllCornerRandom).is_err());
    }
}
