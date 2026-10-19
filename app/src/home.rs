//! What Home shows that takes work to find: the next Champions Meeting's community meta, your best
//! trainees and cards for it, your plan's progress, the next three weeks of events, recent plans
//! and results, badges and tips. Built once per change (`HomeCache::key`), never per frame.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::App;
use crate::knowledge::{EventPreset, Style};
use crate::live::{Announcement, Kind};
use crate::tab_planner::{PLAN_MARKER, Plan, Purpose};

/// What kind of event a timeline bar is; each has a fixed series colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Cat {
    Cm,
    Legend,
    Heroes,
    Event,
    Scouts,
    Campaign,
}

impl Cat {
    pub const ALL: [Cat; 6] = [Cat::Cm, Cat::Legend, Cat::Heroes, Cat::Event, Cat::Scouts, Cat::Campaign];

    pub fn label(self) -> &'static str {
        match self {
            Cat::Cm => "Champions Meeting",
            Cat::Legend => "Legend Race",
            Cat::Heroes => "League of Heroes",
            Cat::Event => "Story and events",
            Cat::Scouts => "Scouts (banners)",
            Cat::Campaign => "Campaigns",
        }
    }

    /// Its slot in `charts::series`.
    pub fn slot(self) -> usize {
        match self {
            Cat::Cm => 0,
            Cat::Legend => 1,
            Cat::Event => 2,
            Cat::Campaign => 3,
            Cat::Scouts => 4,
            Cat::Heroes => 6,
        }
    }
}

/// One bar on the "Coming up" timeline.
#[derive(Clone, Debug)]
pub(crate) struct TimelineItem {
    pub name: String,
    pub start: i64,
    pub end: i64,
    pub cat: Cat,
    pub article: Option<u32>,
    /// More for the hover, e.g. a Legend Race's conditions.
    pub detail: String,
    /// Umalator's guess, not announced.
    pub estimated: bool,
}

/// The community meta for the next CM: its top style, the ranking and the recommended stats.
#[derive(Clone, Debug)]
pub(crate) struct CmMeta {
    pub ranking: Vec<String>,
    pub top: Style,
    pub stats: Vec<String>,
}

/// How far a saved plan for the next CM has got.
#[derive(Clone, Debug)]
pub(crate) struct PlanProgress {
    pub path: PathBuf,
    pub title: String,
    pub trainee: Option<u32>,
    /// Cards chosen, of six (five own plus the friend).
    pub deck: usize,
    pub skills: usize,
    pub simulated: bool,
}

/// A badge on the trainer card: (icon, name, how to earn it, earned).
pub(crate) type Badge = (&'static str, &'static str, &'static str, bool);

#[derive(Clone, Default)]
pub(crate) struct HomeCache {
    pub key: u64,
    pub cm: Option<EventPreset>,
    pub meta: Option<CmMeta>,
    /// Your best trainees for the next CM in its top style.
    pub best: Vec<u32>,
    /// Owned cards the sheet backs strongly (★★★) for the next CM's top style, strongest first.
    pub showcase: Vec<(u32, f32)>,
    /// Cards in that style's decks you don't own, with the strongest tier that runs them.
    pub wishlist: Vec<(u32, &'static str)>,
    pub plan: Option<PlanProgress>,
    /// Saved plans, newest first: (path, title, modified).
    pub recent_plans: Vec<(PathBuf, String, i64)>,
    /// The newest row of each results log: (log, date, summary).
    pub recent_results: Vec<(&'static str, String, String)>,
    pub timeline: Vec<TimelineItem>,
    pub badges: Vec<Badge>,
    pub trainer_since: Option<String>,
    /// Owned and released: umas, SSR cards, SR cards.
    pub completion: [(usize, usize); 3],
    /// Owned umas with A or better at each distance (rows) and style (columns).
    pub aptitude: Vec<Vec<usize>>,
    pub aptitude_names: Vec<Vec<String>>,
    /// (term, explanation, source) for "Did you know?".
    pub tips: Vec<(String, String, String)>,
}

#[derive(Default)]
pub(crate) struct HomeState {
    pub cache: Option<HomeCache>,
    /// Steps the daily picks on by one each, for the ↻ buttons.
    pub uma_offset: usize,
    pub tip_offset: usize,
    /// When Home first showed this session, for the count-up.
    pub shown_at: Option<f64>,
}

/// "The story event Happy New Future! A Sentimental Sunrise is coming soon!" → "Happy New Future! A Sentimental Sunrise".
fn clean_title(title: &str) -> String {
    let mut t = title.trim();
    for prefix in ["The story event ", "The race event ", "The event ", "New ", "The "] {
        if let Some(rest) = t.strip_prefix(prefix) {
            t = rest;
        }
    }
    let lower = t.to_ascii_lowercase();
    let cut = [" is coming", " is here", " has begun", " now available", " out now", " coming soon", " is set to", " has ended", " are here"]
        .iter()
        .filter_map(|s| lower.find(s))
        .min()
        .unwrap_or(t.len());
    t[..cut].trim().trim_matches('"').trim_end_matches('!').trim().to_string()
}

/// What kind of event an announcement is, if it belongs on the timeline.
fn category(a: &Announcement) -> Option<Cat> {
    if a.id >= 100_000 || a.title.contains("has ended") || a.title.starts_with("Issue") {
        return None;
    }
    let t = a.title.to_ascii_lowercase();
    Some(match a.kind {
        Kind::ChampionsMeeting => return None,
        Kind::LegendRace => Cat::Legend,
        Kind::LeagueOfHeroes => Cat::Heroes,
        Kind::Other if t.contains("scout") => Cat::Scouts,
        Kind::Other if t.contains("event") => Cat::Event,
        Kind::Other => Cat::Campaign,
    })
}

/// The next `days` of events: Champions Meetings from the merged schedule, then everything in the
/// news with a date range.
pub(crate) fn timeline(cms: &[&EventPreset], news: &[Announcement], now: i64, days: i64) -> Vec<TimelineItem> {
    let horizon = now + days * 86_400;
    let mut out: Vec<TimelineItem> = Vec::new();
    for e in cms {
        let estimated = !e.announced;
        let (start, end) = match (e.league_selection.or(e.round1), e.ends) {
            (Some(s), Some(en)) => (s, en),
            (Some(s), None) => (s, e.round1.unwrap_or(s) + 6 * 86_400),
            (None, _) => {
                let Some(d) = crate::live::date_epoch(&e.date) else { continue };
                (d - 3 * 86_400, d + 6 * 86_400)
            }
        };
        if end > now && start < horizon {
            let detail = format!("{} · {}{}", e.course_name, e.conditions, if estimated { "\nEstimated by Umalator: not announced yet" } else { "" });
            out.push(TimelineItem { name: format!("CM: {}", e.name), start, end, cat: Cat::Cm, article: e.article, detail, estimated });
        }
    }
    for a in news {
        let Some(cat) = category(a) else { continue };
        let (Some(start), Some(end)) = (a.periods.iter().map(|p| p.0).min().or(a.starts), a.periods.iter().map(|p| p.1).max()) else { continue };
        if end <= now || start >= horizon {
            continue;
        }
        let name = match (cat, &a.conditions) {
            (Cat::Legend, Some(c)) => format!("Legend Race: {}", c.race.clone().unwrap_or_else(|| format!("{} {}m", c.track, c.distance))),
            _ => clean_title(&a.title),
        };
        let detail = a.conditions.as_ref().map(|c| c.text.clone()).unwrap_or_default();
        // "Coming soon" and "out now" announcements of the same thing.
        if out.iter().any(|o| o.name == name && o.start == start) {
            continue;
        }
        out.push(TimelineItem { name, start, end, cat, article: Some(a.id), detail, estimated: false });
    }
    out.sort_by_key(|i| (i.start, i.end));
    out
}

impl App {
    /// A cheap fingerprint of what Home's cache depends on.
    fn home_key(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for u in &self.roster.umas {
            (u.id, u.stars, u.potential).hash(&mut h);
        }
        for c in &self.roster.cards {
            (c.id, c.lb, c.level).hash(&mut h);
        }
        self.roster.profile.hash(&mut h);
        self.planner.files.len().hash(&mut h);
        self.live.data.as_ref().map(|d| d.checked_at).hash(&mut h);
        self.kb.umalator_presets.len().hash(&mut h);
        crate::ui_util::today().hash(&mut h);
        h.finish()
    }

    /// Throw the cache away (a plan was saved, a result logged).
    pub(crate) fn home_dirty(&mut self) {
        self.home.cache = None;
    }

    /// Home's derived data, rebuilt only when what it depends on changed.
    pub(crate) fn home_cache(&mut self) -> HomeCache {
        let key = self.home_key();
        if let Some(c) = self.home.cache.as_ref().filter(|c| c.key == key) {
            return c.clone();
        }
        let c = self.build_home(key);
        self.home.cache = Some(c.clone());
        c
    }

    fn build_home(&mut self, key: u64) -> HomeCache {
        let today = crate::ui_util::today();
        let now = crate::live::now();
        let mut c = HomeCache { key, ..Default::default() };
        let cms: Vec<EventPreset> = self.kb.upcoming_cms(&today).into_iter().cloned().collect();
        c.cm = cms.first().cloned();
        // The meta for the next CM, and the picks for its top style.
        if let Some(cm) = c.cm.clone() {
            let ev = self.kb.pvp_meta.as_ref().and_then(|m| m.events.iter().find(|e| e.series == "CM" && e.number == cm.id).cloned());
            if let Some(ev) = &ev {
                let ranking: Vec<String> = ev.style_ranking.split('>').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                let top = ranking.first().and_then(|s| Style::ALL.into_iter().find(|st| crate::meta::style_key(*st) == s)).unwrap_or(Style::Pace);
                c.meta = Some(CmMeta { ranking, top, stats: ev.stats.clone() });
            }
            let mut plan = Plan::default();
            plan.apply_cm(&self.kb, &cm);
            if let Some(m) = &c.meta {
                plan.style = m.top;
            }
            let (distance, style) = (plan.distance, plan.style);
            let saved = std::mem::replace(&mut self.planner.plan, plan);
            // Those who can run it (B or better at the distance and style) first, in the
            // planner's order (the sheet's picks, then aptitude).
            let fits = |id: &u32| {
                self.kb.umas.get(id).is_some_and(|u| {
                    crate::knowledge::grade_rank(u.aptitude.distance(distance)) >= 6 && crate::knowledge::grade_rank(u.aptitude.style(style)) >= 6
                })
            };
            let ranked = self.trainee_candidates();
            c.best = ranked.iter().copied().filter(fits).chain(ranked.iter().copied().filter(|id| !fits(id))).take(3).collect();
            let meta = self.plan_card_meta();
            self.planner.plan = saved;
            self.planner.key_memo.set(None);
            let mut showcase: Vec<(u32, f32)> = meta.owned.iter().filter(|(id, (w, _))| crate::meta::meta_rank(*w) == 3 && self.owned_card(**id).is_some()).map(|(id, (w, _))| (*id, *w)).collect();
            showcase.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            c.showcase = showcase;
            if let (Some(ev), Some(m)) = (&ev, &c.meta)
                && let Some(st) = ev.styles.get(crate::meta::style_key(m.top))
            {
                for tier in crate::meta::TIERS {
                    let Some(deck) = st.decks.get(tier) else { continue };
                    for card in deck.cards.iter().chain(deck.borrow.iter()) {
                        if let Some(id) = card.id
                            && self.owned_card(id).is_none()
                            && self.kb.cards.contains_key(&id)
                            && !c.wishlist.iter().any(|(w, _)| *w == id)
                        {
                            c.wishlist.push((id, tier));
                        }
                    }
                }
            }
        }
        // Saved plans: newest first, and the one for the next CM.
        let mut plans: Vec<(PathBuf, String, i64, Option<Plan>, String)> = Vec::new();
        for (path, title) in &self.planner.files {
            let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let plan = text.split(PLAN_MARKER).nth(1).and_then(|t| t.split("-->").next()).and_then(|j| serde_json::from_str::<Plan>(j).ok());
            plans.push((path.clone(), title.clone(), modified, plan, text));
        }
        plans.sort_by(|a, b| b.2.cmp(&a.2));
        c.recent_plans = plans.iter().take(4).map(|(p, t, m, _, _)| (p.clone(), t.clone(), *m)).collect();
        let simulated = |text: &str| text.contains("https://umalator.app/#") || text.contains("Gap, Uma 2 ahead");
        if let Some(cm) = &c.cm {
            c.plan = plans.iter().find_map(|(path, title, _, plan, text)| {
                let p = plan.as_ref()?;
                let linked = self.kb.find_cm(p.cm_date.as_deref()?, p.umalator_course, p.cm_name.as_deref())?;
                (linked.name == cm.name && linked.date == cm.date).then(|| PlanProgress {
                    path: path.clone(),
                    title: title.clone(),
                    trainee: p.trainee,
                    deck: p.deck.iter().filter(|d| d.is_some()).count() + usize::from(p.friend.is_some()),
                    skills: p.skills.len(),
                    simulated: simulated(text),
                })
            });
        }
        // The newest row of each results log.
        let mut logged = 0;
        if let Some(root) = &self.root {
            for (label, file) in crate::tab_results::LOGS {
                let text = std::fs::read_to_string(root.join(file)).unwrap_or_default();
                let Some(t) = crate::data::tables(&text).into_iter().next() else { continue };
                let rows: Vec<&Vec<String>> = t.rows.iter().filter(|r| r.iter().any(|x| !x.is_empty())).collect();
                logged += rows.len();
                if let Some(r) = rows.first() {
                    let date = t.headers.iter().position(|h| h.starts_with("Date")).and_then(|i| r.get(i)).cloned().unwrap_or_default();
                    let summary: Vec<String> = t
                        .headers
                        .iter()
                        .zip(r.iter())
                        .filter(|(h, v)| !h.starts_with("Date") && !v.is_empty())
                        .take(3)
                        .map(|(h, v)| format!("{h}: {}", crate::ui_util::truncate(v, 28)))
                        .collect();
                    c.recent_results.push((label, date, summary.join(" · ")));
                }
            }
        }
        c.recent_results.sort_by(|a, b| b.1.cmp(&a.1));
        // The next three weeks.
        let news = self.live.data.as_ref().map(|d| d.announcements.clone()).unwrap_or_default();
        let cm_refs: Vec<&EventPreset> = cms.iter().collect();
        c.timeline = timeline(&cm_refs, &news, now, 21);
        // The stable.
        let released_umas = self.kb.umas.values().filter(|u| u.release.as_str() <= today.as_str()).count();
        let released = |r: &str| self.kb.cards.values().filter(|x| x.rarity == r && x.released(&today)).count();
        let owned = |r: crate::data::Rarity| self.roster.cards.iter().filter(|x| x.rarity == r).count();
        c.completion = [
            (self.roster.umas.len(), released_umas),
            (owned(crate::data::Rarity::SSR), released("SSR")),
            (owned(crate::data::Rarity::SR), released("SR")),
        ];
        use crate::knowledge::{Distance, grade_rank};
        c.aptitude = vec![vec![0; 4]; 4];
        c.aptitude_names = vec![vec![String::new(); 4]; 4];
        let mut names: Vec<Vec<Vec<String>>> = vec![vec![Vec::new(); 4]; 4];
        for u in &self.roster.umas {
            let Some(info) = self.kb.umas.get(&u.id) else { continue };
            for (i, d) in Distance::ALL.into_iter().enumerate() {
                for (j, s) in Style::ALL.into_iter().enumerate() {
                    if grade_rank(info.aptitude.distance(d)) >= 7 && grade_rank(info.aptitude.style(s)) >= 7 {
                        c.aptitude[i][j] += 1;
                        names[i][j].push(u.name.clone());
                    }
                }
            }
        }
        for i in 0..4 {
            for j in 0..4 {
                names[i][j].sort();
                names[i][j].dedup();
                c.aptitude_names[i][j] = names[i][j].join(", ");
            }
        }
        // The trainer card.
        let cards = &self.roster.cards;
        let full_house = crate::data::CardType::ALL[..5].iter().all(|t| cards.iter().any(|x| x.card_type == *t && x.rarity == crate::data::Rarity::SSR && x.is_mlb()));
        let cm_plan = plans.iter().any(|(_, _, _, p, _)| p.as_ref().is_some_and(|p| p.purpose == Purpose::ChampionsMeeting));
        let sim = plans.iter().any(|(_, _, _, _, t)| simulated(t)) || self.sim.compare.is_some();
        c.badges = vec![
            ("🐴", "Stable of 50", "Own 50 umas", self.roster.umas.len() >= 50),
            ("🃏", "Hundred cards", "Own 100 support cards", cards.len() >= 100),
            ("💎", "Full house", "A maxed (MLB) SSR of every stat type: Speed, Stamina, Power, Guts and Wit", full_house),
            ("✨", "Unlocked", "10 umas at Potential 5", self.roster.umas.iter().filter(|u| u.potential >= 5).count() >= 10),
            ("⭐", "Five stars", "A 5★ uma", self.roster.umas.iter().any(|u| u.stars >= 5)),
            ("📋", "First CM plan", "Save a plan for a Champions Meeting", cm_plan),
            ("🏁", "First race", "Race a build in the simulator", sim),
            ("🏆", "On the record", "Log a real result on Career → Results", logged > 0),
        ];
        c.trainer_since = self.root.as_ref().and_then(|r| std::fs::metadata(r.join(crate::data::UMAS_FILE)).ok()).and_then(|m| m.created().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| crate::live::utc_date(d.as_secs() as i64));
        // Tips: the sourced facts and glossary in knowledge/mechanics.md (a fact has no term);
        // uma.guide's glossary only if that file has none.
        c.tips = self
            .kb
            .tips
            .iter()
            .map(|t| (String::new(), t.text.clone(), t.source.clone()))
            .chain(self.kb.terms.iter().filter(|t| t.source != "general").map(|t| (t.term.clone(), t.meaning.clone(), t.source.clone())))
            .collect();
        if c.tips.is_empty() {
            c.tips = self
                .kb
                .glossary
                .iter()
                .filter(|g| g.description.len() > 40 && g.description.len() < 420)
                .map(|g| (g.term.clone(), g.description.clone(), "uma.guide glossary".to_string()))
                .collect();
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_read_cleanly() {
        assert_eq!(clean_title("The story event Happy New Future! A Sentimental Sunrise is coming soon!"), "Happy New Future! A Sentimental Sunrise");
        assert_eq!(clean_title("New Spotlight Pretty Derby and Spotlight Support Card Scouts coming soon!"), "Spotlight Pretty Derby and Spotlight Support Card Scouts");
        assert_eq!(clean_title("Fall G1 Celebration Part 3 now available!"), "Fall G1 Celebration Part 3");
        assert_eq!(clean_title("The Sparkly Magic Show collaboration event is here!"), "Sparkly Magic Show collaboration event");
    }

    /// From the fixture news on 2026-10-18: the Capricorn Cup, the Legend Race and the story event,
    /// each once, soonest first; the ended Sagittarius Cup isn't there.
    #[test]
    fn the_next_three_weeks() {
        let fixture = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/news_sample.json")).unwrap();
        let news = crate::live::parse_news(&fixture).unwrap();
        let kb = crate::knowledge::Knowledge::load(Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()));
        let live = crate::live::LiveData { announcements: news.clone(), ..Default::default() };
        let merged = crate::live::merge(&kb, &kb.umalator_presets, &live);
        let now = crate::live::epoch(2026, 10, 18, 9, 0);
        let cms: Vec<&EventPreset> = merged.iter().filter(|e| e.is_cm() && e.date.as_str() >= "2026-10-18").collect();
        let items = timeline(&cms, &news, now, 21);
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"CM: Capricorn Cup"), "{names:?}");
        assert!(names.contains(&"Legend Race: Arima Kinen"), "{names:?}");
        assert!(names.contains(&"Happy New Future! A Sentimental Sunrise"), "{names:?}");
        assert!(!names.iter().any(|n| n.contains("Sagittarius")), "{names:?}");
        assert!(items.windows(2).all(|w| w[0].start <= w[1].start));
        let cap = items.iter().find(|i| i.name == "CM: Capricorn Cup").unwrap();
        assert_eq!((cap.start, cap.estimated), (crate::live::epoch(2026, 10, 27, 22, 0), false), "from league selection, announced");
    }
}
