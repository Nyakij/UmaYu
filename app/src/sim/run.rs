//! The three simulator modes: Compare (two builds head to head over N runs, compare.ts), Skill
//! chart (lengths gained per skill, simulator.worker.ts) and Stamina (HP left at the finish, hpcalc.ts).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::builder::{Builder, HpKind};
use super::course::Course;
use super::data::SimData;
use super::hp::Hp;
use super::params::{HorseDesc, RaceParams, adjusted_stats, base_stats, order_range_for};
use super::rng::Rng;
use super::solver::{DuelingRates, Perspective, PosKeepMode, Solver, get_pacer, step, update_first_in_late_race};

pub const DT: f64 = 1.0 / 15.0;

/// One uma as entered in the simulator.
#[derive(Clone, Debug, PartialEq)]
pub struct UmaBuild {
    pub outfit: Option<u32>,
    pub stars: u8,
    pub unique_level: u32,
    pub desc: HorseDesc,
    /// Skill ids, unique included. One per skill group, except debuffs.
    pub skills: Vec<u32>,
    pub forced: HashMap<u32, f64>,
}

impl UmaBuild {
    /// Keep one skill per group (the last added), as Umalator's group-keyed skill map does.
    pub fn normalise(&mut self, data: &SimData) {
        let mut seen: HashMap<u32, usize> = HashMap::new();
        let mut out: Vec<u32> = Vec::new();
        for &id in &self.skills {
            if data.is_debuff(id) {
                out.push(id);
                continue;
            }
            match seen.get(&data.group_of(id)) {
                Some(&i) => out[i] = id,
                None => {
                    seen.insert(data.group_of(id), out.len());
                    out.push(id);
                }
            }
        }
        self.skills = out;
    }

    fn unique(&self, data: &SimData) -> Option<u32> {
        self.outfit.and_then(|o| data.unique_skill(o, self.stars))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimOptions {
    pub samples: usize,
    pub seed: f64,
    pub sync_rng: bool,
    pub wit_check: bool,
    pub rushed: bool,
    pub lead_competition: bool,
    pub compete_fight: bool,
    pub dueling: Option<DuelingRates>,
    pub lane_movement: bool,
}

impl Default for SimOptions {
    fn default() -> Self {
        SimOptions {
            samples: 1000,
            seed: 12345.0,
            sync_rng: true,
            wit_check: true,
            rushed: true,
            lead_competition: false,
            compete_fight: false,
            dueling: None,
            lane_movement: true,
        }
    }
}

/// Progress and cancellation shared with the UI thread.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicUsize,
    pub total: AtomicUsize,
    pub cancel: AtomicBool,
}

impl Progress {
    fn tick(&self) {
        self.done.fetch_add(1, Ordering::Relaxed);
    }
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// Speed and HP over one run, for charts.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    pub pos: Vec<f32>,
    pub speed: Vec<f32>,
    pub hp: Vec<f32>,
    /// Own skill activations: (skill, start m, end m or -1).
    pub skills: Vec<(u32, f32, f32)>,
}

#[derive(Clone, Debug, Default)]
pub struct SkillStat {
    pub id: u32,
    /// Share of runs where it activated at least once.
    pub rate: f64,
    pub mean_start: f64,
}

#[derive(Clone, Debug, Default)]
pub struct UmaStats {
    pub full_spurt_rate: f64,
    pub survival_rate: f64,
    pub rushed_rate: f64,
    pub rushed_mean_length: f64,
    pub lead_competition_rate: f64,
    pub dueling_rate: f64,
    pub first_late_rate: f64,
    pub mean_time: f64,
    pub mean_start_delay: f64,
    /// Mean last-spurt speed shortfall when not at full spurt (m/s).
    pub non_full_speed_loss: f64,
    pub skills: Vec<SkillStat>,
}

#[derive(Clone, Debug, Default)]
pub struct CompareResult {
    /// Uma 2's lead over Uma 1 in lengths (1 L = 2.5 m), per run, sorted. Negative: Uma 1 ahead.
    pub gaps: Vec<f64>,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub median: f64,
    pub umas: [UmaStats; 2],
    /// The run whose gap is closest to the median.
    pub median_run: [Trace; 2],
    pub warnings: Vec<String>,
}

fn median(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let mid = sorted.len() / 2;
    if mid > 0 && sorted.len().is_multiple_of(2) { (sorted[mid - 1] + sorted[mid]) / 2.0 } else { sorted[mid] }
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
}

#[derive(Default)]
struct Tally {
    n: usize,
    hp_died: usize,
    full_spurt: usize,
    rushed: Vec<f64>,
    lead: usize,
    dueling: usize,
    first_late: usize,
    times: Vec<f64>,
    delays: Vec<f64>,
    speed_loss: Vec<f64>,
    skill_runs: BTreeMap<u32, usize>,
    skill_starts: BTreeMap<u32, Vec<f64>>,
}

impl Tally {
    fn add(&mut self, s: &Solver, finish_time: f64) {
        self.n += 1;
        self.hp_died += usize::from(s.hp_died);
        if s.full_spurt {
            self.full_spurt += 1;
        } else if let Some(v) = s.non_full_velocity_diff {
            self.speed_loss.push(v);
        }
        if let Some((a, b)) = s.rushed_span {
            self.rushed.push(if b < 0.0 { s.course.distance - a } else { b - a });
        }
        self.lead += usize::from(s.lead_start.is_some());
        self.dueling += usize::from(s.compete_start.is_some());
        self.first_late += usize::from(s.first_in_late_race);
        self.times.push(finish_time);
        self.delays.push(s.start_delay);
        let mut seen = Vec::new();
        for a in &s.activations {
            self.skill_starts.entry(a.id).or_default().push(a.start);
            if !seen.contains(&a.id) {
                seen.push(a.id);
                *self.skill_runs.entry(a.id).or_default() += 1;
            }
        }
    }

    fn stats(&self) -> UmaStats {
        let n = self.n.max(1) as f64;
        UmaStats {
            full_spurt_rate: self.full_spurt as f64 / n * 100.0,
            survival_rate: (self.n - self.hp_died) as f64 / n * 100.0,
            rushed_rate: self.rushed.len() as f64 / n * 100.0,
            rushed_mean_length: mean(&self.rushed),
            lead_competition_rate: self.lead as f64 / n * 100.0,
            dueling_rate: self.dueling as f64 / n * 100.0,
            first_late_rate: self.first_late as f64 / n * 100.0,
            mean_time: mean(&self.times),
            mean_start_delay: mean(&self.delays),
            non_full_speed_loss: mean(&self.speed_loss),
            skills: self
                .skill_runs
                .iter()
                .map(|(id, runs)| SkillStat { id: *id, rate: *runs as f64 / n * 100.0, mean_start: mean(&self.skill_starts[id]) })
                .collect(),
        }
    }
}

/// Skill group keys as Umalator's skill map has them: the group id, or "group-n" for debuffs.
fn group_keys(data: &SimData, skills: &[u32]) -> Vec<String> {
    let mut n = 0;
    skills
        .iter()
        .map(|id| {
            let g = data.group_of(*id);
            if data.is_debuff(*id) {
                n += 1;
                format!("{g}-{}", n - 1)
            } else {
                g.to_string()
            }
        })
        .collect()
}

/// Run two builds head to head. `compare_mode` false is the skill chart's setting (position keep
/// for 3 sections instead of 10, no RNG desync).
pub fn run_comparison(
    data: &SimData,
    course: &Arc<Course>,
    race: &RaceParams,
    uma1: &UmaBuild,
    uma2: &UmaBuild,
    opts: &SimOptions,
    compare_mode: bool,
    traces: bool,
    progress: Option<&Progress>,
) -> CompareResult {
    let n = opts.samples.max(1);
    let mut standard = Builder::new(n, course.clone(), race.clone(), opts.seed);
    standard.pos_keep = PosKeepMode::Approximate;
    standard.compare_mode = compare_mode;
    let mut compare = standard.fork();
    if compare_mode && !opts.sync_rng {
        standard.desync();
    }
    standard.horse(uma1.desc.clone());
    compare.horse(uma2.desc.clone());
    for b in [&mut standard, &mut compare] {
        b.wit_check = opts.wit_check;
        b.rushed = opts.rushed;
        b.compete_fight = opts.compete_fight;
        b.dueling = opts.dueling;
        b.lead_competition = opts.lead_competition;
        b.lane_movement = opts.lane_movement;
    }

    // Skills both umas share (by group) go first in the same order, so their activation rolls stay in sync.
    let (k1, k2) = (group_keys(data, &uma1.skills), group_keys(data, &uma2.skills));
    let mut common: Vec<String> = k1.iter().filter(|k| k2.contains(k)).cloned().collect();
    common.sort_by(|a, b| match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x.total_cmp(&y),
        _ => std::cmp::Ordering::Equal,
    });
    let common_idx = |id: u32| common.iter().position(|g| *g == data.group_of(id).to_string()).unwrap_or(common.len());
    let sorted = |skills: &[u32]| {
        let mut v = skills.to_vec();
        v.sort_by(|a, b| common_idx(*a).cmp(&common_idx(*b)).then(a.cmp(b)));
        v
    };
    let (s1, s2) = (sorted(&uma1.skills), sorted(&uma2.skills));
    let wisdom = |u: &UmaBuild| adjusted_stats(&base_stats(&u.desc), course, race.ground).wisdom;
    let (w1, w2) = (wisdom(uma1), wisdom(uma2));
    let (u1, u2) = (uma1.unique(data), uma2.unique(data));
    let level = |id: u32, unique: Option<u32>, u: &UmaBuild| if Some(id) == unique { u.unique_level.max(1) } else { 1 };
    for &id in &s1 {
        standard.add_skill(id, Perspective::Mine, None, level(id, u1, uma1), uma1.forced.get(&id).copied());
    }
    for &id in &s2 {
        compare.add_skill(id, Perspective::Mine, None, level(id, u2, uma2), uma2.forced.get(&id).copied());
    }
    for &id in &s1 {
        compare.add_skill(id, Perspective::Other, Some(w1), level(id, u1, uma1), uma1.forced.get(&id).copied());
    }
    for &id in &s2 {
        standard.add_skill(id, Perspective::Other, Some(w2), level(id, u2, uma2), uma2.forced.get(&id).copied());
    }
    standard.with_asitame();
    compare.with_asitame();
    let pacer_horse = standard.use_default_pacer(data, &s1);

    let mut a = standard.build(data);
    let mut b = compare.build(data);
    let mut warnings = a.warnings.clone();
    for w in &b.warnings {
        if !warnings.contains(w) {
            warnings.push(w.clone());
        }
    }
    let mut pacer_rng = Rng::new(opts.seed + 1.0);
    let d = course.distance;
    let mut gaps = Vec::with_capacity(n);
    let (mut t1, mut t2) = (Tally::default(), Tally::default());
    let cutoff = (n * 4 / 5).max(n.saturating_sub(200));
    let (mut est_median, mut best_median_diff) = (0.0, f64::INFINITY);
    let mut median_run: [Trace; 2] = Default::default();
    let (mut p1, mut p2): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    let (mut tr1, mut tr2) = (Trace::default(), Trace::default());

    for i in 0..n {
        if progress.is_some_and(Progress::cancelled) {
            break;
        }
        let pacer = standard.build_pacer(&pacer_horse, i, Rng::new(pacer_rng.int32()));
        let mut x = a.next_solver();
        let mut y = b.next_solver();
        x.record = true;
        y.record = true;
        // Indices: 0 = uma 1, 1 = uma 2, 2 = pacemaker.
        x.umas = vec![1, 2, 0];
        y.umas = vec![0, 2, 1];
        let mut pm = pacer;
        pm.umas = vec![0, 1, 2];
        let mut solvers = vec![x, y, pm];
        p1.clear();
        p2.clear();
        if traces {
            tr1 = Trace::default();
            tr2 = Trace::default();
        }
        let (mut f1, mut f2) = (false, false);
        let (mut time1, mut time2) = (0.0, 0.0);
        while !f1 || !f2 {
            let current = get_pacer(&mut solvers, 2);
            for u in solvers[2].umas.clone() {
                solvers[u].pacer = current;
            }
            if solvers[2].pos < d {
                step(&mut solvers, 2, DT);
            }
            for (idx, fin, time, ps, tr) in [(1usize, &mut f2, &mut time2, &mut p2, &mut tr2), (0, &mut f1, &mut time1, &mut p1, &mut tr1)] {
                let s = &solvers[idx];
                if s.pos < d {
                    step(&mut solvers, idx, DT);
                    let s = &solvers[idx];
                    ps.push(s.pos);
                    if traces {
                        tr.pos.push(s.pos as f32);
                        tr.speed.push(s.displayed_speed() as f32);
                        tr.hp.push(s.hp.value() as f32);
                    }
                    if s.pos >= d {
                        let v = s.displayed_speed().max(0.1);
                        *time = s.time() - (s.pos - d) / v;
                    }
                } else if !*fin {
                    *fin = true;
                }
            }
            update_first_in_late_race(&mut solvers, 1);
        }
        solvers[1].cleanup();
        solvers[0].cleanup();
        let m = p1.len().min(p2.len());
        let gap = if m == 0 { 0.0 } else { (p2[m - 1] - p1[m - 1]) / 2.5 };
        gaps.push(gap);
        t1.add(&solvers[0], time1);
        t2.add(&solvers[1], time2);
        if i == cutoff {
            let mut s = gaps.clone();
            s.sort_by(f64::total_cmp);
            est_median = median(&s);
        }
        if traces && i >= cutoff && (gap - est_median).abs() < best_median_diff {
            best_median_diff = (gap - est_median).abs();
            for (k, tr) in [(0usize, &mut tr1), (1, &mut tr2)] {
                tr.skills = solvers[k].activations.iter().map(|a| (a.id, a.start as f32, a.end as f32)).collect();
            }
            median_run = [std::mem::take(&mut tr1), std::mem::take(&mut tr2)];
        }
        if let Some(p) = progress {
            p.tick();
        }
    }
    gaps.sort_by(f64::total_cmp);
    CompareResult {
        min: gaps.first().copied().unwrap_or(0.0),
        max: gaps.last().copied().unwrap_or(0.0),
        mean: mean(&gaps),
        median: median(&gaps),
        gaps,
        umas: [t1.stats(), t2.stats()],
        median_run,
        warnings,
    }
}

/// Race parameters for one course and conditions. `order_for` sets the expected field position
/// for that style, which the skill chart uses for order conditions.
pub fn race_params(ground: i32, weather: i32, season: i32, time: i32, order_for: Option<i32>) -> RaceParams {
    RaceParams {
        ground,
        weather,
        season,
        time,
        order_range: order_for.map(order_range_for),
        num_umas: Some(9),
        ..RaceParams::default()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChartRow {
    pub id: u32,
    pub mean: f64,
    pub median: f64,
    pub min: f64,
    pub max: f64,
    pub samples: usize,
}

/// Chart-mode options: no wit checks, rushing, dueling, spot struggle or lane movement, so each
/// skill's effect stands out.
pub fn chart_options(seed: f64, samples: usize) -> SimOptions {
    SimOptions {
        samples,
        seed,
        sync_rng: false,
        wit_check: false,
        rushed: false,
        lead_competition: false,
        compete_fight: false,
        dueling: None,
        lane_movement: false,
    }
}

fn with_skill(data: &SimData, base: &UmaBuild, id: u32) -> UmaBuild {
    let mut u = base.clone();
    let g = data.group_of(id);
    if !data.is_debuff(id) {
        u.skills.retain(|s| data.is_debuff(*s) || data.group_of(*s) != g);
    }
    u.skills.push(id);
    u
}

fn chart_round(data: &SimData, course: &Arc<Course>, race: &RaceParams, uma: &UmaBuild, skills: &[u32], seed: f64, samples: usize, progress: &Progress) -> Vec<ChartRow> {
    let opts = chart_options(seed, samples);
    skills
        .iter()
        .map(|&id| {
            if progress.cancelled() {
                return ChartRow { id, ..Default::default() };
            }
            let r = run_comparison(data, course, race, uma, &with_skill(data, uma, id), &opts, false, false, None);
            progress.tick();
            ChartRow { id, mean: r.mean, median: r.median, min: r.min, max: r.max, samples }
        })
        .collect()
}

/// Lengths each skill gains when added to `uma` (replacing a skill of its group), on this course.
/// Two rounds like Moomoolator: 25 runs each, then 175 more for skills that showed a gain.
pub fn run_chart(data: &SimData, course: &Arc<Course>, race: &RaceParams, uma: &UmaBuild, skills: &[u32], seed: f64, progress: &Progress) -> Vec<ChartRow> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16);
    let parallel = |list: &[u32], samples: usize| -> Vec<ChartRow> {
        if list.is_empty() {
            return Vec::new();
        }
        let chunk = list.len().div_ceil(threads);
        std::thread::scope(|scope| {
            let handles: Vec<_> = list.chunks(chunk).map(|c| scope.spawn(move || chart_round(data, course, race, uma, c, seed, samples, progress))).collect();
            handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        })
    };
    progress.total.store(skills.len() * 2, Ordering::Relaxed);
    let mut rows = parallel(skills, 25);
    let promising: Vec<u32> = rows.iter().filter(|r| r.max > 0.1 && (r.max - r.min).abs() > 0.1).map(|r| r.id).collect();
    progress.total.store(skills.len() + promising.len(), Ordering::Relaxed);
    let more = parallel(&promising, 175);
    for m in more {
        if let Some(r) = rows.iter_mut().find(|r| r.id == m.id) {
            let total = r.samples + m.samples;
            r.mean = (r.mean * r.samples as f64 + m.mean * m.samples as f64) / total as f64;
            if m.samples > r.samples {
                r.median = m.median;
            }
            r.min = r.min.min(m.min);
            r.max = r.max.max(m.max);
            r.samples = total;
        }
    }
    rows.sort_by(|a, b| b.mean.total_cmp(&a.mean));
    rows
}

#[derive(Clone, Debug, Default)]
pub struct HpCalcResult {
    /// HP left at the finish per run, sorted (negative: ran out).
    pub remaining: Vec<f64>,
    /// HP used up to the start of the last spurt, per run, sorted.
    pub required: Vec<f64>,
    pub downhill_save: Vec<f64>,
    /// Runs with HP for a full spurt at the last-spurt decision.
    pub full_spurt_runs: usize,
    pub runs: usize,
    pub median_run: Trace,
    pub max_hp: f64,
}

/// Stamina mode: how much HP the build has left at the finish (hpcalc.ts).
pub fn run_hpcalc(data: &SimData, course: &Arc<Course>, race: &RaceParams, uma: &UmaBuild, opts: &SimOptions, force_full: bool, progress: Option<&Progress>) -> HpCalcResult {
    let n = opts.samples.max(1);
    let mut b0 = Builder::new(n, course.clone(), race.clone(), opts.seed);
    b0.pos_keep = PosKeepMode::Approximate;
    b0.compare_mode = true;
    b0.horse(uma.desc.clone());
    b0.wit_check = opts.wit_check;
    b0.rushed = opts.rushed;
    b0.compete_fight = opts.compete_fight;
    b0.lead_competition = opts.lead_competition;
    b0.lane_movement = opts.lane_movement;
    b0.dueling = opts.dueling;
    let unique = uma.unique(data);
    for &id in &uma.skills {
        let lv = if Some(id) == unique { uma.unique_level.max(1) } else { 1 };
        b0.add_skill(id, Perspective::Mine, None, lv, uma.forced.get(&id).copied());
    }
    b0.with_asitame();
    let _pacer = b0.use_default_pacer(data, &uma.skills);
    let mut b1 = b0.fork();
    b1.hp_kind = HpKind::CalcRequired;
    b0.hp_kind = HpKind::ForceFull(force_full);
    let mut g0 = b0.build(data);
    let mut g1 = b1.build(data);
    let d = course.distance;
    let cutoff = (n * 4 / 5).max(n.saturating_sub(200));
    let mut out = HpCalcResult { runs: n, ..Default::default() };
    let (mut est_median, mut best) = (0.0, f64::INFINITY);
    for i in 0..n {
        if progress.is_some_and(Progress::cancelled) {
            break;
        }
        let mut s0 = g0.next_solver();
        s0.record = true;
        s0.umas = vec![0];
        let mut solvers = vec![s0];
        let mut tr = Trace::default();
        while solvers[0].pos < d {
            step(&mut solvers, 0, DT);
            let s = &solvers[0];
            tr.pos.push(s.pos as f32);
            tr.speed.push(s.displayed_speed() as f32);
            tr.hp.push(s.hp.value() as f32);
        }
        solvers[0].cleanup();
        let s0 = &solvers[0];
        if let Hp::ForceFull { hp, force, balance, was_full, downhill_save } = &s0.hp {
            out.max_hp = hp.max_hp;
            out.full_spurt_runs += usize::from(*was_full);
            out.downhill_save.push(*downhill_save);
            let final_hp = if *force { balance.min(hp.hp) } else { hp.hp };
            out.remaining.push(final_hp);
            if i == cutoff {
                let mut s = out.remaining.clone();
                s.sort_by(f64::total_cmp);
                est_median = median(&s);
            }
            if i >= cutoff && (final_hp - est_median).abs() < best {
                best = (final_hp - est_median).abs();
                tr.skills = s0.activations.iter().map(|a| (a.id, a.start as f32, a.end as f32)).collect();
                out.median_run = tr;
            }
        }
        let mut s1 = g1.next_solver();
        s1.umas = vec![0];
        let mut solvers = vec![s1];
        let mut pushed = false;
        while solvers[0].pos < d {
            step(&mut solvers, 0, DT);
            if solvers[0].is_last_spurt {
                out.required.push(-solvers[0].hp.value());
                pushed = true;
                break;
            }
        }
        if !pushed {
            out.required.push(-solvers[0].hp.value());
        }
        if let Some(p) = progress {
            p.tick();
        }
    }
    out.remaining.sort_by(f64::total_cmp);
    out.required.sort_by(f64::total_cmp);
    out.downhill_save.sort_by(f64::total_cmp);
    out
}

pub fn median_of(sorted: &[f64]) -> f64 {
    median(sorted)
}
