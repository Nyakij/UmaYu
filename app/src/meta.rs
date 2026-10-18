//! The community PvP meta sheet (`knowledge/data/pvp_meta.json`, from the "PvP Deck & Uma
//! Recommendation" Google Sheet by Lightning, Daptrius and Henry): recommended decks, umas, stats
//! and skills for each Champions Meeting and running style.
//!
//! The planner puts the sheet's picks first: the deck fill and the card picker rank cards the
//! sheet recommends for the plan's CM and style above the rest (the career simulation orders the
//! rest, and breaks ties), and Auto-pick buys the skills it recommends before spending what's left
//! on simulated lengths. Community opinion, not game data.

use std::collections::HashMap;

use serde::Deserialize;

use crate::App;
use crate::knowledge::Style;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PvpMeta {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub authors: String,
    #[serde(default)]
    pub fetched: String,
    #[serde(default)]
    pub caveat: String,
    #[serde(default)]
    pub events: Vec<MetaEvent>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MetaEvent {
    /// "CM" or "LoH".
    pub series: String,
    pub number: u32,
    #[serde(default)]
    pub name: String,
    /// Speed, Stamina, Power, Guts, Wit as the sheet writes them ("1100++").
    #[serde(default)]
    pub stats: Vec<String>,
    /// e.g. "Pace>End>Front>Late".
    #[serde(default)]
    pub style_ranking: String,
    #[serde(default)]
    pub analysis: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub styles: HashMap<String, MetaStyle>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MetaStyle {
    #[serde(default)]
    pub umas: Vec<MetaUma>,
    /// Acceleration advice, e.g. "1. nishino 2. Creek (HW) 3. Head-on".
    #[serde(default)]
    pub accel: String,
    #[serde(default)]
    pub advice: String,
    /// "Beginner", "Standard", "Whale".
    #[serde(default)]
    pub decks: HashMap<String, MetaDeck>,
    #[serde(default)]
    pub honorable: Vec<MetaCard>,
    #[serde(default)]
    pub skills: Vec<MetaSkill>,
    /// Umas whose inherited unique the advice names, to look for in parents.
    #[serde(default)]
    pub parents: Vec<MetaParent>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MetaDeck {
    #[serde(default)]
    pub cards: Vec<MetaCard>,
    pub borrow: Option<MetaCard>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MetaCard {
    /// None for a card not on Global yet, or a placeholder like "any Wit card".
    pub id: Option<u32>,
    pub name: String,
    /// What the card is brought for, e.g. "2SP + Gold".
    #[serde(default)]
    pub note: String,
    /// The lowest limit break the sheet allows for it (Beginner decks).
    pub min_lb: Option<u8>,
    /// The fallback when you don't have that LB, e.g. "or spd".
    #[serde(default, rename = "or")]
    pub alt: String,
    /// A placeholder slot's card type: "Wit" means any Wit card.
    #[serde(default)]
    pub any: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MetaUma {
    pub name: String,
    pub id: Option<u32>,
    #[serde(default)]
    pub rating: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MetaSkill {
    pub id: u32,
    /// "advice" (the sheet's skill text) and/or the card note it came from.
    #[serde(default, rename = "from")]
    pub why: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MetaParent {
    /// As the advice names her, e.g. "King Halo (Cheer)".
    pub uma: String,
    /// Her unique skill's inherited version.
    pub skill: u32,
}

/// Deck tiers, strongest first: what someone with every card would run, a regular player, a new one.
pub const TIERS: [&str; 3] = ["Whale", "Standard", "Beginner"];

fn tier_weight(tier: &str) -> f32 {
    match tier {
        "Whale" => 3.0,
        "Standard" => 2.0,
        _ => 1.0,
    }
}

/// The sheet's name for a running style.
pub fn style_key(s: Style) -> &'static str {
    match s {
        Style::Front => "Front",
        Style::Pace => "Pace",
        Style::Late => "Late",
        Style::End => "End",
    }
}

/// Lengths a meta skill is worth on top of its simulated gain, per unit of weight: enough that
/// Auto-pick buys every recommended skill it can afford before anything else.
pub const META_SKILL_LENGTHS: f64 = 2.0;

/// The sheet's backing as a rank: 3 for a core pick (in most of its decks), 2 recommended,
/// 1 mentioned, 0 none. Card rankings sort by this first, then by the simulation's score, so the
/// simulation still chooses among cards the sheet backs about equally.
pub fn meta_rank(weight: f32) -> u8 {
    match weight {
        w if w >= 4.0 => 3,
        w if w >= 2.0 => 2,
        w if w > 0.0 => 1,
        _ => 0,
    }
}

/// "★★★" for a rank.
pub fn stars(weight: f32) -> &'static str {
    ["", "★", "★★", "★★★"][usize::from(meta_rank(weight))]
}

/// How much an owned card's backing counts at its limit break: the sheet's decks assume a decent
/// LB (Whale decks are maxed), so a low-LB copy counts for less.
fn lb_factor(lb: u8) -> f32 {
    [0.4, 0.55, 0.7, 0.85, 1.0][usize::from(lb.min(4))]
}

/// How strongly the sheet backs each card for the plan, as (weight, why). Weights add up over the
/// tiers a card is in (Whale 3, Standard 2, Beginner 1) and honorable mentions (0.5); an owned
/// card's weight is scaled by its limit break.
#[derive(Clone, Debug, Default)]
pub(crate) struct CardMeta {
    pub owned: HashMap<u32, (f32, String)>,
    pub friend: HashMap<u32, (f32, String)>,
    /// Card type → weight, for decks that ask for "any Wit card".
    pub any: HashMap<String, (f32, String)>,
}

impl CardMeta {
    pub fn of(&self, id: u32, friend: bool, kind: &str) -> (f32, String) {
        let map = if friend { &self.friend } else { &self.owned };
        map.get(&id).or_else(|| self.any.get(kind)).cloned().unwrap_or_default()
    }
}

#[derive(Default)]
struct Backing {
    weight: f32,
    tiers: Vec<String>,
    notes: Vec<String>,
}

impl Backing {
    fn add(&mut self, w: f32, what: String, note: &str) {
        self.weight += w;
        if !self.tiers.contains(&what) {
            self.tiers.push(what);
        }
        if !note.is_empty() && !self.notes.iter().any(|n| n == note) {
            self.notes.push(note.to_string());
        }
    }
    fn why(&self, label: &str) -> String {
        let notes = if self.notes.is_empty() { String::new() } else { format!(" ({})", self.notes.join("; ")) };
        format!("meta sheet {label}: {}{notes}", self.tiers.join(", "))
    }
}

impl App {
    /// The Champions Meeting number the plan is for: its linked CM, else the CM on its course
    /// nearest to today.
    pub(crate) fn plan_cm_number(&self) -> Option<u32> {
        let p = &self.planner.plan;
        let presets = &self.kb.umalator_presets;
        if let Some(date) = p.cm_date.as_deref()
            && let Some(e) = presets.iter().find(|e| e.is_cm() && e.date == date)
        {
            return Some(e.id);
        }
        let course = self.plan_race_setup().course?;
        let today = crate::ui_util::today();
        presets
            .iter()
            .filter(|e| e.is_cm() && e.course == course)
            .min_by_key(|e| (crate::tab_planner::day_number(&e.date) - crate::tab_planner::day_number(&today)).abs())
            .map(|e| e.id)
    }

    /// The sheet's entry for the plan's Champions Meeting and running style.
    pub(crate) fn plan_meta(&self) -> Option<(&MetaEvent, &MetaStyle)> {
        let meta = self.kb.pvp_meta.as_ref()?;
        let cm = self.plan_cm_number()?;
        let ev = meta.events.iter().find(|e| e.series == "CM" && e.number == cm)?;
        let st = ev.styles.get(style_key(self.planner.plan.style))?;
        Some((ev, st))
    }

    /// The trainees the sheet recommends for the plan's CM and style, in its order.
    pub(crate) fn plan_meta_trainees(&self) -> Vec<u32> {
        self.plan_meta().map(|(_, st)| st.umas.iter().filter_map(|u| u.id).collect()).unwrap_or_default()
    }

    /// "CM 21 Capricorn Cup · Pace" for the plan's meta entry.
    pub(crate) fn plan_meta_label(&self) -> Option<String> {
        let (ev, _) = self.plan_meta()?;
        Some(format!("CM {} {} · {}", ev.number, ev.name, style_key(self.planner.plan.style)))
    }

    /// How strongly the sheet backs each card for the plan's CM and style. An owned card below
    /// the LB a Beginner deck asks for doesn't count for that deck.
    pub(crate) fn plan_card_meta(&self) -> CardMeta {
        let Some((_, st)) = self.plan_meta() else { return CardMeta::default() };
        let style = style_key(self.planner.plan.style);
        let mut owned: HashMap<u32, Backing> = HashMap::new();
        let mut friend: HashMap<u32, Backing> = HashMap::new();
        let mut any: HashMap<String, Backing> = HashMap::new();
        for tier in TIERS {
            let Some(deck) = st.decks.get(tier) else { continue };
            let w = tier_weight(tier);
            for c in &deck.cards {
                match c.id {
                    Some(id) => {
                        let lb_ok = c.min_lb.is_none_or(|m| self.owned_card(id).is_none_or(|o| o.lb >= m));
                        if lb_ok {
                            owned.entry(id).or_default().add(w, format!("{tier} {style} deck"), &c.note);
                        }
                        friend.entry(id).or_default().add(w * 0.5, format!("{tier} {style} deck"), &c.note);
                    }
                    None if !c.any.is_empty() => {
                        any.entry(c.any.clone()).or_default().add(w * 0.3, format!("{tier} {style} deck asks for any {} card", c.any), "");
                    }
                    None => {}
                }
            }
            if let Some(id) = deck.borrow.as_ref().and_then(|b| b.id) {
                let note = deck.borrow.as_ref().map_or("", |b| b.note.as_str());
                friend.entry(id).or_default().add(w, format!("{tier} {style} borrow"), note);
                owned.entry(id).or_default().add(w * 0.5, format!("{tier} {style} borrow"), note);
            }
        }
        for c in &st.honorable {
            if let Some(id) = c.id {
                owned.entry(id).or_default().add(0.5, "honorable mention".into(), &c.note);
                friend.entry(id).or_default().add(0.5, "honorable mention".into(), &c.note);
            }
        }
        let label = self.plan_meta_label().unwrap_or_default();
        let owned = owned
            .into_iter()
            .map(|(id, b)| {
                let lb = self.owned_card(id).map_or(4, |o| o.lb);
                let mut why = b.why(&label);
                if lb < 4 {
                    why += &format!("; counts {:.0}% at your LB {lb}", lb_factor(lb) * 100.0);
                }
                (id, (b.weight * lb_factor(lb), why))
            })
            .collect();
        let friend = friend.into_iter().map(|(id, b)| (id, (b.weight, b.why(&label)))).collect();
        CardMeta { owned, friend, any: any.into_iter().map(|(k, b)| (k, (b.weight, b.why(&label)))).collect() }
    }

    /// Skills the sheet recommends for the plan's CM and style, as id → (weight, why). The skill
    /// advice counts 2, what a recommended card is brought for 1, and a recommended gold's
    /// ○/white a quarter of the gold (the fallback when the gold doesn't fit the budget).
    pub(crate) fn plan_meta_skills(&self) -> HashMap<u32, (f32, String)> {
        let mut out: HashMap<u32, (f32, String)> = HashMap::new();
        let Some((_, st)) = self.plan_meta() else { return out };
        for s in &st.skills {
            let w = if s.why.starts_with("advice") { 2.0 } else { 1.0 };
            let e = out.entry(s.id).or_insert((0.0, String::new()));
            if w > e.0 {
                *e = (w, s.why.clone());
            }
            if let Some(lower) = self.lower_version(s.id) {
                let gold = self.kb.skill_name(s.id);
                let e = out.entry(lower).or_insert((0.0, String::new()));
                if w * 0.25 > e.0 {
                    *e = (w * 0.25, format!("the ○/white of {gold}"));
                }
            }
        }
        out
    }
}
