//! Career simulation: a Monte Carlo of the planned training run (trainee, deck, scenario) that
//! estimates the skill points and stats at the end, and what each card adds.
//!
//! The rules come from `knowledge/scenarios.json`, which lists a source for each number or marks
//! it as an assumption. This is a model of the career, not the game: it trains greedily toward
//! the plan's target stats, rests below a fixed energy, and draws events from fixed odds. Treat
//! its output as an estimate, and calibrate it against careers logged in results/careers.md.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::knowledge::{CardInfo, Knowledge, UmaInfo, effect};
use crate::sim::rng::Rng;

#[derive(Deserialize, Clone, Debug)]
pub struct Assumed {
    pub mood: f64,
    pub race_energy: f64,
    pub rest_energy: f64,
    pub rest_below_energy: f64,
    pub objective_win_rate: f64,
    pub placed_reward_factor: f64,
    pub optional_races: u32,
    pub hint_chance: f64,
    pub chain_event_chance: f64,
    pub random_event_chance: f64,
    pub date_event_chance: f64,
    pub uma_event_chance: f64,
    pub summer_camp_level: usize,
    /// Energy events and outings give back, on average per turn.
    #[serde(default)]
    pub event_energy_per_turn: f64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Common {
    pub turns: usize,
    pub summer_camp_turns: Vec<usize>,
    pub level_multiplier: Vec<f64>,
    pub trainings_per_level: u32,
    pub mood_step: f64,
    pub support_bonus: f64,
    pub appearance_weight: f64,
    pub absent_weight: f64,
    pub rainbow_bond: f64,
    pub bond_per_training: f64,
    pub bond_per_hint: f64,
    pub assumed: Assumed,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ExtraSp {
    pub what: String,
    pub sp: f64,
    #[serde(default)]
    pub race_bonus: bool,
    #[serde(default)]
    pub sourced: bool,
    /// Extra SP per event for each linked card in the deck, over `per` events.
    #[serde(default)]
    pub link_extra: f64,
    #[serde(default)]
    pub per: f64,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct LinkBonus {
    #[serde(default)]
    pub sp: f64,
    #[serde(default)]
    pub stats: f64,
    #[serde(default)]
    pub assumed: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Scenario {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub global: bool,
    #[serde(default)]
    pub source: String,
    /// Per facility (Speed, Stamina, Power, Guts, Wit): Spd, Sta, Pow, Guts, Wit, SP, energy at level 1.
    pub base_training: Vec<[f64; 7]>,
    pub stat_caps: [f64; 5],
    #[serde(default)]
    pub link: Vec<String>,
    #[serde(default)]
    pub link_note: String,
    #[serde(default)]
    pub link_golds: HashMap<String, String>,
    pub race_sp: HashMap<String, f64>,
    #[serde(default)]
    pub finale_races: u32,
    #[serde(default)]
    pub optional_races: Option<u32>,
    #[serde(default)]
    pub extra_sp: Vec<ExtraSp>,
    /// (from turn, extra SP per training).
    #[serde(default)]
    pub sp_per_training_from: Vec<(usize, f64)>,
    #[serde(default)]
    pub link_bonus: LinkBonus,
    #[serde(default)]
    pub event_chance_multiplier: Option<f64>,
    #[serde(default)]
    pub card_note: String,
}

impl Scenario {
    pub fn matches(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
            || self.aliases.iter().any(|a| a.eq_ignore_ascii_case(name))
    }

    pub fn is_linked(&self, uma: &str) -> bool {
        self.link.iter().any(|l| l == uma)
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Scenarios {
    pub updated: String,
    pub common: Common,
    pub scenarios: Vec<Scenario>,
}

pub const SCENARIOS_FILE: &str = "knowledge/scenarios.json";

impl Scenarios {
    pub fn load(root: Option<&Path>) -> Option<Scenarios> {
        let text = std::fs::read_to_string(root?.join(SCENARIOS_FILE)).ok()?;
        serde_json::from_str(&text)
            .map_err(|e| eprintln!("{SCENARIOS_FILE}: {e}"))
            .ok()
    }

    pub fn get(&self, name: &str) -> Option<&Scenario> {
        self.scenarios.iter().find(|s| s.matches(name))
    }
}

/// Facility of a card type: Speed 0 … Wit 4; Pal and Group have none.
pub fn facility_of(kind: &str) -> Option<usize> {
    ["Speed", "Stamina", "Power", "Guts", "Wit"]
        .iter()
        .position(|k| *k == kind)
}

/// One deck card's numbers at the level it's used at.
#[derive(Clone, Debug, Default)]
pub struct CardStats {
    pub id: u32,
    pub uma: String,
    pub facility: Option<usize>,
    pub friendship: f64,
    pub mood: f64,
    pub training: f64,
    pub stat_bonus: [f64; 5],
    pub sp_bonus: f64,
    pub init_stats: [f64; 5],
    pub init_bond: f64,
    pub race_bonus: f64,
    pub hint_levels: f64,
    pub hint_freq: f64,
    pub specialty: f64,
    pub energy_cut: f64,
    pub wit_recovery: f64,
    /// Bond needed before the unique effect's bonuses apply (0: always), and those bonuses.
    pub unique_bond: f64,
    pub unique: Vec<(u32, f64)>,
    pub hints: Vec<u32>,
    /// Each event's best option: (kind, SP, stats). Kinds: chain, random, date.
    pub events: Vec<(EventKind, Gain)>,
    pub linked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventKind {
    Chain,
    Random,
    Date,
}

/// Skill points and stats an event option gives.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gain {
    pub sp: f64,
    pub stats: [f64; 5],
}

impl Gain {
    fn total(&self) -> f64 {
        self.sp * 0.5 + self.stats.iter().sum::<f64>()
    }
}

/// "+10/+20" → the mean of the numbers after `key` in a line.
fn number_after(line: &str, key: &str) -> Option<f64> {
    let l = line.to_ascii_lowercase();
    let i = l.find(key)?;
    let nums: Vec<f64> = l[i + key.len()..]
        .split('/')
        .filter_map(|p| {
            p.trim()
                .trim_start_matches('+')
                .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
                .next()?
                .parse()
                .ok()
        })
        .collect();
    (!nums.is_empty()).then(|| nums.iter().sum::<f64>() / nums.len() as f64)
}

/// "Skill points +20" → 20; "Skill Points +10/+20" → their mean.
pub fn sp_in(line: &str) -> Option<f64> {
    number_after(line, "skill points")
}

/// What one outcome line gives: "Speed +10", "All stats +5", "Skill points +20". A line can
/// list several, separated by commas.
pub fn gain_in(line: &str) -> Gain {
    let mut g = Gain::default();
    for part in line.split(',') {
        let p = part.trim().to_ascii_lowercase();
        if let Some(v) = sp_in(&p) {
            g.sp += v;
            continue;
        }
        if p.starts_with("all stats") || p.starts_with("all status") {
            if let Some(v) =
                number_after(&p, "all stats").or_else(|| number_after(&p, "all status"))
            {
                for s in &mut g.stats {
                    *s += v;
                }
            }
            continue;
        }
        for (i, key) in ["speed", "stamina", "power", "guts", "wit"]
            .iter()
            .enumerate()
        {
            if p.starts_with(key) || (i == 4 && p.starts_with("wisdom")) {
                if let Some(v) = number_after(
                    &p,
                    if i == 4 && p.starts_with("wisdom") {
                        "wisdom"
                    } else {
                        key
                    },
                ) {
                    g.stats[i] += v;
                }
            }
        }
    }
    g
}

/// The best option of a card event: alternatives averaged, then the option worth most.
fn event_gain(options: &[Vec<Vec<String>>]) -> Gain {
    options
        .iter()
        .map(|alts| {
            let n = alts.len().max(1) as f64;
            let mut sum = Gain::default();
            for lines in alts {
                for l in lines {
                    let g = gain_in(l);
                    sum.sp += g.sp / n;
                    for i in 0..5 {
                        sum.stats[i] += g.stats[i] / n;
                    }
                }
            }
            sum
        })
        .fold(Gain::default(), |best, g| {
            if g.total() > best.total() { g } else { best }
        })
}

/// Effect ids by name, for reading unique effects ("Training Effectiveness" → 8).
pub fn effect_names(kb: &Knowledge) -> HashMap<String, u32> {
    let mut out = HashMap::new();
    for c in kb.cards.values() {
        for e in &c.effects {
            out.entry(e.name.clone()).or_insert(e.kind);
        }
    }
    out
}

pub fn card_stats(
    info: &CardInfo,
    level: u32,
    names: &HashMap<String, u32>,
    scenario: Option<&Scenario>,
) -> CardStats {
    let e = |k: u32| f64::from(info.effect(k, level).unwrap_or(0));
    let mut s = CardStats {
        id: info.id,
        uma: info.uma.clone(),
        facility: facility_of(&info.kind),
        friendship: e(effect::FRIENDSHIP),
        mood: e(effect::MOOD),
        training: e(effect::TRAINING),
        stat_bonus: [0, 1, 2, 3, 4].map(|i| e(effect::SPEED + i)),
        sp_bonus: e(effect::SKILL_POINTS),
        init_stats: [0, 1, 2, 3, 4].map(|i| e(effect::INIT_SPEED + i)),
        init_bond: e(effect::INIT_BOND),
        race_bonus: e(effect::RACE),
        hint_levels: e(effect::HINT_LEVELS),
        hint_freq: e(effect::HINT_FREQ),
        specialty: e(effect::SPECIALTY),
        energy_cut: e(effect::ENERGY_COST),
        wit_recovery: e(effect::WIT_RECOVERY),
        hints: info.hints.clone(),
        linked: scenario.is_some_and(|sc| sc.is_linked(&info.uma)),
        ..Default::default()
    };
    if let Some(u) = info.unique.as_ref().filter(|u| level >= u.level) {
        // "At Bond 80" uniques apply once the bond is there; plain ones always.
        s.unique_bond = u
            .condition
            .strip_prefix("At Bond ")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0.0);
        let plain = u.condition.is_empty() || u.condition.starts_with("At Bond ");
        if plain {
            for ne in &u.effects {
                let value = ne
                    .value
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.')
                    .collect::<String>()
                    .parse::<f64>()
                    .unwrap_or(0.0);
                if let Some(id) = names.get(&ne.name) {
                    s.unique.push((*id, value));
                }
            }
        }
    }
    for ev in &info.events {
        let g = event_gain(&ev.options);
        if g.total() <= 0.0 {
            continue;
        }
        let kind = match ev.kind.as_str() {
            "Chain Event" => EventKind::Chain,
            "Date Event" => EventKind::Date,
            _ => EventKind::Random,
        };
        s.events.push((kind, g));
    }
    s
}

impl CardStats {
    /// An effect's value including the unique effect once its bond is reached.
    fn get(&self, id: u32, base: f64, bond: f64) -> f64 {
        base + if bond >= self.unique_bond {
            self.unique
                .iter()
                .filter(|(k, _)| *k == id)
                .map(|(_, v)| v)
                .sum()
        } else {
            0.0
        }
    }
}

/// Turn index (0..72) of "junior year, late june" style dates.
pub fn turn_of(when: &str) -> Option<usize> {
    let w = when.to_ascii_lowercase();
    let year = if w.contains("junior") {
        0
    } else if w.contains("classic") {
        1
    } else if w.contains("senior") {
        2
    } else {
        return None;
    };
    const MONTHS: [&str; 12] = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    let month = MONTHS.iter().position(|m| w.contains(m))?;
    Some(year * 24 + month * 2 + usize::from(w.contains("late")))
}

pub struct CareerInput<'a> {
    pub common: &'a Common,
    pub scenario: &'a Scenario,
    pub growth: [f64; 5],
    pub base_stats: [f64; 5],
    pub deck: Vec<CardStats>,
    /// How much each stat is wanted (the plan's target stats, normalised).
    pub weights: [f64; 5],
    /// The plan's target stats (0 = no target): training aims to fill the gaps to these first.
    pub target: [f64; 5],
    /// (turn, race grade) of the trainee's objective races.
    pub objectives: Vec<(usize, String)>,
    /// What each of the trainee's own events gives (best option).
    pub uma_events: Vec<Gain>,
    pub runs: usize,
    pub seed: f64,
}

#[derive(Clone, Debug, Default)]
pub struct CareerResult {
    pub sp_mean: f64,
    pub sp_p10: f64,
    pub sp_p90: f64,
    /// Mean SP by source: training, races, events, scenario.
    pub sp_sources: Vec<(String, f64)>,
    pub stats_mean: [f64; 5],
    /// Expected hint levels per skill from the deck's training hints.
    pub hint_levels: HashMap<u32, f64>,
    pub rainbows_mean: f64,
    pub trainings_mean: f64,
    pub races_mean: f64,
    pub runs: usize,
}

fn race_sp(sc: &Scenario, grade: &str) -> f64 {
    let g = if grade.starts_with("G1") {
        "G1"
    } else if grade.starts_with("G2") {
        "G2"
    } else if grade.starts_with("G3") {
        "G3"
    } else {
        "OP"
    };
    sc.race_sp.get(g).copied().unwrap_or(30.0)
}

fn race_stats(grade: &str) -> f64 {
    // Game8: a G1 win gives +10 to a random stat, G2/G3 +8, OP/Pre-OP +5.
    if grade.starts_with("G1") {
        10.0
    } else if grade.starts_with("G2") || grade.starts_with("G3") {
        8.0
    } else {
        5.0
    }
}

/// The worth of moving a stat from `from` by `delta` when `target` is wanted: the part below the
/// target counts fully, the part above it counts `beyond` (losses likewise).
pub fn need_value(from: f64, delta: f64, target: f64, beyond: f64) -> f64 {
    let to = from + delta;
    let below = |x: f64| x.min(target);
    let above = |x: f64| (x - target).max(0.0);
    (below(to) - below(from)) + beyond * (above(to) - above(from))
}

/// Simulate the career `runs` times and average.
pub fn simulate(input: &CareerInput) -> CareerResult {
    let c = input.common;
    let a = &c.assumed;
    let sc = input.scenario;
    let mut rng = Rng::new(input.seed);
    let runs = input.runs.max(1);
    let mut sps = Vec::with_capacity(runs);
    let mut sources = [0.0f64; 4];
    let mut stats_sum = [0.0f64; 5];
    let mut hints: HashMap<u32, f64> = HashMap::new();
    let (mut rainbow_sum, mut train_sum, mut race_sum) = (0.0, 0.0, 0.0);
    let deck = &input.deck;
    let race_bonus: f64 = deck.iter().map(|d| d.race_bonus).sum();
    let event_mult = sc.event_chance_multiplier.unwrap_or(1.0);
    let linked = deck.iter().filter(|d| d.linked).count() as f64;
    let total_weight: f64 = input.weights.iter().sum::<f64>().max(1e-9);
    let weights = input.weights.map(|w| w / total_weight * 5.0);
    let optional = sc.optional_races.unwrap_or(a.optional_races) as usize;
    let target_total: f64 = input.target.iter().sum();

    for _ in 0..runs {
        let mut stats = input.base_stats;
        for d in deck {
            for i in 0..5 {
                stats[i] += d.init_stats[i];
            }
        }
        let mut sp = 0.0;
        let mut src = [0.0f64; 4];
        let mut energy = 100.0;
        let mut bond: Vec<f64> = deck.iter().map(|d| d.init_bond).collect();
        let mut used = [0u32; 5];
        let mut hint_lv: HashMap<u32, f64> = HashMap::new();
        let (mut rainbows, mut trainings, mut races) = (0.0, 0.0, 0.0);
        // Optional races on random free turns in years 2 and 3, outside summer camp.
        let mut race_turns: Vec<(usize, String)> = input.objectives.clone();
        let mut free: Vec<usize> = (24..c.turns)
            .filter(|t| !c.summer_camp_turns.contains(t) && !race_turns.iter().any(|(r, _)| r == t))
            .collect();
        for _ in 0..optional.min(free.len()) {
            let k = (rng.uniform(free.len() as f64) as usize).min(free.len() - 1);
            let grade = if sc.optional_races.is_some() && rng.random() < 0.5 {
                "G2"
            } else {
                "G1"
            };
            race_turns.push((free.remove(k), grade.to_string()));
        }
        let race = |grade: &str,
                    objective: bool,
                    sp: &mut f64,
                    src: &mut [f64; 4],
                    stats: &mut [f64; 5],
                    rng: &mut Rng| {
            let win = !objective || rng.random() < a.objective_win_rate;
            let factor =
                if win { 1.0 } else { a.placed_reward_factor } * (1.0 + race_bonus / 100.0);
            let gain = race_sp(sc, grade) * factor;
            *sp += gain;
            src[1] += gain;
            for s in stats.iter_mut() {
                *s += race_stats(grade) * factor / 5.0;
            }
        };
        for t in 0..c.turns {
            energy = (energy + a.event_energy_per_turn).min(100.0);
            if let Some((_, grade)) = race_turns.iter().find(|(r, _)| *r == t) {
                let objective = input.objectives.iter().any(|(r, _)| *r == t);
                race(grade, objective, &mut sp, &mut src, &mut stats, &mut rng);
                energy = (energy - a.race_energy).max(0.0);
                races += 1.0;
                continue;
            }
            // Deal cards to facilities (or nowhere): 100 per facility, 50 absent, specialty added to its own.
            let mut on: [Vec<usize>; 5] = Default::default();
            for (ci, d) in deck.iter().enumerate() {
                let spec = d.get(effect::SPECIALTY, d.specialty, bond[ci]);
                let mut weights_f = [c.appearance_weight; 5];
                if let Some(f) = d.facility {
                    weights_f[f] += spec;
                }
                let total = weights_f.iter().sum::<f64>() + c.absent_weight;
                let mut roll = rng.random() * total;
                for (f, w) in weights_f.iter().enumerate() {
                    if roll < *w {
                        on[f].push(ci);
                        break;
                    }
                    roll -= w;
                }
            }
            let summer = c.summer_camp_turns.contains(&t);
            let gains = |f: usize| -> ([f64; 6], f64, usize) {
                let level = if summer {
                    a.summer_camp_level
                } else {
                    (used[f] / c.trainings_per_level) as usize + 1
                }
                .clamp(1, 5);
                let lm = c.level_multiplier.get(level - 1).copied().unwrap_or(1.0);
                let base = sc.base_training[f];
                let cards = &on[f];
                let n = cards.len() as f64;
                let mood_effect: f64 = cards
                    .iter()
                    .map(|&ci| deck[ci].get(effect::MOOD, deck[ci].mood, bond[ci]))
                    .sum();
                let train: f64 = cards
                    .iter()
                    .map(|&ci| deck[ci].get(effect::TRAINING, deck[ci].training, bond[ci]))
                    .sum();
                let mut friend = 1.0;
                let mut rb = 0;
                for &ci in cards {
                    if deck[ci].facility == Some(f) && bond[ci] >= c.rainbow_bond {
                        friend *= 1.0
                            + deck[ci].get(effect::FRIENDSHIP, deck[ci].friendship, bond[ci])
                                / 100.0;
                        rb += 1;
                    }
                }
                let mult = (1.0 + a.mood * c.mood_step * (1.0 + mood_effect / 100.0))
                    * (1.0 + train / 100.0)
                    * friend
                    * (1.0 + c.support_bonus * n);
                let mut out = [0.0; 6];
                for i in 0..5 {
                    if base[i] > 0.0 {
                        let bonus: f64 = cards
                            .iter()
                            .map(|&ci| {
                                deck[ci].get(
                                    effect::SPEED + i as u32,
                                    deck[ci].stat_bonus[i],
                                    bond[ci],
                                )
                            })
                            .sum();
                        out[i] = (base[i] * lm + bonus) * mult * (1.0 + input.growth[i] / 100.0);
                    }
                }
                let sp_bonus: f64 = cards
                    .iter()
                    .map(|&ci| deck[ci].get(effect::SKILL_POINTS, deck[ci].sp_bonus, bond[ci]))
                    .sum();
                let extra: f64 = sc
                    .sp_per_training_from
                    .iter()
                    .filter(|(from, _)| t >= *from)
                    .map(|(_, v)| v)
                    .sum();
                out[5] = (base[5] * lm + sp_bonus) * mult + extra;
                let cut: f64 = cards.iter().map(|&ci| deck[ci].energy_cut).sum();
                let mut energy = if base[6] < 0.0 {
                    base[6] * (1.0 - cut / 100.0)
                } else {
                    base[6]
                };
                if f == 4 && rb > 0 {
                    energy += cards.iter().map(|&ci| deck[ci].wit_recovery).sum::<f64>();
                }
                (out, energy, rb)
            };
            // Train toward the target's shape: each stat's share of the total so far, scaled from the
            // plan's target line. Gains that fill a gap count fully, the rest a quarter.
            let scale = stats.iter().sum::<f64>() / target_total.max(1.0);
            let value = |g: &[f64; 6]| -> f64 {
                (0..5)
                    .map(|i| {
                        let gain = g[i].min((sc.stat_caps[i] - stats[i]).max(0.0));
                        weights[i]
                            * if input.target[i] > 0.0 {
                                need_value(stats[i], gain, input.target[i] * scale, 0.25)
                            } else {
                                gain
                            }
                    })
                    .sum::<f64>()
                    + 0.5 * g[5]
            };
            let mut best = (0usize, f64::MIN);
            for f in 0..5 {
                let (g, _, rb) = gains(f);
                // Early on, training with cards that still need bond is worth a bit extra.
                let bonding = if t < 48 {
                    on[f]
                        .iter()
                        .filter(|&&ci| bond[ci] < c.rainbow_bond)
                        .count() as f64
                        * 3.0
                } else {
                    0.0
                };
                let v = value(&g) + bonding + rb as f64 * 5.0;
                if v > best.1 {
                    best = (f, v);
                }
            }
            if energy < a.rest_below_energy && best.0 != 4 {
                // Wit training restores energy; otherwise rest.
                let (gw, _, _) = gains(4);
                if value(&gw) >= best.1 * 0.6 {
                    best.0 = 4;
                } else {
                    energy = (energy + a.rest_energy).min(100.0);
                    continue;
                }
            }
            let f = best.0;
            let (g, de, rb) = gains(f);
            for i in 0..5 {
                stats[i] = (stats[i] + g[i]).min(sc.stat_caps[i]);
            }
            sp += g[5];
            src[0] += g[5];
            energy = (energy + de).clamp(0.0, 100.0);
            used[f] += 1;
            trainings += 1.0;
            rainbows += rb as f64;
            for &ci in &on[f] {
                bond[ci] = (bond[ci] + c.bond_per_training).min(100.0);
                let chance = a.hint_chance * (1.0 + deck[ci].hint_freq / 100.0);
                if !deck[ci].hints.is_empty() && rng.random() < chance {
                    bond[ci] = (bond[ci] + c.bond_per_hint).min(100.0);
                    let k = (rng.uniform(deck[ci].hints.len() as f64) as usize)
                        .min(deck[ci].hints.len() - 1);
                    let lv = hint_lv.entry(deck[ci].hints[k]).or_default();
                    *lv = (*lv + 1.0 + deck[ci].hint_levels).min(5.0);
                }
            }
        }
        for _ in 0..sc.finale_races {
            race("G1", true, &mut sp, &mut src, &mut stats, &mut rng);
            races += 1.0;
        }
        // Events: each triggers with its assumed chance, worth its best option.
        let apply = |g: &Gain, sp: &mut f64, src: &mut [f64; 4], stats: &mut [f64; 5]| {
            *sp += g.sp;
            src[2] += g.sp;
            for i in 0..5 {
                stats[i] = (stats[i] + g.stats[i]).min(sc.stat_caps[i]);
            }
        };
        for d in deck {
            for (kind, g) in &d.events {
                let chance = match kind {
                    EventKind::Chain => a.chain_event_chance,
                    EventKind::Random => a.random_event_chance * event_mult,
                    EventKind::Date => a.date_event_chance,
                };
                if rng.random() < chance.min(1.0) {
                    apply(g, &mut sp, &mut src, &mut stats);
                }
            }
        }
        for g in &input.uma_events {
            if rng.random() < a.uma_event_chance {
                apply(g, &mut sp, &mut src, &mut stats);
            }
        }
        // Team events (Unity Cup bursts) spread over the deck and about four story characters.
        let link_share = linked / (deck.len() as f64 + 4.0);
        for e in &sc.extra_sp {
            let mut v = e.sp + e.link_extra * e.per * link_share;
            if e.race_bonus {
                v *= 1.0 + race_bonus / 100.0;
            }
            sp += v;
            src[3] += v;
        }
        let lb = sc.link_bonus.sp * linked;
        sp += lb;
        src[3] += lb;
        for s in &mut stats {
            *s += sc.link_bonus.stats * linked / 5.0;
        }
        sps.push(sp);
        for i in 0..4 {
            sources[i] += src[i];
        }
        for i in 0..5 {
            stats_sum[i] += stats[i];
        }
        for (k, v) in hint_lv {
            *hints.entry(k).or_default() += v;
        }
        rainbow_sum += rainbows;
        train_sum += trainings;
        race_sum += races;
    }
    sps.sort_by(f64::total_cmp);
    let n = runs as f64;
    let pct = |p: f64| sps[((sps.len() - 1) as f64 * p).round() as usize];
    CareerResult {
        sp_mean: sps.iter().sum::<f64>() / n,
        sp_p10: pct(0.1),
        sp_p90: pct(0.9),
        sp_sources: [t!("Training"), t!("Races"), t!("Events"), t!("Scenario")]
            .into_iter()
            .zip(sources)
            .map(|(k, v)| (k.to_string(), v / n))
            .collect(),
        stats_mean: stats_sum.map(|s| s / n),
        hint_levels: hints.into_iter().map(|(k, v)| (k, v / n)).collect(),
        rainbows_mean: rainbow_sum / n,
        trainings_mean: train_sum / n,
        races_mean: race_sum / n,
        runs,
    }
}

/// The trainee's objective races as (turn, grade) and what her events give.
pub fn trainee_inputs(kb: &Knowledge, uma: &UmaInfo) -> (Vec<(usize, String)>, Vec<Gain>) {
    let objectives = uma
        .objectives
        .iter()
        .filter_map(|o| {
            let turn = turn_of(&o.when)?;
            let grade = o
                .race_id
                .and_then(|id| kb.races.iter().find(|r| r.id == id))
                .map_or("OP".to_string(), |r| r.grade.clone());
            Some((turn, grade))
        })
        .collect();
    let events = uma
        .events
        .iter()
        .filter(|e| matches!(e.kind.as_str(), "Choice Event" | "No-Choice Event"))
        .map(|e| {
            e.options
                .iter()
                .map(|o| {
                    o.outcomes
                        .iter()
                        .map(|l| gain_in(l))
                        .fold(Gain::default(), |mut a, g| {
                            a.sp += g.sp;
                            for i in 0..5 {
                                a.stats[i] += g.stats[i];
                            }
                            a
                        })
                })
                .fold(Gain::default(), |best, g| {
                    if g.total() > best.total() { g } else { best }
                })
        })
        .filter(|g| g.total() > 0.0)
        .collect();
    (objectives, events)
}

/// Skill cost after hints: 10% off per level for three levels, then 5% (uma.guide's skills guide).
pub fn hint_discount(level: f64) -> f64 {
    let l = level.clamp(0.0, 5.0);
    if l <= 3.0 {
        0.10 * l
    } else {
        0.30 + 0.05 * (l - 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_event_text() {
        assert_eq!(sp_in("Skill points +20"), Some(20.0));
        assert_eq!(sp_in("Skill Points +10/+20"), Some(15.0));
        assert_eq!(sp_in("Speed +10"), None);
        assert_eq!(turn_of("junior year, late june"), Some(11));
        assert_eq!(turn_of("senior year, early december"), Some(70));
        assert!(
            (hint_discount(4.0) - 0.35).abs() < 1e-9 && (hint_discount(1.0) - 0.1).abs() < 1e-9
        );
    }
}
