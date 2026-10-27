//! Setup's "Paste a list": one uma or support card per line, like `Kitasan Black MLB`,
//! `Super Creek SSR LB3` or `Oguri Cap Starlight Beat 5* P5`. Each line is matched to the game
//! data by its words (typos allowed), with LB, stars, potential, rarity and type read as hints.
//! A preview shows every match with how sure it is, and lets the user fix one before adding.

use eframe::egui::{self, RichText, vec2};

use crate::App;
use crate::knowledge::Knowledge;
use crate::theme::{pal, size};
use crate::ui_util::UiExt;
use crate::ui_util::{ToastKind, draw_lb, semibold};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Target {
    Uma(u32),
    Card(u32),
}

/// One pasted line, read: the hints, and the words left for the name.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Line {
    pub text: String,
    pub lb: Option<u8>,
    pub stars: Option<u8>,
    pub potential: Option<u8>,
    pub rarity: Option<&'static str>,
    pub kind: Option<&'static str>,
    pub words: Vec<String>,
}

/// A line matched to the game data.
#[derive(Clone, Debug)]
pub(crate) struct Match {
    pub line: Line,
    pub best: Option<Target>,
    /// The next best guesses, best first, for fixing a wrong match.
    pub others: Vec<Target>,
    /// How sure the match is, 0–1.
    pub confidence: f32,
    /// Another target scored about as well.
    pub ambiguous: bool,
    /// Add it when the list is confirmed.
    pub include: bool,
}

/// Lower case, ★ as *, and everything but letters, digits and * as spaces.
fn normalise(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '★' | '☆' => '*',
            c if c.is_alphanumeric() || c == '*' => c.to_lowercase().next().unwrap_or(c),
            _ => ' ',
        })
        .collect()
}

fn words(s: &str) -> Vec<String> {
    normalise(s).split_whitespace().map(String::from).collect()
}

const KINDS: [(&str, &[&str]); 7] = [
    ("Speed", &["speed", "spd"]),
    ("Stamina", &["stamina", "stam", "sta"]),
    ("Power", &["power", "pow", "pwr"]),
    ("Guts", &["guts"]),
    ("Wit", &["wit", "wits", "wisdom", "wis", "int"]),
    ("Pal", &["pal", "friend"]),
    ("Group", &["group"]),
];

/// Read the hints out of a line; what's left is the name.
pub(crate) fn read_line(text: &str) -> Line {
    let mut line = Line {
        text: text.trim().to_string(),
        ..Default::default()
    };
    let toks = words(text);
    let digit = |s: &str| s.parse::<u8>().ok();
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i].as_str();
        let next = toks.get(i + 1).map(String::as_str);
        let mut used = 1;
        if t == "mlb" || t == "maxed" {
            line.lb = Some(4);
        } else if let Some(n) = t.strip_prefix("lb").and_then(digit).filter(|n| *n <= 4) {
            line.lb = Some(n);
        } else if t == "lb" && next.and_then(digit).is_some_and(|n| n <= 4) {
            line.lb = next.and_then(digit);
            used = 2;
        } else if matches!(t, "ssr" | "sr" | "r") {
            line.rarity = Some(match t {
                "ssr" => "SSR",
                "sr" => "SR",
                _ => "R",
            });
        } else if let Some(k) = KINDS
            .iter()
            .find(|(_, names)| names.contains(&t))
            .map(|(k, _)| *k)
        {
            line.kind = Some(k);
        } else if let Some(n) = t
            .strip_suffix('*')
            .or_else(|| t.strip_prefix('*'))
            .and_then(digit)
            .filter(|n| (1..=5).contains(n))
        {
            line.stars = Some(n);
        } else if let Some(n) = digit(t).filter(|n| (1..=5).contains(n))
            && matches!(next, Some("*" | "star" | "stars"))
        {
            line.stars = Some(n);
            used = 2;
        } else if let Some(n) = t
            .strip_suffix("star")
            .or_else(|| t.strip_suffix("stars"))
            .and_then(digit)
            .filter(|n| (1..=5).contains(n))
        {
            line.stars = Some(n);
        } else if let Some(n) = t
            .strip_prefix("pot")
            .or_else(|| t.strip_prefix('p'))
            .and_then(digit)
            .filter(|n| (1..=5).contains(n))
        {
            line.potential = Some(n);
        } else if matches!(t, "pot" | "potential")
            && next.and_then(digit).is_some_and(|n| (1..=5).contains(&n))
        {
            line.potential = next.and_then(digit);
            used = 2;
        } else if t.starts_with("lv") && t[2..].trim_start_matches('l').parse::<u32>().is_ok()
            || t == "*"
        {
            // A level ("Lv50"; setup sets the most the LB allows) or a stray star.
        } else if matches!(t, "lv" | "lvl" | "level")
            && next.and_then(|n| n.parse::<u32>().ok()).is_some()
        {
            used = 2;
        } else {
            line.words.push(t.to_string());
        }
        i += used;
    }
    line
}

/// Edit distance between two short words.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != cb))
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// How well a typed word matches a word of a name: exact, a prefix, or a typo or two.
fn word_score(q: &str, c: &str) -> f32 {
    if q == c {
        1.0
    } else if q.len() >= 3 && c.starts_with(q) {
        0.9
    } else if q.chars().count() >= 4 && levenshtein(q, c) <= 1 {
        0.85
    } else if q.chars().count() >= 7 && levenshtein(q, c) <= 2 {
        0.7
    } else {
        0.0
    }
}

/// A candidate's words: the character's name, then the outfit or card title.
struct Candidate {
    target: Target,
    name: Vec<String>,
    all: Vec<String>,
    rarity: &'static str,
    kind: &'static str,
    release: String,
}

fn candidates(kb: &Knowledge, today: &str) -> Vec<Candidate> {
    let mut out = Vec::new();
    for u in kb.umas.values().filter(|u| u.release.as_str() <= today) {
        let name = words(&u.name);
        let mut all = name.clone();
        all.extend(words(&u.outfit));
        out.push(Candidate {
            target: Target::Uma(u.id),
            name,
            all,
            rarity: "",
            kind: "",
            release: u.release.clone(),
        });
    }
    for c in kb.cards.values().filter(|c| c.released(today)) {
        let name = words(&c.uma);
        let mut all = name.clone();
        all.extend(words(&c.title));
        let rarity = match c.rarity.as_str() {
            "SSR" => "SSR",
            "SR" => "SR",
            _ => "R",
        };
        let kind = KINDS
            .iter()
            .find(|(k, _)| *k == c.kind)
            .map_or("", |(k, _)| *k);
        out.push(Candidate {
            target: Target::Card(c.id),
            name,
            all,
            rarity,
            kind,
            release: c.release.clone(),
        });
    }
    // A fixed order, so ties break the same way every time.
    out.sort_by(|a, b| {
        a.release
            .cmp(&b.release)
            .then(format!("{:?}", a.target).cmp(&format!("{:?}", b.target)))
    });
    out
}

/// How well a line fits a candidate, 0 to about 1.1.
fn score(line: &Line, c: &Candidate) -> f32 {
    if line.words.is_empty() {
        return 0.0;
    }
    let best = |q: &String| {
        c.all
            .iter()
            .map(|w| word_score(q, w))
            .fold(0.0f32, f32::max)
    };
    let mean = line.words.iter().map(best).sum::<f32>() / line.words.len() as f32;
    let named = c
        .name
        .iter()
        .filter(|n| line.words.iter().any(|q| word_score(q, n) >= 0.7))
        .count() as f32
        / c.name.len().max(1) as f32;
    let mut s = 0.75 * mean + 0.25 * named;
    match c.target {
        Target::Card(_) => {
            // Hints that only cards have.
            if let Some(r) = line.rarity {
                s += if r == c.rarity { 0.08 } else { -0.15 };
            } else if line.lb.is_some() {
                // "Kitasan Black MLB" means the SSR people chase, if there is one.
                s += match c.rarity {
                    "SSR" => 0.03,
                    "SR" => 0.015,
                    _ => 0.0,
                };
            }
            if let Some(k) = line.kind {
                s += if k == c.kind { 0.08 } else { -0.1 };
            }
            if line.stars.is_some() || line.potential.is_some() {
                s -= 0.3;
            }
        }
        Target::Uma(_) => {
            if line.lb.is_some() || line.rarity.is_some() || line.kind.is_some() {
                s -= 0.3;
            }
        }
    }
    s
}

/// Match one line against prepared candidates.
fn match_with(cands: &[Candidate], line: Line) -> Match {
    let mut scored: Vec<(Target, f32)> = cands
        .iter()
        .map(|c| (c.target, score(&line, c)))
        .filter(|(_, s)| *s > 0.25)
        .collect();
    // Stable: equal scores keep the candidates' fixed order.
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    let best = scored.first().copied();
    let ambiguous = matches!((best, scored.get(1)), (Some((_, a)), Some((_, b))) if a - b < 0.04);
    let mut confidence = best.map_or(0.0, |(_, s)| s.clamp(0.0, 1.0));
    if ambiguous {
        confidence *= 0.75;
    }
    Match {
        others: scored.iter().skip(1).take(8).map(|(t, _)| *t).collect(),
        best: best.map(|(t, _)| t),
        confidence,
        ambiguous,
        include: confidence >= 0.6,
        line,
    }
}

/// Match a whole pasted list; blank lines are skipped.
pub(crate) fn match_list(kb: &Knowledge, today: &str, text: &str) -> Vec<Match> {
    let cands = candidates(kb, today);
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| match_with(&cands, read_line(l)))
        .collect()
}

/// What a target is called, e.g. "Oguri Cap [Starlight Beat]" or "SSR Kitasan Black [Fire at My Heels] (Speed)".
pub(crate) fn target_label(kb: &Knowledge, t: Target) -> String {
    match t {
        Target::Uma(id) => kb.umas.get(&id).map_or(format!("Uma {id}"), |u| {
            format!("{} [{}]", u.name, u.outfit)
        }),
        Target::Card(id) => kb.cards.get(&id).map_or(format!("Card {id}"), |c| {
            format!("{} {} [{}] ({})", c.rarity, c.uma, c.title, c.kind)
        }),
    }
}

#[derive(Default)]
pub(crate) struct PasteState {
    pub open: bool,
    pub text: String,
    pub rows: Vec<Match>,
    /// The text the rows were matched from.
    matched: String,
}

impl App {
    /// The "Paste a list" dialog.
    pub(crate) fn paste_dialog(&mut self, ctx: &egui::Context) {
        if !self.onboarding.paste.open {
            return;
        }
        if self.onboarding.paste.text != self.onboarding.paste.matched {
            let today = crate::ui_util::today();
            let text = self.onboarding.paste.text.clone();
            self.onboarding.paste.rows = match_list(&self.kb, &today, &text);
            self.onboarding.paste.matched = text;
        }
        let screen = ctx.content_rect();
        let size = vec2(
            (screen.width() - 60.0).min(900.0),
            (screen.height() - 60.0).min(640.0),
        );
        let mut add = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::unique("paste_list")).show(ctx, |ui| {
            let p = pal(ui);
            ui.set_width(size.x);
            ui.set_max_height(size.y);
            ui.label(semibold(t!("📋 Paste a list"), size::XL));
            ui.label(
                RichText::new(t!(
                    "One uma or card per line, like \"Kitasan Black MLB\", \"Super Creek SSR LB3\" or \"Oguri Cap Starlight Beat 5* P5\". \
                     Typos are fine. Check the matches below, fix any that are wrong, then add them."
                ))
                .color(p.weak),
            );
            ui.add_space(6.0);
            ui.add(
                egui::TextEdit::multiline(&mut self.onboarding.paste.text)
                    .hint_text("Kitasan Black MLB\nSuper Creek SSR LB3\nOguri Cap Starlight Beat 5* P5")
                    .desired_rows(6)
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(6.0);
            let rows_h = (size.y - 300.0).max(120.0);
            let (brush_lb, brush_stars, brush_pot) = (self.onboarding.brush_lb, self.onboarding.brush_stars, self.onboarding.brush_pot);
            egui::ScrollArea::vertical().max_height(rows_h).auto_shrink([false, true]).show(ui, |ui| {
                egui::Grid::new("paste_rows").num_columns(5).striped(true).spacing([12.0, 6.0]).show(ui, |ui| {
                    for h in ["", "You wrote", "Matched to", "Adds as", "Sure?"] {
                        ui.label(RichText::new(t!(h)).size(size::SM).color(p.weak));
                    }
                    ui.end_row();
                    let kb = &self.kb;
                    for (i, m) in self.onboarding.paste.rows.iter_mut().enumerate() {
                        ui.tick(&mut m.include, "");
                        ui.label(RichText::new(crate::ui_util::truncate(&m.line.text, 34)).size(13.0)).on_hover_text(&m.line.text);
                        let current = m.best.map_or_else(|| t!("No match: pick one").to_string(), |t| target_label(kb, t));
                        let others = m.others.clone();
                        let want_card = m.line.lb.is_some() || m.line.rarity.is_some();
                        let picked = crate::ui_util::search_combo(
                            ui,
                            ("paste_pick", i),
                            crate::ui_util::truncate(&current, 46),
                            300.0,
                            || {
                                let mut items: crate::ui_util::ComboItems<Target> = vec![(None, t!("Best guesses").into(), String::new())];
                                items.extend(others.iter().map(|t| (Some(*t), target_label(kb, *t), String::new())));
                                items.push((None, if want_card { t!("Every support card").into() } else { t!("Every uma, then every card").into() }, String::new()));
                                let mut umas: Vec<_> = kb.umas.values().map(|u| (Some(Target::Uma(u.id)), format!("{} [{}]", u.name, u.outfit), String::new())).collect();
                                umas.sort_by(|a, b| a.1.cmp(&b.1));
                                let mut cards: Vec<_> = kb.cards.values().map(|c| (Some(Target::Card(c.id)), target_label(kb, Target::Card(c.id)), String::new())).collect();
                                cards.sort_by(|a, b| a.1.cmp(&b.1));
                                if !want_card {
                                    items.extend(umas);
                                }
                                items.extend(cards);
                                items
                            },
                            m.best.as_ref(),
                        );
                        if let Some(t) = picked {
                            m.best = Some(t);
                            m.confidence = 1.0;
                            m.ambiguous = false;
                            m.include = true;
                        }
                        match m.best {
                            Some(Target::Card(_)) => {
                                let lb = m.line.lb.unwrap_or(brush_lb);
                                let (rect, _) = ui.allocate_exact_size(vec2(80.0, 16.0), egui::Sense::hover());
                                draw_lb(ui.painter(), rect.left_center(), lb, 5.0);
                            }
                            Some(Target::Uma(_)) => {
                                ui.label(format!("{}★ P{}", m.line.stars.unwrap_or(brush_stars), m.line.potential.unwrap_or(brush_pot)));
                            }
                            None => {
                                ui.label("—");
                            }
                        }
                        let (text, color) = if m.best.is_none() {
                            (t!("no match").to_string(), p.bad)
                        } else if m.ambiguous {
                            (t!("unsure: check it").to_string(), p.warn)
                        } else if m.confidence >= 0.8 {
                            (format!("{:.0}%", m.confidence * 100.0), p.good)
                        } else {
                            (format!("{:.0}%", m.confidence * 100.0), p.warn)
                        };
                        ui.label(RichText::new(text).size(13.0).color(color));
                        ui.end_row();
                    }
                });
            });
            ui.add_space(8.0);
            let n = self.onboarding.paste.rows.iter().filter(|m| m.include && m.best.is_some()).count();
            ui.horizontal(|ui| {
                if ui.add_enabled(n > 0, crate::ui_util::primary_button(ui, tf!("＋ Add {} to my collection", n), size::BODY).min_size(vec2(0.0, 32.0))).clicked() {
                    add = true;
                }
                if ui.button(t!("Cancel")).clicked() {
                    cancel = true;
                }
                ui.label(RichText::new(t!("Missing values come from \"New picks start as\". Ctrl+Z undoes the whole list.")).size(size::SM).color(p.weak));
            });
        });
        if add {
            let (lb, stars, pot) = (
                self.onboarding.brush_lb,
                self.onboarding.brush_stars,
                self.onboarding.brush_pot,
            );
            let edits: Vec<crate::onboarding::Edit> = self
                .onboarding
                .paste
                .rows
                .iter()
                .filter(|m| m.include)
                .filter_map(|m| match m.best? {
                    Target::Uma(id) => Some(crate::onboarding::Edit::Uma {
                        id,
                        stars: m.line.stars.unwrap_or(stars),
                        potential: m.line.potential.unwrap_or(pot),
                    }),
                    Target::Card(id) => Some(crate::onboarding::Edit::Card {
                        id,
                        lb: m.line.lb.unwrap_or(lb),
                        level: None,
                    }),
                })
                .collect();
            match self.apply_edits(&edits, true) {
                Ok(n) => {
                    self.toast_action(
                        tf!("Added or updated {} from your list.", n),
                        ToastKind::Success,
                        t!("Undo"),
                        crate::ui_util::ToastAction::Undo,
                    );
                    self.onboarding.paste = PasteState::default();
                }
                Err(e) => self.toast(e, ToastKind::Error),
            }
        } else if cancel || modal.should_close() {
            self.onboarding.paste.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb() -> Knowledge {
        Knowledge::load(Some(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap(),
        ))
    }

    fn card(kb: &Knowledge, uma: &str, rarity: &str, kind: &str) -> Target {
        let mut ids: Vec<u32> = kb
            .cards
            .values()
            .filter(|c| c.uma == uma && c.rarity == rarity && c.kind == kind)
            .map(|c| c.id)
            .collect();
        ids.sort_unstable();
        Target::Card(
            *ids.first()
                .unwrap_or_else(|| panic!("no {rarity} {kind} {uma}")),
        )
    }

    fn uma(kb: &Knowledge, name: &str, outfit: &str) -> Target {
        Target::Uma(
            kb.umas
                .values()
                .find(|u| u.name == name && u.outfit == outfit)
                .unwrap_or_else(|| panic!("no {name} [{outfit}]"))
                .id,
        )
    }

    #[test]
    fn reads_hints() {
        let l = read_line("Kitasan Black MLB");
        assert_eq!(
            (l.lb, l.words.clone()),
            (Some(4), vec!["kitasan".to_string(), "black".to_string()])
        );
        let l = read_line("Super Creek SSR LB3");
        assert_eq!((l.lb, l.rarity), (Some(3), Some("SSR")));
        let l = read_line("Oguri Cap Starlight Beat 5* P5");
        assert_eq!((l.stars, l.potential, l.words.len()), (Some(5), Some(5), 4));
        let l = read_line("oguri cap 4★ pot 3 lv 50");
        assert_eq!((l.stars, l.potential, l.words.len()), (Some(4), Some(3), 2));
        let l = read_line("Fine Motion wit lb 2");
        assert_eq!((l.kind, l.lb), (Some("Wit"), Some(2)));
        let l = read_line("Seiun Sky 3 stars");
        assert_eq!(l.stars, Some(3));
    }

    /// About twenty lines, typos and ambiguous names included.
    #[test]
    fn matches_a_pasted_list() {
        let kb = kb();
        let today = "2026-10-18";
        let lines = [
            (
                "Kitasan Black MLB",
                Some(card(&kb, "Kitasan Black", "SSR", "Speed")),
                false,
            ),
            (
                "Kitasn Blck mlb",
                Some(card(&kb, "Kitasan Black", "SSR", "Speed")),
                false,
            ),
            (
                "Super Creek SSR LB3",
                Some(card(&kb, "Super Creek", "SSR", "Stamina")),
                false,
            ),
            (
                "Silence Suzuka SSR speed LB4",
                Some(card(&kb, "Silence Suzuka", "SSR", "Speed")),
                false,
            ),
            (
                "Fine Motion wit mlb",
                Some(card(&kb, "Fine Motion", "SSR", "Wit")),
                false,
            ),
            (
                "Narita Brian Two Pieces lb2",
                Some(Target::Card(30044)),
                false,
            ),
            (
                "Mihono Bourbon U & Me MLB",
                Some(Target::Card(30121)),
                false,
            ),
            ("Q≠0 Agnes Digital MLB", Some(Target::Card(30101)), false),
            (
                "Oguri Cap Starlight Beat 5* P5",
                Some(uma(&kb, "Oguri Cap", "Starlight Beat")),
                false,
            ),
            (
                "oguri cap starlite beat 4★ p3",
                Some(uma(&kb, "Oguri Cap", "Starlight Beat")),
                false,
            ),
            (
                "Seiun Sky Reeling in the Big One 5*",
                Some(uma(&kb, "Seiun Sky", "Reeling in the Big One")),
                false,
            ),
            (
                "Agnes Tachyon tach-nology P5",
                Some(uma(&kb, "Agnes Tachyon", "tach-nology")),
                false,
            ),
            (
                "Sakura Bakushin O Blossom in Learning 3*",
                Some(uma(&kb, "Sakura Bakushin O", "Blossom in Learning")),
                false,
            ),
            (
                "Gold Ship Red Strife P2",
                Some(uma(&kb, "Gold Ship", "Red Strife")),
                false,
            ),
            (
                "lv50 Kitasan Black LB4",
                Some(card(&kb, "Kitasan Black", "SSR", "Speed")),
                false,
            ),
            (
                "Winning Ticket Get to Winning! 4 stars",
                Some(uma(&kb, "Winning Ticket", "Get to Winning!")),
                false,
            ),
            ("Special Week", None, true),
            ("Gold Ship", None, true),
            ("Tokai Teio", None, true),
            ("zzzz qqqq", None, false),
        ];
        let text: String = lines.iter().map(|(l, _, _)| format!("{l}\n\n")).collect();
        let matches = match_list(&kb, today, &text);
        assert_eq!(matches.len(), lines.len(), "blank lines are skipped");
        for (m, (line, want, ambiguous)) in matches.iter().zip(&lines) {
            let got = m.best.map(|t| target_label(&kb, t));
            if let Some(want) = want {
                assert_eq!(
                    m.best,
                    Some(*want),
                    "{line}: got {got:?}, sure {:.2}",
                    m.confidence
                );
                assert!(
                    m.include,
                    "{line}: a good match is ticked ({:.2})",
                    m.confidence
                );
            }
            if *ambiguous {
                assert!(m.ambiguous, "{line}: should be flagged unsure, got {got:?}");
            }
        }
        let none = matches.last().unwrap();
        assert!(
            none.best.is_none() && !none.include,
            "nonsense matches nothing"
        );
    }
}
