//! Skill auto-pick that optimises the set, not single skills: screen every candidate in realistic
//! races, choose greedily by each candidate's measured marginal gain on top of what's already
//! chosen (re-simulated every step, on common random numbers), try 1-for-1 swaps, then verify the
//! final set in 1000 races. Runs on a background thread; the planner builds the input
//! (`App::skill_opt_input`) and shows the result.
//!
//! Everything here is simulator output (Moomoolator's engine), not game data.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::sim::course::Course;
use crate::sim::data::SimData;
use crate::sim::params::RaceParams;
use crate::sim::run::{
    Progress, SimOptions, UmaBuild, realistic_options, run_chart, run_comparison,
};

/// How much the community meta sheet's skill advice counts against the simulation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub(crate) enum MetaInfluence {
    /// Simulation only.
    Off,
    /// +0.05 L per unit of meta weight: enough to order near-equals.
    #[default]
    TieBreak,
    /// The sheet's picks first (2 L per unit, the old behaviour).
    Strong,
}

impl MetaInfluence {
    pub const ALL: [MetaInfluence; 3] = [
        MetaInfluence::Off,
        MetaInfluence::TieBreak,
        MetaInfluence::Strong,
    ];

    /// Lengths added per unit of meta weight.
    pub fn lengths(self) -> f64 {
        match self {
            MetaInfluence::Off => 0.0,
            MetaInfluence::TieBreak => 0.05,
            MetaInfluence::Strong => crate::meta::META_SKILL_LENGTHS,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MetaInfluence::Off => "Off",
            MetaInfluence::TieBreak => "Tie-break",
            MetaInfluence::Strong => "Strong",
        }
    }

    pub fn explain(self) -> &'static str {
        match self {
            MetaInfluence::Off => "Simulation only: the meta sheet's skill advice doesn't count.",
            MetaInfluence::TieBreak => {
                "The meta sheet's advice only decides between skills the simulation finds about equal (+0.05 L per unit of backing)."
            }
            MetaInfluence::Strong => {
                "The meta sheet's skills first, as long as they can trigger on the course (+2 L per unit of backing)."
            }
        }
    }
}

/// What a pick aims for.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub(crate) enum PickObjective {
    /// The most lengths in the CM race.
    #[default]
    Lengths,
    /// Team Trials: gains weighted by how often the skill triggers; cheap, reliable skills.
    Consistency,
    /// Rating or parent value: the most evaluation points within the budget, no racing.
    Rating,
}

impl PickObjective {
    pub const ALL: [PickObjective; 3] = [
        PickObjective::Lengths,
        PickObjective::Consistency,
        PickObjective::Rating,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PickObjective::Lengths => "Win the CM race (lengths)",
            PickObjective::Consistency => "Team Trials (consistency)",
            PickObjective::Rating => "Rating / parent (points)",
        }
    }

    pub fn explain(self) -> &'static str {
        match self {
            PickObjective::Lengths => {
                "The set that gains the most lengths on average in the plan's race."
            }
            PickObjective::Consistency => {
                "Each skill's gain is weighted by how often it triggers, so cheap, reliable skills win."
            }
            PickObjective::Rating => {
                "The most skill evaluation points within the SP budget, for the uma's rank: an SS rank parent makes better sparks (uma.guide, Parenting). Points come from uma.guide's skill data. The game also scales skills tied to a distance or style by the uma's aptitude, which isn't modelled. No racing."
            }
        }
    }
}

/// One skill the optimiser may buy.
#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    pub id: u32,
    /// Its own price after hint discounts (and Fast Learner).
    pub own: f64,
    /// The ○/white it needs first, and that one's price, unless it's already held.
    pub lower: Option<(u32, f64)>,
    /// Meta sheet backing (0 when the sheet doesn't mention it).
    pub meta: f64,
    pub pinned: bool,
}

/// Everything a run needs, copied so the plan can change while it runs.
#[derive(Clone)]
pub(crate) struct Input {
    pub data: Arc<SimData>,
    pub course: Arc<Course>,
    pub race: RaceParams,
    /// The trainee with what she has for free (her unique).
    pub base: UmaBuild,
    pub candidates: Vec<Candidate>,
    pub budget: f64,
    pub meta: MetaInfluence,
    pub objective: PickObjective,
    pub seed: f64,
    pub tune: Tune,
}

/// Run sizes. `Tune::default()` is the app's; tests use smaller ones.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tune {
    /// How many candidates survive the screen, by gain and by gain per SP (each).
    pub keep: usize,
    /// Races per marginal evaluation.
    pub step_runs: usize,
    pub verify_runs: usize,
    /// Races per drop-one ("gain in this set") evaluation.
    pub drop_runs: usize,
    /// Under this much SP left, the best absolute gain that fits beats the best gain per SP.
    pub endgame_sp: f64,
    /// A step must gain more than this (lengths) to add a skill.
    pub min_gain: f64,
    pub swap_time: Duration,
    /// Unchosen skills tried in swaps.
    pub swap_pool: usize,
}

impl Tune {
    /// Small runs, for tests that drive the whole app.
    pub fn quick() -> Self {
        Tune {
            keep: 8,
            step_runs: 40,
            verify_runs: 200,
            drop_runs: 40,
            swap_time: Duration::from_secs(2),
            swap_pool: 4,
            ..Tune::default()
        }
    }
}

impl Default for Tune {
    fn default() -> Self {
        Tune {
            keep: 25,
            step_runs: 500,
            verify_runs: 1000,
            drop_runs: 300,
            endgame_sp: 250.0,
            min_gain: 0.02,
            swap_time: Duration::from_secs(15),
            swap_pool: 12,
        }
    }
}

/// The verify run: the final set against the trainee with only her unique.
#[derive(Clone, Debug, Default)]
pub(crate) struct Verify {
    pub mean: f64,
    pub lo: f64,
    pub hi: f64,
    pub runs: usize,
    /// The chosen skills' single gains (the screen), added up: what the set would gain if gains added.
    pub singles_sum: f64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Outcome {
    /// Skills to buy, in plan form (a gold stands for itself and its white).
    pub chosen: Vec<u32>,
    /// Gain alone: each candidate's screen gain (realistic races), lengths.
    pub singles: HashMap<u32, f64>,
    /// Gain in this set: the final set minus each chosen skill.
    pub marginal: HashMap<u32, f64>,
    /// How often each chosen skill triggered in the verify run, 0–1.
    pub activation: HashMap<u32, f64>,
    /// The step each chosen skill was added at, with its marginal gain then.
    pub added: HashMap<u32, (usize, f64)>,
    /// What happened, in order, for the Explain panel.
    pub log: Vec<String>,
    pub verify: Option<Verify>,
    pub spent: f64,
    pub seconds: f64,
    pub cancelled: bool,
}

/// What the run is doing, for the progress bar's label.
#[derive(Default)]
pub(crate) struct Phase(Mutex<String>);

impl Phase {
    pub fn set(&self, s: impl Into<String>) {
        if let Ok(mut p) = self.0.lock() {
            *p = s.into();
        }
    }

    pub fn get(&self) -> String {
        self.0.lock().map(|p| p.clone()).unwrap_or_default()
    }
}

/// A set being built: plan-form ids. A gold replaces its white when both would be in.
#[derive(Clone, Debug, Default)]
pub(crate) struct Set {
    pub ids: Vec<u32>,
}

/// One per group: a gold and its white share `id / 10`.
fn group(id: u32) -> u32 {
    id / 10
}

impl Set {
    /// Can `c` join, and what would it cost on top? A gold whose white is in costs only its own
    /// price (it replaces the white); otherwise a gold costs its white too. Nothing else may share
    /// its group.
    pub fn add_cost(&self, c: &Candidate) -> Option<f64> {
        let mut cost = c.own;
        let mut white_in = false;
        for &s in &self.ids {
            if group(s) != group(c.id) {
                continue;
            }
            match c.lower {
                Some((l, _)) if s == l => white_in = true,
                _ => return None,
            }
        }
        if !white_in && let Some((_, lp)) = c.lower {
            cost += lp;
        }
        Some(cost)
    }

    pub fn add(&mut self, c: &Candidate) {
        if let Some((l, _)) = c.lower {
            self.ids.retain(|s| *s != l);
        }
        self.ids.push(c.id);
    }

    /// What the set costs in all.
    pub fn cost(&self, by_id: &HashMap<u32, &Candidate>) -> f64 {
        self.ids
            .iter()
            .filter_map(|id| by_id.get(id))
            .map(|c| c.own + c.lower.map_or(0.0, |(_, p)| p))
            .sum()
    }

    fn build(&self, base: &UmaBuild, data: &SimData) -> UmaBuild {
        let mut u = base.clone();
        u.skills.extend(
            self.ids
                .iter()
                .copied()
                .filter(|id| data.skills.contains_key(id)),
        );
        u.normalise(data);
        u
    }
}

/// Mean gap of `b` over `a`, and how often each of `b`'s skills triggered (0–1).
fn race(
    input: &Input,
    a: &UmaBuild,
    b: &UmaBuild,
    runs: usize,
    seed: f64,
) -> (f64, Vec<f64>, HashMap<u32, f64>) {
    let opts: SimOptions = realistic_options(seed, runs);
    let r = run_comparison(
        &input.data,
        &input.course,
        &input.race,
        a,
        b,
        &opts,
        false,
        false,
        None,
    );
    let rates = r.umas[1]
        .skills
        .iter()
        .map(|s| (s.id, s.rate / 100.0))
        .collect();
    (r.mean, r.gaps, rates)
}

/// Evaluate many builds against one in parallel: each item's (mean gap, trigger rates).
fn race_many(
    input: &Input,
    a: &UmaBuild,
    items: &[UmaBuild],
    runs: usize,
    seed: f64,
    progress: &Progress,
) -> Vec<Option<(f64, HashMap<u32, f64>)>> {
    if items.is_empty() {
        return Vec::new();
    }
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 16);
    let chunk = items.len().div_ceil(threads);
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|b| {
                            if progress.cancel.load(Ordering::Relaxed) {
                                return None;
                            }
                            let (mean, _, rates) = race(input, a, b, runs, seed);
                            progress.done.fetch_add(1, Ordering::Relaxed);
                            Some((mean, rates))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    })
}

/// A step's score for a candidate: its measured marginal gain, as the objective sees it, plus the
/// meta sheet's backing at the chosen influence.
fn score(input: &Input, c: &Candidate, gain: f64, rate: f64) -> f64 {
    let g = match input.objective {
        PickObjective::Consistency => gain * rate,
        _ => gain,
    };
    g + c.meta * input.meta.lengths()
}

/// Lengths with a sign, never "-0.00".
fn lengths(x: f64) -> String {
    let r = (x * 100.0).round() / 100.0;
    format!("{:+.2} L", if r == 0.0 { 0.0 } else { r })
}

fn name(input: &Input, id: u32, names: &BTreeMap<u32, String>) -> String {
    let _ = input;
    names
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("skill {id}"))
}

/// Run the optimiser. `names` labels the log.
pub(crate) fn optimise(
    input: &Input,
    names: &BTreeMap<u32, String>,
    progress: &Progress,
    phase: &Phase,
) -> Outcome {
    let start = Instant::now();
    let t = input.tune;
    let data = &input.data;
    let by_id: HashMap<u32, &Candidate> = input.candidates.iter().map(|c| (c.id, c)).collect();
    let mut out = Outcome::default();
    let cancelled = || progress.cancel.load(Ordering::Relaxed);
    let set_total = |n: usize| {
        progress.done.store(0, Ordering::Relaxed);
        progress.total.store(n.max(1), Ordering::Relaxed);
    };

    // 1. Screen: every simulatable candidate alone, in realistic races.
    let sim_ids: Vec<u32> = input
        .candidates
        .iter()
        .map(|c| c.id)
        .filter(|id| data.skills.contains_key(id))
        .collect();
    phase.set(tf!(
        "Screening {} skills in realistic races…",
        sim_ids.len()
    ));
    let rows = run_chart(
        data,
        &input.course,
        &input.race,
        &input.base,
        &sim_ids,
        input.seed,
        true,
        progress,
    );
    out.singles = rows.iter().map(|r| (r.id, r.mean)).collect();
    if cancelled() {
        out.cancelled = true;
        out.seconds = start.elapsed().as_secs_f64();
        return out;
    }
    let single = |id: u32| out.singles.get(&id).copied().unwrap_or(0.0);
    let full_cost = |c: &Candidate| c.own + c.lower.map_or(0.0, |(_, p)| p);
    let mut by_gain: Vec<&Candidate> = input
        .candidates
        .iter()
        .filter(|c| single(c.id) > 0.0)
        .collect();
    by_gain.sort_by(|a, b| single(b.id).total_cmp(&single(a.id)));
    let mut by_eff = by_gain.clone();
    by_eff.sort_by(|a, b| {
        (single(b.id) / full_cost(b).max(1.0)).total_cmp(&(single(a.id) / full_cost(a).max(1.0)))
    });
    let mut pool: Vec<u32> = Vec::new();
    for c in by_gain
        .iter()
        .take(t.keep)
        .chain(by_eff.iter().take(t.keep))
    {
        if !pool.contains(&c.id) {
            pool.push(c.id);
        }
    }
    for c in input
        .candidates
        .iter()
        .filter(|c| c.pinned || (c.meta > 0.0 && input.meta != MetaInfluence::Off))
    {
        if !pool.contains(&c.id) && (c.pinned || data.skills.contains_key(&c.id)) {
            pool.push(c.id);
        }
    }
    // A gold's white can be bought alone too.
    let whites: Vec<u32> = pool
        .iter()
        .filter_map(|id| by_id.get(id).and_then(|c| c.lower))
        .map(|(l, _)| l)
        .filter(|l| by_id.contains_key(l) && !pool.contains(l))
        .collect();
    pool.extend(whites);
    out.log.push(tf!(
        "Screened {} skills alone in realistic races (wit checks and rushing on); kept {} by gain and by gain per SP, plus every pinned and meta-backed one.",
        sim_ids.len(),
        pool.len()
    ));

    // 2. Greedy: the pinned skills, then each step the best measured marginal gain (per SP, until
    // the budget is nearly spent).
    let mut set = Set::default();
    for c in input.candidates.iter().filter(|c| c.pinned) {
        if set.add_cost(c).is_some() {
            set.add(c);
            out.log
                .push(tf!("📌 {} (pinned)", name(input, c.id, names)));
        }
    }
    let mut step = 0;
    loop {
        if cancelled() {
            out.cancelled = true;
            break;
        }
        let spent = set.cost(&by_id);
        let left = input.budget - spent;
        let options: Vec<(&Candidate, f64)> = pool
            .iter()
            .filter(|id| !set.ids.contains(id))
            .filter_map(|id| by_id.get(id).copied())
            .filter_map(|c| {
                set.add_cost(c)
                    .filter(|cost| *cost <= left + 0.5)
                    .map(|cost| (c, cost))
            })
            .filter(|(c, _)| data.skills.contains_key(&c.id))
            .collect();
        if options.is_empty() {
            out.log.push(tf!(
                "Stopped: nothing else fits the {} SP left.",
                format!("{left:.0}")
            ));
            break;
        }
        step += 1;
        phase.set(tf!(
            "Choosing skill {}: racing {} options on top of {} chosen…",
            step,
            options.len(),
            set.ids.len()
        ));
        set_total(options.len());
        let b = set.build(&input.base, data);
        let builds: Vec<UmaBuild> = options
            .iter()
            .map(|(c, _)| crate::sim::run::with_skill(data, &b, c.id))
            .collect();
        let results = race_many(
            input,
            &b,
            &builds,
            t.step_runs,
            input.seed + step as f64,
            progress,
        );
        let endgame = left < t.endgame_sp;
        let mut best: Option<(usize, f64, f64, f64)> = None;
        for (i, r) in results.iter().enumerate() {
            let Some((gain, rates)) = r else { continue };
            let (c, cost) = options[i];
            let rate = rates.get(&c.id).copied().unwrap_or(0.0);
            let s = score(input, c, *gain, rate);
            let key = if endgame { s } else { s / cost.max(1.0) };
            if best.is_none_or(|(_, k, _, _)| key > k) {
                best = Some((i, key, s, *gain));
            }
        }
        let Some((i, _, s, gain)) = best else { break };
        let (c, cost) = options[i];
        if s <= t.min_gain {
            out.log.push(tf!(
                "Stopped: the best next skill, {}, adds only {}.",
                name(input, c.id, names),
                lengths(gain)
            ));
            break;
        }
        let upgrade = c
            .lower
            .filter(|(l, _)| set.ids.contains(l))
            .map_or(String::new(), |(l, _)| {
                tf!(", upgrading {}", name(input, l, names))
            });
        set.add(c);
        out.added.insert(c.id, (step, gain));
        let meta = if c.meta > 0.0 && input.meta != MetaInfluence::Off {
            tf!(
                " (meta +{})",
                format!("{:.2}", c.meta * input.meta.lengths())
            )
        } else {
            String::new()
        };
        out.log.push(tf!(
            "{}. {}: {} on top of the set{}, for {} SP{},{} — {} SP left.",
            step,
            name(input, c.id, names),
            lengths(gain),
            meta,
            format!("{cost:.0}"),
            upgrade,
            if endgame { t!(", best that fits") } else { "" },
            format!("{:.0}", left - cost)
        ));
    }

    // 3. Swaps: drop one chosen skill and add one that isn't, within budget, if that gains.
    let mut unchosen: Vec<&Candidate> = pool
        .iter()
        .filter_map(|id| by_id.get(id).copied())
        .filter(|c| data.skills.contains_key(&c.id))
        .collect();
    unchosen.sort_by(|a, b| {
        (single(b.id) / full_cost(b).max(1.0)).total_cmp(&(single(a.id) / full_cost(a).max(1.0)))
    });
    let swap_start = Instant::now();
    let mut round = 0;
    while !out.cancelled && swap_start.elapsed() < t.swap_time && !set.ids.is_empty() {
        round += 1;
        let current = set.build(&input.base, data);
        let mut trials: Vec<(u32, u32, Set)> = Vec::new();
        for &drop in set
            .ids
            .iter()
            .filter(|id| by_id.get(id).is_some_and(|c| !c.pinned))
        {
            let mut without = set.clone();
            without.ids.retain(|s| *s != drop);
            for c in unchosen
                .iter()
                .filter(|c| !set.ids.contains(&c.id) && c.id != drop)
                .take(t.swap_pool)
            {
                let Some(cost) = without.add_cost(c) else {
                    continue;
                };
                if without.cost(&by_id) + cost > input.budget + 0.5 {
                    continue;
                }
                let mut next = without.clone();
                next.add(c);
                trials.push((drop, c.id, next));
            }
        }
        if trials.is_empty() {
            break;
        }
        phase.set(tf!("Trying {} swaps (round {})…", trials.len(), round));
        set_total(trials.len());
        let builds: Vec<UmaBuild> = trials
            .iter()
            .map(|(_, _, s)| s.build(&input.base, data))
            .collect();
        let results = race_many(
            input,
            &current,
            &builds,
            t.step_runs,
            input.seed + 100.0 + f64::from(round),
            progress,
        );
        if cancelled() {
            out.cancelled = true;
            break;
        }
        let best = results
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.as_ref().map(|(g, _)| (i, *g)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((i, gain)) if gain > t.min_gain * 1.5 => {
                let (drop, add, next) = trials.swap_remove(i);
                out.log.push(tf!(
                    "Swap: {} out, {} in: {}.",
                    name(input, drop, names),
                    name(input, add, names),
                    lengths(gain)
                ));
                out.added.remove(&drop);
                out.added.insert(add, (0, gain));
                set = next;
            }
            _ => {
                out.log
                    .push(t!("No swap of one chosen skill for another does better.").to_string());
                break;
            }
        }
    }

    // 4. Verify: the final set against her unique only, and each skill's gain within the set.
    if !out.cancelled && !set.ids.is_empty() {
        phase.set(tf!("Checking the set in {} races…", t.verify_runs));
        set_total(1 + set.ids.len());
        let fin = set.build(&input.base, data);
        let (mean, gaps, rates) = race(input, &input.base, &fin, t.verify_runs, input.seed + 999.0);
        progress.done.fetch_add(1, Ordering::Relaxed);
        let n = gaps.len().max(1) as f64;
        let sd = (gaps.iter().map(|g| (g - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0)).sqrt();
        let half = 1.96 * sd / n.sqrt();
        let singles_sum = set.ids.iter().map(|id| single(*id)).sum();
        out.verify = Some(Verify {
            mean,
            lo: mean - half,
            hi: mean + half,
            runs: gaps.len(),
            singles_sum,
        });
        out.activation = set
            .ids
            .iter()
            .map(|id| (*id, rates.get(id).copied().unwrap_or(0.0)))
            .collect();
        let without: Vec<UmaBuild> = set
            .ids
            .iter()
            .map(|drop| {
                let mut s = set.clone();
                s.ids.retain(|x| x != drop);
                s.build(&input.base, data)
            })
            .collect();
        // Each skill's gain in the set: the full set against the set without it.
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .clamp(1, 16);
        let chunk = without.len().div_ceil(threads).max(1);
        let gains: Vec<f64> = std::thread::scope(|scope| {
            let handles: Vec<_> = without
                .chunks(chunk)
                .map(|part| {
                    let fin = &fin;
                    scope.spawn(move || {
                        part.iter()
                            .map(|w| {
                                let (g, _, _) =
                                    race(input, w, fin, t.drop_runs, input.seed + 555.0);
                                progress.done.fetch_add(1, Ordering::Relaxed);
                                g
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap_or_default())
                .collect()
        });
        out.marginal = set.ids.iter().copied().zip(gains).collect();
        out.log.push(tf!(
            "Checked in {} races: {} L (95%: {} to {}) against her unique alone. Their single gains add up to {} L: skills overlap and share the same stretches of the race.",
            gaps.len(),
            format!("{mean:+.2}"),
            format!("{:+.2}", mean - half),
            format!("{:+.2}", mean + half),
            format!("{singles_sum:.2}")
        ));
    }
    out.spent = set.cost(&by_id);
    out.chosen = set.ids;
    out.seconds = start.elapsed().as_secs_f64();
    out
}

/// The verify run for any set (plan-form ids): mean gap against the base, with its 95% interval.
#[cfg(test)]
pub(crate) fn check_set(input: &Input, ids: &[u32]) -> Verify {
    let set = Set { ids: ids.to_vec() };
    let fin = set.build(&input.base, &input.data);
    let (mean, gaps, _) = race(
        input,
        &input.base,
        &fin,
        input.tune.verify_runs,
        input.seed + 999.0,
    );
    let n = gaps.len().max(1) as f64;
    let sd = (gaps.iter().map(|g| (g - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0)).sqrt();
    let half = 1.96 * sd / n.sqrt();
    Verify {
        mean,
        lo: mean - half,
        hi: mean + half,
        runs: gaps.len(),
        singles_sum: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: u32, own: f64, lower: Option<(u32, f64)>) -> Candidate {
        Candidate {
            id,
            own,
            lower,
            meta: 0.0,
            pinned: false,
        }
    }

    /// One per group: a gold costs its white too unless the white is in, and replaces it.
    #[test]
    fn group_and_cost_rules() {
        let white = cand(200352, 100.0, None);
        let gold = cand(200351, 150.0, Some((200352, 100.0)));
        let other = cand(200362, 80.0, None);
        let mut s = Set::default();
        assert_eq!(s.add_cost(&gold), Some(250.0));
        s.add(&white);
        assert_eq!(s.add_cost(&gold), Some(150.0));
        assert_eq!(s.add_cost(&white), None, "the same skill twice");
        s.add(&gold);
        assert_eq!(s.ids, vec![200351], "the gold replaces its white");
        assert_eq!(s.add_cost(&white), None, "a white after its gold");
        assert_eq!(s.add_cost(&other), Some(80.0));
        let by_id: HashMap<u32, &Candidate> =
            [(white.id, &white), (gold.id, &gold), (other.id, &other)]
                .into_iter()
                .collect();
        assert_eq!(s.cost(&by_id), 250.0);
    }

    #[test]
    fn meta_influence_levels() {
        assert_eq!(MetaInfluence::Off.lengths(), 0.0);
        assert!(MetaInfluence::TieBreak.lengths() < 0.1);
        assert_eq!(
            MetaInfluence::Strong.lengths(),
            crate::meta::META_SKILL_LENGTHS
        );
        assert_eq!(MetaInfluence::default(), MetaInfluence::TieBreak);
    }
}
