//! Turns a build into ready-to-run solvers: skill data → trigger regions → one sampled trigger
//! per run (RaceSolverBuilder.ts).

use std::collections::HashMap;
use std::sync::Arc;

use super::conditions::{Ctx, Dyn, ParseError, Policy, parse};
use super::course::{Course, phase_start};
use super::data::{SimData, SkillAlternative};
use super::hp::{GameHp, Hp};
use super::params::{Horse, HorseDesc, NIGE, RaceParams, adjusted_stats, base_stats};
use super::region::{Region, clip};
use super::rng::Rng;
use super::solver::{
    ACCEL, ASITAME, DuelingRates, EXTEND_EVOLVED_DURATION, Effect, KNOWN_EFFECTS, NOOP,
    PendingSkill, Perspective, PosKeepMode, Solver, SolverParams,
};

/// Skill level scaling from master.mdb's skill_level_value table (unique skill levels).
pub fn level_coef(effect: i32, level: u32) -> f64 {
    if level <= 1 {
        return 1.0;
    }
    const STAT: [f64; 10] = [1.0, 1.01, 1.02, 1.03, 1.04, 1.05, 1.06, 1.07, 1.08, 1.10];
    const TARGET: [f64; 10] = [1.0, 1.01, 1.04, 1.07, 1.10, 1.13, 1.16, 1.19, 1.22, 1.25];
    const ACC: [f64; 10] = [1.0, 1.02, 1.04, 1.06, 1.08, 1.10, 1.125, 1.15, 1.175, 1.20];
    const DEFAULT: [f64; 10] = [1.0, 1.02, 1.04, 1.06, 1.08, 1.10, 1.12, 1.14, 1.16, 1.18];
    let table = match effect {
        1..=5 => &STAT,
        27 => &TARGET,
        31 => &ACC,
        _ => &DEFAULT,
    };
    table[(level as usize).min(10) - 1]
}

/// A skill's activation as built for one course and uma, before trigger sampling.
#[derive(Clone)]
pub struct SkillData {
    pub id: u32,
    pub perspective: Perspective,
    pub rarity: i32,
    pub policy: Policy,
    pub regions: Vec<Region>,
    pub cond: Arc<Dyn>,
    pub effects: Arc<Vec<Effect>>,
}

fn is_target(p: Perspective, target: i32) -> bool {
    target == 2 || ((p == Perspective::Mine) == (target == 1))
}

fn build_effects(alt: &SkillAlternative, p: Perspective, level: u32) -> Vec<Effect> {
    let mut effects: Vec<Effect> = alt
        .effects
        .iter()
        .map(|e| Effect {
            kind: if KNOWN_EFFECTS.contains(&e.kind) && is_target(p, e.target) {
                e.kind
            } else {
                NOOP
            },
            base_duration: alt.base_duration / 10000.0,
            modifier: e.modifier / 10000.0 * level_coef(e.kind, level),
            duration_scaling: alt.duration_scaling.filter(|d| *d != 0).unwrap_or(1),
            value_scaling: e.scaling.filter(|s| *s > 1),
        })
        .collect();
    // The evolved-duration extension applies last, so it doesn't stretch its own skill's effects.
    effects.sort_by_key(|e| e.kind == EXTEND_EVOLVED_DURATION);
    effects
}

/// The trigger regions, sample policy and dynamic check for each alternative of a skill that can
/// activate on this course. Returns why it can't be built, e.g. a condition the engine doesn't model.
pub fn build_skill_data(
    data: &SimData,
    horse: &Horse,
    race: &RaceParams,
    course: &Course,
    id: u32,
    p: Perspective,
    level: u32,
) -> Result<Vec<SkillData>, String> {
    let def = data
        .skills
        .get(&id)
        .ok_or_else(|| format!("unknown skill {id}"))?;
    let whole = vec![Region::new(0.0, course.distance)];
    let ctx = Ctx {
        course,
        horse,
        race,
        skill_id: id,
    };
    let rarity = if (3..=5).contains(&def.rarity) {
        3
    } else {
        def.rarity
    };
    let mut out: Vec<SkillData> = Vec::new();
    for alt in &def.alternatives {
        let mut full = whole.clone();
        if !alt.precondition.is_empty() {
            let pre = match parse(&alt.precondition) {
                Ok(op) => op,
                Err(ParseError::Unknown) => continue,
                Err(ParseError::Syntax(e)) => return Err(e),
            };
            let (pre_regions, _) = pre.apply(&whole, &ctx)?;
            let Some(first) = pre_regions.first() else {
                continue;
            };
            full = clip(&full, Region::new(first.start, course.distance));
        }
        let op = match parse(&alt.condition) {
            Ok(op) => op,
            Err(ParseError::Unknown) => continue,
            Err(ParseError::Syntax(e)) => return Err(e),
        };
        let (regions, cond) = op.apply(&full, &ctx)?;
        if regions.is_empty() {
            continue;
        }
        // Only skills whose second trigger depends on the first place two triggers.
        if !out.is_empty()
            && !(alt.condition.contains("is_activate_other_skill_detail")
                || alt.condition.contains("is_used_skill_id"))
        {
            continue;
        }
        let effects = build_effects(alt, p, level);
        if !effects.is_empty() {
            out.push(SkillData {
                id,
                perspective: p,
                rarity,
                policy: op.policy()?,
                regions,
                cond: Arc::new(cond),
                effects: Arc::new(effects),
            });
        }
    }
    if !out.is_empty() {
        return Ok(out);
    }
    // Nothing can trigger here; keep the skill (past the finish, never true) so random-gold
    // effects can still pick it.
    let Some(first) = def.alternatives.first() else {
        return Ok(Vec::new());
    };
    let effects = build_effects(first, p, level);
    if effects.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![SkillData {
        id,
        perspective: p,
        rarity,
        policy: Policy::Immediate,
        regions: vec![Region::new(9999.0, 9999.0)],
        cond: Arc::new(Dyn::False),
        effects: Arc::new(effects),
    }])
}

/// Conserve Power (足を貯める): extra late-race acceleration once displayed Power passes 1200.
fn asitame_modifier(power: f64, strategy: i32, distance_type: i32) -> f64 {
    const COEF: [[f64; 6]; 5] = [
        [0.0; 6],
        [0.0, 1.0, 0.7, 0.75, 0.7, 1.0],
        [0.0, 1.0, 0.8, 0.7, 0.75, 1.0],
        [0.0, 1.0, 0.9, 0.875, 0.86, 1.0],
        [0.0, 1.0, 0.9, 1.0, 0.9, 1.0],
    ];
    0.00875
        * (power - 1200.0).sqrt()
        * COEF[distance_type.clamp(0, 4) as usize][strategy.clamp(0, 5) as usize]
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HpKind {
    Game,
    ForceFull(bool),
    CalcRequired,
}

#[derive(Clone)]
struct SkillReq {
    id: u32,
    perspective: Perspective,
    origin_wisdom: Option<f64>,
    level: u32,
}

#[derive(Clone)]
pub struct Builder {
    pub nsamples: usize,
    pub course: Arc<Course>,
    pub race: RaceParams,
    horse: Option<HorseDesc>,
    seed: f64,
    rng: Rng,
    skills: Vec<SkillReq>,
    forced: HashMap<(u32, Perspective), f64>,
    pub pos_keep: PosKeepMode,
    /// "compare" (true) or "chart" (false); both use the game HP model.
    pub compare_mode: bool,
    pub wit_check: bool,
    pub rushed: bool,
    pub compete_fight: bool,
    pub lead_competition: bool,
    pub lane_movement: bool,
    pub dueling: Option<DuelingRates>,
    asitame: Option<f64>,
    pub hp_kind: HpKind,
    pacer_default_skills: Vec<PendingSkill>,
    pacer_skill_data: Vec<SkillData>,
}

impl Builder {
    pub fn new(nsamples: usize, course: Arc<Course>, race: RaceParams, seed: f64) -> Builder {
        Builder {
            nsamples,
            course,
            race,
            horse: None,
            seed,
            rng: Rng::new(seed),
            skills: Vec::new(),
            forced: HashMap::new(),
            pos_keep: PosKeepMode::None,
            compare_mode: true,
            wit_check: true,
            rushed: true,
            compete_fight: false,
            lead_competition: false,
            lane_movement: true,
            dueling: None,
            asitame: None,
            hp_kind: HpKind::Game,
            pacer_default_skills: Vec::new(),
            pacer_skill_data: Vec::new(),
        }
    }

    /// A copy that draws the same random numbers (same seed).
    pub fn fork(&self) -> Builder {
        let mut b = self.clone();
        b.rng = Rng::new(self.seed);
        b
    }

    /// Reseed from this builder's own stream, so a forked pair stops sharing draws.
    pub fn desync(&mut self) {
        self.seed = self.rng.int32();
        self.rng = Rng::new(self.seed);
    }

    pub fn horse(&mut self, h: HorseDesc) {
        self.horse = Some(h);
    }

    pub fn add_skill(
        &mut self,
        id: u32,
        p: Perspective,
        origin_wisdom: Option<f64>,
        level: u32,
        forced: Option<f64>,
    ) {
        self.skills.push(SkillReq {
            id,
            perspective: p,
            origin_wisdom,
            level,
        });
        if let Some(pos) = forced {
            self.forced.insert((id, p), pos);
        }
    }

    /// Must come after `horse`: conserve power reads displayed Power at the builder's mood (great).
    pub fn with_asitame(&mut self) {
        if let Some(h) = &self.horse {
            self.asitame = Some(h.power * (1.0 + 0.02 * f64::from(self.race.mood)));
        }
    }

    fn policy_for(&self, sd: &SkillData) -> Policy {
        self.forced
            .get(&(sd.id, sd.perspective))
            .map_or(sd.policy, |p| Policy::Fixed(*p))
    }

    /// The default pacemaker for approximate position keep: this uma's stats as a front runner,
    /// with its skills (or, without any, two early acceleration skills). Returns its race stats.
    pub fn use_default_pacer(&mut self, data: &SimData, skill_ids: &[u32]) -> Horse {
        let mut desc = self.horse.clone().expect("horse set before pacer");
        desc.strategy = NIGE;
        let base = base_stats(&desc);
        self.pacer_default_skills = [(201_601u32, 3.0), (200_532, 1.2)]
            .into_iter()
            .map(|(id, dur)| PendingSkill {
                id,
                perspective: Perspective::Mine,
                rarity: 1,
                trigger: Region::new(0.0, 100.0),
                cond: Arc::new(Dyn::True),
                effects: Arc::new(vec![Effect {
                    kind: ACCEL,
                    base_duration: dur,
                    modifier: 0.2,
                    duration_scaling: 1,
                    value_scaling: None,
                }]),
                origin_wisdom: None,
            })
            .collect();
        self.pacer_skill_data = skill_ids
            .iter()
            .filter(|id| data.skills.contains_key(id))
            .flat_map(|id| {
                build_skill_data(
                    data,
                    &base,
                    &self.race,
                    &self.course,
                    *id,
                    Perspective::Mine,
                    1,
                )
                .unwrap_or_default()
            })
            .collect();
        adjusted_stats(&base, &self.course, self.race.ground)
    }

    pub fn build_pacer(&self, horse: &Horse, i: usize, mut rng: Rng) -> Solver {
        let skills: Vec<PendingSkill> = if self.pacer_skill_data.is_empty() {
            self.pacer_default_skills.clone()
        } else {
            let triggers: Vec<Vec<Region>> = self
                .pacer_skill_data
                .iter()
                .map(|sd| {
                    self.policy_for(sd)
                        .sample(&sd.regions, self.nsamples, &mut rng)
                })
                .collect();
            self.pacer_skill_data
                .iter()
                .zip(&triggers)
                .filter(|(_, t)| !t.is_empty())
                .map(|(sd, t)| PendingSkill {
                    id: sd.id,
                    perspective: sd.perspective,
                    rarity: sd.rarity,
                    trigger: t[i % t.len()],
                    cond: sd.cond.clone(),
                    effects: sd.effects.clone(),
                    origin_wisdom: None,
                })
                .collect()
        };
        Solver::new(SolverParams {
            horse: *horse,
            course: self.course.clone(),
            rng,
            skills,
            hp: Hp::Noop,
            pos_keep_mode: self.pos_keep,
            compare_mode: self.compare_mode,
            is_pacer: true,
            wit_check: true,
            rushed: true,
            compete_fight: self.compete_fight,
            lead_competition: self.lead_competition,
            dueling: self.dueling,
            lane_movement: self.lane_movement,
        })
    }

    /// Build the skill triggers for all runs; `Built::next` then makes one solver per run.
    pub fn build(&self, data: &SimData) -> Built {
        let desc = self.horse.clone().expect("horse set before build");
        let base = base_stats(&desc);
        let mut rng = self.rng.clone();
        let mut skill_rng = Rng::new(rng.int32());
        let mut skilldata: Vec<(SkillData, Option<f64>)> = Vec::new();
        let mut warnings = Vec::new();
        for req in &self.skills {
            if !data.skills.contains_key(&req.id) {
                continue;
            }
            match build_skill_data(
                data,
                &base,
                &self.race,
                &self.course,
                req.id,
                req.perspective,
                req.level,
            ) {
                Ok(list) => skilldata.extend(list.into_iter().map(|sd| (sd, req.origin_wisdom))),
                Err(e) => warnings.push(format!("{}: {e}", data.skill_name(req.id))),
            }
        }
        if let Some(display_power) = self.asitame {
            let power = skilldata
                .iter()
                .filter(|(sd, _)| sd.regions.first().is_some_and(|r| r.start < 9999.0))
                .filter_map(|(sd, _)| {
                    sd.effects
                        .iter()
                        .find(|e| e.kind == super::solver::POWER_UP)
                        .map(|e| e.modifier)
                })
                .fold(display_power, |a, b| a + b);
            if power > 1200.0 {
                let d = self.course.distance;
                skilldata.push((
                    SkillData {
                        id: ASITAME,
                        perspective: Perspective::Mine,
                        rarity: 1,
                        policy: Policy::Immediate,
                        regions: vec![Region::new(phase_start(d, 2), d)],
                        cond: Arc::new(Dyn::True),
                        effects: Arc::new(vec![Effect {
                            kind: ACCEL,
                            base_duration: 3.0 / (d / 1000.0),
                            modifier: asitame_modifier(
                                power,
                                base.strategy,
                                self.course.distance_type,
                            ),
                            duration_scaling: 1,
                            value_scaling: None,
                        }]),
                    },
                    None,
                ));
            }
        }
        let triggers: Vec<Vec<Region>> = skilldata
            .iter()
            .map(|(sd, _)| {
                self.policy_for(sd)
                    .sample(&sd.regions, self.nsamples, &mut skill_rng)
            })
            .collect();
        Built {
            horse: adjusted_stats(&base, &self.course, self.race.ground),
            skilldata,
            triggers,
            rng,
            index: 0,
            builder: self.clone(),
            warnings,
        }
    }
}

pub struct Built {
    pub horse: Horse,
    skilldata: Vec<(SkillData, Option<f64>)>,
    triggers: Vec<Vec<Region>>,
    rng: Rng,
    index: usize,
    builder: Builder,
    /// Skills left out because their data couldn't be built.
    pub warnings: Vec<String>,
}

impl Built {
    pub fn next_solver(&mut self) -> Solver {
        let i = self.index;
        self.index += 1;
        let solver_rng = Rng::new(self.rng.int32());
        let skills: Vec<PendingSkill> = self
            .skilldata
            .iter()
            .zip(&self.triggers)
            .filter(|(_, t)| !t.is_empty())
            .map(|((sd, ow), t)| PendingSkill {
                id: sd.id,
                perspective: sd.perspective,
                rarity: sd.rarity,
                trigger: t[i % t.len()],
                cond: sd.cond.clone(),
                effects: sd.effects.clone(),
                origin_wisdom: *ow,
            })
            .collect();
        let hp_rng = Rng::new(self.rng.int32());
        let b = &self.builder;
        let game = || GameHp::new(&b.course, b.race.ground, hp_rng.clone());
        let hp = match b.hp_kind {
            HpKind::Game => Hp::Game(game()),
            HpKind::ForceFull(force) => Hp::ForceFull {
                hp: game(),
                force,
                balance: f64::INFINITY,
                was_full: false,
                downhill_save: 0.0,
            },
            HpKind::CalcRequired => Hp::CalcRequired(game()),
        };
        Solver::new(SolverParams {
            horse: self.horse,
            course: b.course.clone(),
            rng: solver_rng,
            skills,
            hp,
            pos_keep_mode: b.pos_keep,
            compare_mode: b.compare_mode,
            is_pacer: false,
            wit_check: b.wit_check,
            rushed: b.rushed,
            compete_fight: b.compete_fight,
            lead_competition: b.lead_competition,
            dueling: b.dueling,
            lane_movement: b.lane_movement,
        })
    }
}
