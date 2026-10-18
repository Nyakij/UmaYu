//! Recommendations for the planner: support cards, trainee and skills for the plan's goal.
//!
//! Cards: each candidate is scored by what it adds to a simulated career in the plan's scenario
//! (`career.rs`: stats toward the target, skill points, scenario link), plus the value of the
//! skills it can teach on the target course, style and conditions. That skill value comes from
//! the race simulator's skill chart when it has run for this goal, otherwise from whether the
//! skill's conditions fit the target. Everything converts to stat points: a length (2.5 m) is
//! worth what +Speed would buy on this course in the simulator. These are suggestions built on
//! the assumptions listed in knowledge/scenarios.json, not a solved optimum.
//!
//! Skills: a knapsack over the plan's skill candidates, maximising simulated lengths gained within
//! the skill points the career simulation expects, with hint discounts.
//!
//! Both put the community meta sheet's picks for the plan's Champions Meeting and style first
//! (`meta.rs`); the simulation orders everything else.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

use crate::App;
use crate::career::{self, CardStats, CareerInput, CareerResult, Common, Scenario, card_stats, effect_names, hint_discount};
use crate::data::Rarity;
use crate::knowledge::{Distance, Style, Surface, grade_rank};
use crate::tab_planner::Fit;

#[derive(Clone, Debug)]
pub(crate) struct CardPick {
    pub id: u32,
    pub score: f32,
    /// How strongly the meta sheet backs it for this plan (0 = not at all); ranks before `score`.
    pub meta: f32,
    pub reason: String,
}

/// Stat points a length is worth when nothing better is known (≈ +60 Speed per length).
const DEFAULT_STATS_PER_LENGTH: f64 = 60.0;
/// Stat points a skill point is worth when nothing better is known.
const DEFAULT_SP_VALUE: f64 = 0.2;
/// Career runs per candidate. Candidates share random numbers, so differences are stable.
const RUNS: usize = 32;

/// Everything the card ranking needs, copied out of the app so it can run anywhere.
#[derive(Clone)]
pub(crate) struct RankContext {
    pub scenario: Option<Scenario>,
    pub common: Option<Common>,
    pub weights: [f64; 5],
    pub target: [f64; 5],
    pub growth: [f64; 5],
    pub base: [f64; 5],
    pub objectives: Vec<(usize, String)>,
    pub uma_events: Vec<career::Gain>,
    pub trainee: Option<String>,
    pub trainee_skills: HashSet<u32>,
    /// Card stats at the level each would be used at: owned cards, and the friend pool.
    pub owned: HashMap<u32, CardStats>,
    pub friend: HashMap<u32, CardStats>,
    pub kinds: HashMap<u32, String>,
    pub card_skills: HashMap<u32, (Vec<u32>, Vec<u32>)>,
    /// Lengths each skill gains on the target (simulated or estimated).
    pub skill_lengths: HashMap<u32, f64>,
    pub simulated: bool,
    pub stats_per_length: f64,
    pub sp_value: f64,
    pub link_gold: HashMap<String, u32>,
    pub skill_names: HashMap<u32, String>,
    /// What the meta sheet recommends for the plan's CM and style.
    pub meta: crate::meta::CardMeta,
}

impl RankContext {
    fn stats(&self, id: u32, friend: bool) -> Option<&CardStats> {
        if friend { self.friend.get(&id) } else { self.owned.get(&id) }
    }

    fn career(&self, deck: &[&CardStats]) -> Option<CareerResult> {
        let (common, scenario) = (self.common.as_ref()?, self.scenario.as_ref()?);
        Some(career::simulate(&CareerInput {
            common,
            scenario,
            growth: self.growth,
            base_stats: self.base,
            deck: deck.iter().map(|c| (*c).clone()).collect(),
            weights: self.weights,
            target: self.target,
            objectives: self.objectives.clone(),
            uma_events: self.uma_events.clone(),
            runs: RUNS,
            seed: 7.0,
        }))
    }
}

/// Default stat weights when the plan has no target stats: by distance, nudged by style.
fn default_weights(distance: Distance, surface: Surface, style: Style) -> [f64; 5] {
    let stamina = match distance {
        Distance::Sprint => 0.35,
        Distance::Mile => 0.5,
        Distance::Medium => 0.75,
        Distance::Long => 0.95,
    };
    let power = match distance {
        Distance::Sprint => 0.85,
        Distance::Mile => 0.8,
        Distance::Medium => 0.65,
        Distance::Long => 0.55,
    } + if surface == Surface::Dirt { 0.1 } else { 0.0 }
        + if matches!(style, Style::Late | Style::End) { 0.1 } else { 0.0 };
    let wit = 0.7 + if style == Style::Front { 0.1 } else { 0.0 };
    [1.0, stamina, power, 0.45, wit]
}

impl App {
    /// The plan's stat priorities: its target stats, else defaults for its distance and style.
    pub(crate) fn plan_weights(&self) -> [f64; 5] {
        let p = &self.planner.plan;
        if p.target.iter().any(|v| *v > 0) {
            let sum: f64 = p.target.iter().map(|v| f64::from(*v)).sum();
            p.target.map(|v| f64::from(v) / sum * 5.0)
        } else {
            default_weights(p.distance, p.surface, p.style)
        }
    }

    /// The plan's scenario rules, if knowledge/scenarios.json has them.
    pub(crate) fn plan_scenario(&self) -> Option<&Scenario> {
        self.kb.scenarios.as_ref()?.get(&self.plan_scenario_name())
    }

    /// Lengths a skill gains for this plan: simulated if the skill chart ran for it, else an
    /// estimate from how its conditions fit (gold 0.6 L, white 0.25 L if it fits; less if generic).
    pub(crate) fn skill_lengths(&self, id: u32) -> (f64, bool) {
        if let Some(v) = self.planner.current_skill_values(self.plan_key()).and_then(|v| v.rows.get(&id)) {
            return (v.mean.max(0.0), true);
        }
        let Some(s) = self.kb.skills.get(&id) else { return (0.0, false) };
        // Can it trigger on this course at all (direction, ground, weather, season, distance, style)?
        if self.can_activate_on_plan(id) == Some(false) {
            return (0.0, false);
        }
        let gold = s.rarity >= 2;
        let v = match self.skill_fit(id) {
            Fit::Target => if gold { 0.6 } else { 0.25 },
            Fit::General => if gold { 0.35 } else { 0.12 },
            Fit::Other => 0.0,
        };
        (v, false)
    }

    /// Whether a skill's conditions can be met on the plan's course and conditions, by the
    /// simulator's condition engine. `None` when the plan has no course or the simulator lacks the skill.
    pub(crate) fn can_activate_on_plan(&self, id: u32) -> Option<bool> {
        let key = self.plan_key();
        {
            let cache = self.planner.activation_cache.borrow();
            if cache.0 == key
                && let Some(v) = cache.1.get(&id)
            {
                return *v;
            }
        }
        let result = (|| {
            // The plan's Champions Meeting turns some skills off.
            if self.plan_disabled(id) {
                return Some(false);
            }
            let data = self.sim_data.as_ref()?;
            if !data.skills.contains_key(&id) {
                return None;
            }
            let setup = self.plan_race_setup();
            let course = data.courses.get(&setup.course?)?;
            let p = &self.planner.plan;
            let race = crate::sim::run::race_params(
                i32::from(setup.ground.unwrap_or(1)),
                i32::from(setup.weather.unwrap_or(1)),
                i32::from(setup.season.unwrap_or(1)),
                i32::from(setup.time.unwrap_or(2)),
                Some(crate::tab_sim::strategy_of(p.style)),
            );
            let stats = if p.target.iter().any(|v| *v > 0) { p.target.map(f64::from) } else { [1000.0; 5] };
            let desc = crate::sim::params::HorseDesc {
                speed: stats[0],
                stamina: stats[1],
                power: stats[2],
                guts: stats[3],
                wisdom: stats[4],
                strategy: crate::tab_sim::strategy_of(p.style),
                distance_apt: 1,
                surface_apt: 1,
                strategy_apt: 1,
                mood: 2,
            };
            let horse = crate::sim::params::base_stats(&desc);
            let built = crate::sim::builder::build_skill_data(data, &horse, &race, course, id, crate::sim::solver::Perspective::Mine, 1).ok()?;
            Some(built.iter().any(|sd| sd.regions.first().is_some_and(|r| r.start < 9999.0)))
        })();
        let mut cache = self.planner.activation_cache.borrow_mut();
        if cache.0 != key {
            *cache = (key, HashMap::new());
        }
        cache.1.insert(id, result);
        result
    }

    pub(crate) fn rank_context(&self) -> RankContext {
        let p = &self.planner.plan;
        let names = effect_names(&self.kb);
        let scenario = self.plan_scenario().cloned();
        let info = p.trainee.and_then(|t| self.kb.umas.get(&t));
        let (objectives, uma_events) = info.map(|u| career::trainee_inputs(&self.kb, u)).unwrap_or_default();
        let friend_lb = p.friend_lb.min(4);
        let mut owned = HashMap::new();
        let mut friend = HashMap::new();
        let mut kinds = HashMap::new();
        let mut card_skills = HashMap::new();
        let mut all_skills: BTreeSet<u32> = BTreeSet::new();
        for c in &self.roster.cards {
            if let Some(ci) = self.kb.cards.get(&c.id) {
                owned.insert(c.id, card_stats(ci, u32::from(c.level), &names, scenario.as_ref()));
            }
        }
        let today = crate::ui_util::today();
        for ci in self.kb.cards.values().filter(|c| c.rarity == "SSR" && c.released(&today)) {
            let level = u32::from(Rarity::SSR.max_level(friend_lb));
            friend.insert(ci.id, card_stats(ci, level, &names, scenario.as_ref()));
        }
        for ci in self.kb.cards.values() {
            kinds.insert(ci.id, ci.kind.clone());
            card_skills.insert(ci.id, (ci.hints.clone(), ci.event_skills.clone()));
            if owned.contains_key(&ci.id) || friend.contains_key(&ci.id) {
                all_skills.extend(ci.hints.iter().chain(&ci.event_skills).copied());
            }
        }
        let mut trainee_skills = HashSet::new();
        if let (Some(i), Some(o)) = (info, p.trainee.and_then(|t| self.owned_uma(t))) {
            trainee_skills.extend(i.skills.iter().filter(|s| s.potential <= o.potential).map(|s| s.id));
            trainee_skills.extend(i.event_skills.iter().copied());
        }
        let mut link_gold = HashMap::new();
        if let Some(sc) = &scenario {
            for (uma, skill) in &sc.link_golds {
                if let Some(id) = self.kb.skill_order.iter().find(|id| self.kb.skills[*id].name == *skill && self.kb.skills[*id].rarity >= 2) {
                    link_gold.insert(uma.clone(), *id);
                    all_skills.insert(*id);
                }
            }
        }
        let mut skill_lengths = HashMap::new();
        let mut simulated = false;
        for id in &all_skills {
            let (v, sim) = self.skill_lengths(*id);
            simulated |= sim;
            skill_lengths.insert(*id, v);
        }
        let values = self.planner.current_skill_values(self.plan_key());
        let stats_per_length = values.and_then(|v| v.stats_per_length).unwrap_or(DEFAULT_STATS_PER_LENGTH);
        let skill_names = all_skills.iter().map(|id| (*id, self.kb.skill_name(*id))).collect();
        RankContext {
            common: self.kb.scenarios.as_ref().map(|s| s.common.clone()),
            scenario,
            weights: self.plan_weights(),
            target: p.target.map(f64::from),
            growth: info.map_or([0.0; 5], |u| u.growth.map(f64::from)),
            base: info.map_or([100.0; 5], |u| u.base.map(f64::from)),
            objectives,
            uma_events,
            trainee: info.map(|u| u.name.clone()),
            trainee_skills,
            owned,
            friend,
            kinds,
            card_skills,
            skill_lengths,
            simulated,
            stats_per_length,
            sp_value: values.and_then(|v| v.sp_value).unwrap_or(DEFAULT_SP_VALUE),
            link_gold,
            skill_names,
            meta: self.plan_card_meta(),
        }
    }

    fn deck6(&self) -> [Option<u32>; 6] {
        let p = &self.planner.plan;
        [p.deck[0], p.deck[1], p.deck[2], p.deck[3], p.deck[4], p.friend]
    }

    fn rank_cache_key(&self) -> RankKey {
        (self.plan_key(), self.rank_key(), self.deck6())
    }

    /// Ranked candidates for one deck slot (0..=4 owned, 5 friend), given the rest of the deck.
    /// Computes it now if no cached ranking matches; the UI uses `slot_candidates_ready` instead.
    #[cfg(test)]
    pub(crate) fn slot_candidates(&self, slot: usize) -> Vec<CardPick> {
        let key = self.rank_cache_key();
        if let Some(picks) = self.planner.rank_cache.borrow().get(key, slot) {
            return picks.clone();
        }
        let picks = rank_slot(&self.rank_context(), &key.2, slot);
        self.planner.rank_cache.borrow_mut().insert(key, slot, picks.clone());
        picks
    }

    /// The slot's ranking if it's ready; otherwise starts ranking every slot in the background
    /// (this slot first) and returns None. The UI stays responsive while it runs.
    pub(crate) fn slot_candidates_ready(&self, slot: usize, ctx: &eframe::egui::Context) -> Option<Vec<CardPick>> {
        let key = self.rank_cache_key();
        let mut cache = self.planner.rank_cache.borrow_mut();
        cache.drain(key);
        if let Some(picks) = cache.get(key, slot) {
            return Some(picks.clone());
        }
        if !cache.running(key) {
            let rank = self.rank_context();
            cache.start(key, slot, rank, ctx.clone());
        }
        None
    }

    /// Whether a background ranking is still running for the current deck.
    pub(crate) fn ranking_busy(&self) -> bool {
        let key = self.rank_cache_key();
        let mut cache = self.planner.rank_cache.borrow_mut();
        cache.drain(key);
        cache.running(key)
    }

    /// Hash of what card values depend on besides the plan key: scenario, deck LBs, friend LB.
    fn rank_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.plan_scenario_name().hash(&mut h);
        self.planner.plan.friend_lb.hash(&mut h);
        for c in &self.roster.cards {
            (c.id, c.lb, c.level).hash(&mut h);
        }
        self.planner.current_skill_values(self.plan_key()).map(|v| v.rows.len()).hash(&mut h);
        h.finish()
    }

    /// Score and reason for one card in the current deck, from the slot ranking. `Err(())` while
    /// the ranking is still running in the background.
    pub(crate) fn card_value(&self, id: u32, ctx: &eframe::egui::Context) -> Result<Option<(f32, String)>, ()> {
        let p = &self.planner.plan;
        let Some(slot) = (if p.friend == Some(id) { Some(5) } else { p.deck.iter().position(|d| *d == Some(id)) }) else { return Ok(None) };
        let picks = self.slot_candidates_ready(slot, ctx).ok_or(())?;
        Ok(picks.into_iter().find(|c| c.id == id).map(|c| (c.score, c.reason)))
    }

    /// Fill the deck's empty slots (or all of them) with the best candidates, one slot at a time.
    /// Returns the cards it put in.
    pub(crate) fn recommend_deck(&mut self, replace_all: bool) -> Vec<CardPick> {
        if replace_all {
            self.planner.plan.deck = [None; 5];
            self.planner.plan.friend = None;
        }
        let ctx = self.rank_context();
        let p = &self.planner.plan;
        let mut deck = [p.deck[0], p.deck[1], p.deck[2], p.deck[3], p.deck[4], p.friend];
        let mut filled = Vec::new();
        for slot in 0..6 {
            if deck[slot].is_some() {
                continue;
            }
            let Some(best) = rank_slot(&ctx, &deck, slot).into_iter().next() else { continue };
            deck[slot] = Some(best.id);
            filled.push(best);
        }
        let plan = &mut self.planner.plan;
        plan.deck = [deck[0], deck[1], deck[2], deck[3], deck[4]];
        if plan.friend.is_none() && deck[5].is_some() {
            plan.friend_lb = 4;
        }
        plan.friend = deck[5];
        filled
    }

    /// Saved parents from account/parents.md, one line each: "Uma — blue · pink · green".
    pub(crate) fn saved_parents(&self) -> Vec<String> {
        let Some(root) = &self.root else { return Vec::new() };
        let text = std::fs::read_to_string(root.join("account").join("parents.md")).unwrap_or_default();
        let Some(table) = crate::data::tables(&text).into_iter().next() else { return Vec::new() };
        let col = |prefix: &str| table.headers.iter().position(|h| h.starts_with(prefix));
        let (uma, blue, pink, green) = (col("Uma"), col("Blue"), col("Pink"), col("Green"));
        let cell = |row: &[String], i: Option<usize>| i.and_then(|i| row.get(i)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        table
            .rows
            .iter()
            .filter_map(|row| {
                let name = cell(row, uma)?;
                let sparks: Vec<String> = [blue, pink, green].into_iter().filter_map(|i| cell(row, i)).collect();
                Some(if sparks.is_empty() { name } else { format!("{name} — {}", sparks.join(" · ")) })
            })
            .collect()
    }

    /// The trainee's aptitudes for the goal that are below A: pink sparks from parents raise them.
    pub(crate) fn pink_sparks_wanted(&self) -> Vec<String> {
        let p = &self.planner.plan;
        let Some(info) = p.trainee.and_then(|t| self.kb.umas.get(&t)) else { return Vec::new() };
        let a = &info.aptitude;
        [(p.distance.label(), a.distance(p.distance)), (p.surface.label(), a.surface(p.surface)), (p.style.label(), a.style(p.style))]
            .into_iter()
            .filter(|(_, g)| grade_rank(g) < 7)
            .map(|(label, g)| format!("{label} (now {g})"))
            .collect()
    }

    /// Owned umas the meta sheet recommends for the plan's CM and style (in its order), then the
    /// rest ranked by aptitude fit for the goal, stars and potential.
    pub(crate) fn trainee_candidates(&self) -> Vec<u32> {
        let p = &self.planner.plan;
        let meta = self.plan_meta_trainees();
        let mut ids: Vec<(u32, u32)> = self
            .roster
            .umas
            .iter()
            .filter_map(|u| {
                let a = &self.kb.umas.get(&u.id)?.aptitude;
                let fit = u32::from(grade_rank(a.distance(p.distance))) * 3
                    + u32::from(grade_rank(a.surface(p.surface))) * 3
                    + u32::from(grade_rank(a.style(p.style))) * 2;
                let sheet = meta.iter().position(|m| *m == u.id).map_or(0, |i| (20 - i.min(19) as u32) * 100_000);
                Some((u.id, sheet + fit * 100 + u32::from(u.stars) * 10 + u32::from(u.potential)))
            })
            .collect();
        ids.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        ids.into_iter().map(|(id, _)| id).collect()
    }

    /// The career simulation of the current plan (deck as chosen), cached.
    pub(crate) fn plan_career(&self) -> Option<CareerResult> {
        let key = (self.plan_key(), self.rank_key(), self.planner.plan.deck, self.planner.plan.friend);
        if let Some((k, r)) = self.planner.career_cache.borrow().as_ref()
            && *k == key
        {
            return Some(r.clone());
        }
        let ctx = self.rank_context();
        let p = &self.planner.plan;
        let mut deck: Vec<&CardStats> = p.deck.iter().flatten().filter_map(|id| ctx.owned.get(id)).collect();
        if let Some(f) = p.friend.and_then(|id| ctx.friend.get(&id)) {
            deck.push(f);
        }
        let (common, scenario) = (ctx.common.as_ref()?, ctx.scenario.as_ref()?);
        let r = career::simulate(&CareerInput {
            common,
            scenario,
            growth: ctx.growth,
            base_stats: ctx.base,
            deck: deck.into_iter().cloned().collect(),
            weights: ctx.weights,
            target: ctx.target,
            objectives: ctx.objectives.clone(),
            uma_events: ctx.uma_events.clone(),
            runs: 400,
            seed: 11.0,
        });
        *self.planner.career_cache.borrow_mut() = Some((key, r.clone()));
        Some(r)
    }

    /// The SP available for skills: the plan's own budget if set, else the career estimate.
    pub(crate) fn sp_budget(&self) -> Option<f64> {
        self.planner.plan.sp_budget.map(f64::from).or_else(|| self.plan_career().map(|c| c.sp_mean))
    }

    /// A skill's cost after the hints this deck is expected to give, including the lower version
    /// a gold or ◎ skill needs if the plan doesn't already have it.
    pub(crate) fn skill_cost(&self, id: u32, career: Option<&CareerResult>) -> f64 {
        let own = |sid: u32| {
            let sp = self.kb.skills.get(&sid).map_or(0.0, |s| f64::from(s.sp));
            let lv = career.and_then(|c| c.hint_levels.get(&sid)).copied().unwrap_or(0.0);
            sp * (1.0 - hint_discount(lv))
        };
        own(id) + self.lower_version(id).map_or(0.0, own)
    }

    /// The ○/white skill a ◎/gold skill upgrades (same group, the id ending in 2 for one ending in 1).
    pub(crate) fn lower_version(&self, id: u32) -> Option<u32> {
        if id % 10 != 1 || id >= 1_000_000 {
            return None;
        }
        let lower = id + 1;
        let (s, l) = (self.kb.skills.get(&id)?, self.kb.skills.get(&lower)?);
        (s.category != "Unique" && l.sp > 0 && l.rarity <= s.rarity).then_some(lower)
    }

    /// Skills to buy from the candidates: the meta sheet's recommendations for the plan's CM and
    /// style first, then the set that gains the most lengths on the target, all within the SP
    /// budget (simulated gains if the skill chart ran for this goal, estimates otherwise).
    pub(crate) fn recommend_skills(&self, candidates: &[u32]) -> Vec<u32> {
        let career = self.plan_career();
        let budget = self.sp_budget().unwrap_or(1200.0);
        let meta = self.plan_meta_skills();
        let items: Vec<(u32, f64, f64)> = candidates
            .iter()
            // Never buy a skill the plan's Champions Meeting turns off.
            .filter(|id| !self.plan_disabled(**id))
            .filter_map(|id| {
                let s = self.kb.skills.get(id)?;
                if s.sp == 0 || s.category == "Unique" {
                    return None;
                }
                let (gain, _) = self.skill_lengths(*id);
                // A recommended skill that can't trigger on this course isn't worth buying.
                let boost = match meta.get(id) {
                    Some((w, _)) if self.can_activate_on_plan(*id) != Some(false) => f64::from(*w) * crate::meta::META_SKILL_LENGTHS,
                    _ => 0.0,
                };
                (gain + boost > 0.0).then(|| (*id, gain + boost, self.skill_cost(*id, career.as_ref())))
            })
            .collect();
        knapsack(&items, budget, |id| id / 10)
    }
}

/// Multiple-choice knapsack: at most one skill per group (a gold and its white share a group),
/// maximising total gain within `budget` SP. Costs and budget are rounded to 5 SP.
pub(crate) fn knapsack(items: &[(u32, f64, f64)], budget: f64, group: impl Fn(u32) -> u32) -> Vec<u32> {
    let step = 5.0;
    let cap = (budget / step).floor().max(0.0) as usize;
    let mut groups: Vec<(u32, Vec<(u32, f64, usize)>)> = Vec::new();
    for &(id, gain, cost) in items {
        let g = group(id);
        let w = (cost / step).ceil() as usize;
        match groups.iter_mut().find(|(k, _)| *k == g) {
            Some((_, v)) => v.push((id, gain, w)),
            None => groups.push((g, vec![(id, gain, w)])),
        }
    }
    let n = groups.len();
    let mut best = vec![vec![0.0f64; cap + 1]; n + 1];
    let mut choice = vec![vec![usize::MAX; cap + 1]; n + 1];
    for (gi, (_, opts)) in groups.iter().enumerate() {
        for c in 0..=cap {
            best[gi + 1][c] = best[gi][c];
            for (oi, (_, gain, w)) in opts.iter().enumerate() {
                if *w <= c && best[gi][c - w] + gain > best[gi + 1][c] {
                    best[gi + 1][c] = best[gi][c - w] + gain;
                    choice[gi + 1][c] = oi;
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut c = cap;
    for gi in (1..=n).rev() {
        let oi = choice[gi][c];
        if oi != usize::MAX {
            let (id, _, w) = groups[gi - 1].1[oi];
            out.push(id);
            c -= w;
        }
    }
    out.reverse();
    out
}

fn fmt_stats(d: &[f64; 5]) -> String {
    let names = ["Spd", "Sta", "Pow", "Gut", "Wit"];
    let parts: Vec<String> = d.iter().zip(names).filter(|(v, _)| v.abs() >= 1.0).map(|(v, n)| format!("{n} {v:+.0}")).collect();
    if parts.is_empty() { "no stats".into() } else { parts.join(" ") }
}

/// Rank every card that can go in `slot` (0..=4 owned, 5 friend) given the rest of the deck.
pub(crate) fn rank_slot(ctx: &RankContext, deck: &[Option<u32>; 6], slot: usize) -> Vec<CardPick> {
    let friend = slot == 5;
    let others: Vec<(u32, bool)> = (0..6).filter(|s| *s != slot).filter_map(|s| deck[s].map(|id| (id, s == 5))).collect();
    let taken: Vec<String> = others.iter().filter_map(|(id, f)| ctx.stats(*id, *f).map(|c| c.uma.clone())).collect();
    let other_stats: Vec<&CardStats> = others.iter().filter_map(|(id, f)| ctx.stats(*id, *f)).collect();
    // Skills the rest of the deck or the trainee already offer don't count twice.
    let mut offered: HashSet<u32> = ctx.trainee_skills.clone();
    for (id, _) in &others {
        if let Some((h, e)) = ctx.card_skills.get(id) {
            offered.extend(h.iter().chain(e));
        }
    }
    let pool: Vec<u32> = {
        let map = if friend { &ctx.friend } else { &ctx.owned };
        let mut v: Vec<u32> =
            map.values().filter(|c| ctx.trainee.as_deref() != Some(c.uma.as_str()) && !taken.contains(&c.uma)).map(|c| c.id).collect();
        v.sort_unstable();
        v
    };
    let base = ctx.career(&other_stats);
    let score_one = |id: u32| -> Option<CardPick> {
        let c = ctx.stats(id, friend)?;
        let kind = ctx.kinds.get(&id).cloned().unwrap_or_default();
        let mut reason = Vec::new();
        let mut score = 0.0;
        // Career: stats toward the target (less once a stat passes it) and skill points.
        if let (Some(base), Some(with)) = (&base, {
            let mut d = other_stats.clone();
            d.push(c);
            ctx.career(&d)
        }) {
            let mut delta = [0.0; 5];
            for i in 0..5 {
                delta[i] = with.stats_mean[i] - base.stats_mean[i];
                let worth = if ctx.target[i] > 0.0 { career::need_value(base.stats_mean[i], delta[i], ctx.target[i], 0.3) } else { delta[i] };
                score += ctx.weights[i] * worth;
            }
            let dsp = with.sp_mean - base.sp_mean;
            score += dsp * ctx.sp_value;
            reason.push(format!("career {} · SP {dsp:+.0}", fmt_stats(&delta)));
        } else {
            // No scenario data: its own-stat training effects, weighted by the goal.
            let fac = career::facility_of(&kind);
            let w = fac.map_or(0.75, |f| ctx.weights[f]);
            let v = c.friendship + c.training * 1.5 + fac.map_or(0.0, |f| c.stat_bonus[f] * 6.0) + c.specialty * 0.12 + c.race_bonus * 0.5;
            score += v * w;
            reason.push(format!("Friendship {:.0} · Training {:.0}", c.friendship, c.training));
        }
        // Skills it teaches, valued on the target course; the best ones count most.
        let (hints, events) = ctx.card_skills.get(&id).cloned().unwrap_or_default();
        let mut vals: Vec<(f64, u32)> = hints
            .iter()
            .map(|s| (0.55, *s))
            .chain(events.iter().map(|s| (0.8, *s)))
            .filter(|(_, s)| !offered.contains(s))
            .map(|(p, s)| (ctx.skill_lengths.get(&s).copied().unwrap_or(0.0) * p, s))
            .filter(|(v, _)| *v > 0.0)
            .collect();
        vals.sort_by(|a, b| b.0.total_cmp(&a.0));
        vals.dedup_by_key(|v| v.1);
        let decay = [1.0, 0.6, 0.4, 0.3, 0.2, 0.15];
        let lengths: f64 = vals.iter().zip(decay).map(|((v, _), d)| v * d).sum();
        score += lengths * ctx.stats_per_length;
        if let Some((v, s)) = vals.first() {
            let name = ctx.skill_names.get(s).cloned().unwrap_or_default();
            let how = if ctx.simulated { "simulated" } else { "estimated" };
            reason.push(format!("skills ≈ {:+.2} L ({how}; best: {name} {:+.2} L)", lengths, v));
        }
        // Scenario link.
        if c.linked {
            let mut note = "scenario link".to_string();
            if let Some(gold) = ctx.link_gold.get(&c.uma) {
                let v = ctx.skill_lengths.get(gold).copied().unwrap_or(0.0) * 0.7;
                score += v * ctx.stats_per_length;
                note += &format!(" (+{} hint {v:+.2} L)", ctx.skill_names.get(gold).cloned().unwrap_or_default());
            }
            reason.push(note);
        }
        let (meta, why) = ctx.meta.of(id, friend, &kind);
        if meta > 0.0 {
            reason.insert(0, format!("{} {why}", crate::meta::stars(meta)));
        }
        Some(CardPick { id, score: score as f32, meta, reason: format!("{} {kind} · {}", rarity_label(id), reason.join(" · ")) })
    };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 12);
    let chunk = pool.len().div_ceil(threads).max(1);
    let mut picks: Vec<CardPick> = std::thread::scope(|scope| {
        let handles: Vec<_> = pool.chunks(chunk).map(|ids| scope.spawn(move || ids.iter().filter_map(|id| score_one(*id)).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    // The meta sheet's picks first, by how strongly it backs them; the simulation orders cards it
    // backs about equally, and the rest.
    let rank = |c: &CardPick| crate::meta::meta_rank(c.meta);
    picks.sort_by(|a, b| rank(b).cmp(&rank(a)).then(b.score.total_cmp(&a.score)).then(b.id.cmp(&a.id)));
    picks
}

fn rarity_label(id: u32) -> &'static str {
    Rarity::from_id(id).map_or("", |r| r.label())
}

/// Plan key, rank key and the six deck slots: what a slot ranking depends on.
pub(crate) type RankKey = (u64, u64, [Option<u32>; 6]);

/// Slot rankings for one deck, filled synchronously or by a background job.
#[derive(Default)]
pub(crate) struct RankCache {
    key: Option<RankKey>,
    slots: HashMap<usize, Vec<CardPick>>,
    job: Option<RankJob>,
}

struct RankJob {
    key: RankKey,
    rx: mpsc::Receiver<(usize, Vec<CardPick>)>,
    cancel: Arc<AtomicBool>,
}

impl RankCache {
    /// Forget every ranking, e.g. after the roster or the skill values change.
    pub(crate) fn clear(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.key = None;
        self.slots.clear();
    }

    fn get(&self, key: RankKey, slot: usize) -> Option<&Vec<CardPick>> {
        (self.key == Some(key)).then(|| self.slots.get(&slot)).flatten()
    }

    fn insert(&mut self, key: RankKey, slot: usize, picks: Vec<CardPick>) {
        if self.key != Some(key) {
            self.key = Some(key);
            self.slots.clear();
        }
        self.slots.insert(slot, picks);
    }

    fn running(&self, key: RankKey) -> bool {
        self.job.as_ref().is_some_and(|j| j.key == key)
    }

    /// Collect finished slots from the background job; drop a job for an outdated deck.
    fn drain(&mut self, key: RankKey) {
        let Some(job) = self.job.take() else { return };
        if job.key != key {
            job.cancel.store(true, Ordering::Relaxed);
            return;
        }
        let mut done = false;
        loop {
            match job.rx.try_recv() {
                Ok((slot, picks)) => self.insert(job.key, slot, picks),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
            }
        }
        // Every slot ranked: the job is finished, even if its thread hasn't quite exited yet.
        if self.key == Some(job.key) && self.slots.len() == 6 {
            done = true;
        }
        if !done {
            self.job = Some(job);
        }
    }

    /// Rank every slot on a background thread, `first` first, skipping slots already cached.
    fn start(&mut self, key: RankKey, first: usize, ctx: RankContext, repaint: eframe::egui::Context) {
        if let Some(old) = self.job.take() {
            old.cancel.store(true, Ordering::Relaxed);
        }
        let cached: Vec<usize> = if self.key == Some(key) { self.slots.keys().copied().collect() } else { Vec::new() };
        let order: Vec<usize> = std::iter::once(first).chain((0..6).filter(|s| *s != first)).filter(|s| !cached.contains(s)).collect();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        std::thread::spawn(move || {
            for slot in order {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let picks = rank_slot(&ctx, &key.2, slot);
                if tx.send((slot, picks)).is_err() {
                    return;
                }
                repaint.request_repaint();
            }
        });
        self.job = Some(RankJob { key, rx, cancel });
    }
}

#[cfg(test)]
mod tests {
    use crate::App;
    use crate::knowledge::{Distance, Style, Surface};

    fn app_for(trainee: u32, distance: Distance, style: Style) -> App {
        let mut a = App::for_tests();
        a.planner.plan.trainee = Some(trainee);
        a.planner.plan.distance = distance;
        a.planner.plan.surface = Surface::Turf;
        a.planner.plan.style = style;
        a
    }

    #[test]
    fn recommended_deck_follows_the_rules() {
        let mut a = app_for(101601, Distance::Long, Style::Pace); // Narita Brian [Maverick]
        a.planner.plan.target = [1200, 1000, 1000, 500, 900];
        let picks = a.recommend_deck(true);
        assert_eq!(picks.len(), 6);
        for pick in &picks {
            eprintln!("{:5.1} {} [{}]: {}", pick.score, a.kb.cards[&pick.id].uma, a.kb.cards[&pick.id].title, pick.reason);
        }
        let deck: Vec<u32> = a.planner.plan.deck.iter().flatten().copied().collect();
        assert_eq!(deck.len(), 5);
        let mut names: Vec<String> = deck.iter().chain(a.planner.plan.friend.iter()).map(|id| a.kb.cards[id].uma.clone()).collect();
        assert!(!names.contains(&"Narita Brian".to_string()), "trainee's own card in deck: {names:?}");
        names.sort();
        let n = names.len();
        names.dedup();
        assert_eq!(names.len(), n, "two cards of one character");
        assert!(deck.iter().all(|id| a.owned_card(*id).is_some()), "deck slots must be owned cards");
        assert_eq!(a.planner.plan.friend_lb, 4);
    }

    #[test]
    fn stamina_cards_rank_higher_for_long_targets() {
        // Where the best owned Stamina card lands in an empty deck's first-slot ranking.
        let stamina_rank = |distance: Distance, target: [u32; 5]| {
            let mut a = app_for(101601, distance, Style::Pace);
            a.planner.plan.target = target;
            let picks = a.slot_candidates(0);
            picks.iter().position(|p| a.kb.cards[&p.id].kind == "Stamina").unwrap_or(picks.len())
        };
        let long = stamina_rank(Distance::Long, [1200, 1000, 1000, 500, 900]);
        let sprint = stamina_rank(Distance::Sprint, [1600, 450, 1100, 900, 1100]);
        eprintln!("best Stamina card: #{} for Long, #{} for Sprint", long + 1, sprint + 1);
        assert!(long < sprint, "Stamina cards should rank higher for a Long target");
    }

    /// The UI's background ranking gives the same order as ranking on the spot, and fills every slot.
    #[test]
    fn background_ranking_matches_direct() {
        let mut a = app_for(101601, Distance::Long, Style::Pace);
        a.planner.plan.target = [1200, 1000, 1000, 500, 900];
        a.recommend_deck(true);
        let ctx = eframe::egui::Context::default();
        let wait = |slot: usize| {
            let start = std::time::Instant::now();
            loop {
                if let Some(picks) = a.slot_candidates_ready(slot, &ctx) {
                    return picks;
                }
                assert!(start.elapsed().as_secs() < 120, "background ranking never finished");
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        let background: Vec<u32> = wait(2).iter().map(|p| p.id).collect();
        // Every other slot gets ranked by the same job.
        for slot in 0..6 {
            assert!(!wait(slot).is_empty(), "slot {slot}");
        }
        assert!(!a.ranking_busy());
        a.planner.rank_cache.borrow_mut().clear();
        let direct: Vec<u32> = a.slot_candidates(2).iter().map(|p| p.id).collect();
        assert_eq!(background, direct);
    }

    /// Debug aid: `cargo test --release explain_ranking -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn explain_ranking() {
        let mut a = app_for(101601, Distance::Long, Style::Pace);
        a.planner.plan.target = [1200, 1000, 1000, 500, 900];
        eprintln!("scenario {}", a.plan_scenario_name());
        for p in a.slot_candidates(0).iter().take(25) {
            eprintln!("{:6.1} {:8} {} [{}]: {}", p.score, a.kb.cards[&p.id].kind, a.kb.cards[&p.id].uma, a.kb.cards[&p.id].title, p.reason);
        }
        a.recommend_deck(true);
        let c = a.plan_career().unwrap();
        eprintln!("deck stats {:?} sp {:.0} trainings {:.0} races {:.0}", c.stats_mean.map(|v| v.round()), c.sp_mean, c.trainings_mean, c.races_mean);
    }

    #[test]
    fn sprint_target_skips_stamina() {
        // Sprint with 450 Stamina wanted (as the CM 21 guide's line): no stamina card should lead.
        let mut a = app_for(100601, Distance::Sprint, Style::Pace);
        a.planner.plan.target = [1600, 450, 1100, 900, 1100];
        let picks = a.recommend_deck(true);
        let stamina = picks.iter().filter(|p| a.kb.cards[&p.id].kind == "Stamina").count();
        assert!(stamina <= 1, "sprint deck with {stamina} stamina cards");
    }

    #[test]
    fn scenario_changes_the_value_of_race_bonus() {
        // Mean race bonus of the ten best friend cards, by scenario.
        let top_race_bonus = |scenario: &str| {
            let mut a = app_for(100601, Distance::Mile, Style::Pace);
            a.planner.plan.target = [1200, 800, 1000, 600, 900];
            a.planner.plan.scenario = scenario.into();
            let picks = a.slot_candidates(5);
            picks.iter().take(10).map(|p| a.kb.cards[&p.id].effect(crate::knowledge::effect::RACE, 50).unwrap_or(0)).sum::<i32>()
        };
        let (tb, ura) = (top_race_bonus("Trackblazer"), top_race_bonus("URA Finale"));
        eprintln!("race bonus in the top 10: Trackblazer {tb}, URA {ura}");
        assert!(tb > ura, "race bonus counts for more in Trackblazer");
    }

    #[test]
    fn filling_keeps_chosen_cards() {
        let mut a = app_for(100601, Distance::Mile, Style::Pace);
        a.planner.plan.deck[0] = Some(30019); // Haru Urara Guts card, picked by hand
        a.recommend_deck(false);
        assert_eq!(a.planner.plan.deck[0], Some(30019));
        assert!(a.planner.plan.deck.iter().all(Option::is_some));
    }

    #[test]
    fn trainee_suggestions_fit_the_goal() {
        let a = app_for(100601, Distance::Long, Style::Pace);
        let best = a.trainee_candidates()[0];
        let apt = &a.kb.umas[&best].aptitude;
        let at_least_a = |g: &str| crate::knowledge::grade_rank(g) >= 7;
        assert!(at_least_a(&apt.long) && at_least_a(&apt.turf) && at_least_a(&apt.pace), "best Long Turf Pace fit is {best}");
    }

    #[test]
    fn knapsack_respects_budget_and_groups() {
        // Gold 201 (gain 1.0, cost 300) and its white 202 (0.4, 150) share a group.
        let items = [(201, 1.0, 300.0), (202, 0.4, 150.0), (301, 0.5, 160.0), (401, 0.2, 200.0)];
        let picked = super::knapsack(&items, 470.0, |id| id / 10);
        assert_eq!(picked, vec![201, 301]);
        // 1.0 L alone beats the white plus the other skill (0.9 L) when 320 SP fits only one pair.
        let picked = super::knapsack(&items, 320.0, |id| id / 10);
        assert_eq!(picked, vec![201]);
        let picked = super::knapsack(&items, 160.0, |id| id / 10);
        assert_eq!(picked, vec![301]);
    }
}
