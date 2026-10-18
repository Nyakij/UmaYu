//! Ask: questions about this account, answered by Claude with tools over the roster, the
//! knowledge base and the notes. Raw HTTP through Windows' curl.exe (Rust has no official
//! Anthropic SDK, and TLS crates need a C compiler this toolchain lacks).
//!
//! The UI thread runs the tools (they read App state); a worker thread only does HTTP.

use std::path::Path;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Stroke, vec2};
use serde_json::{Value, json};

use crate::App;
use crate::knowledge::{Distance, Style, Surface, grade_rank};
use crate::tab_umas::filter_heading;
use crate::theme::pal;
use crate::ui_util::{GOOD, WARN, badge, semibold, truncate};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MAX_TOOL_ROUNDS: u32 = 12;

/// (model id, label, supports server-side refusal fallbacks)
pub const MODELS: [(&str, &str, bool); 3] = [
    ("claude-opus-5-5", "Claude Opus 5.5", true),
    ("claude-sonnet-5-5", "Claude Sonnet 5.5", true),
    ("claude-haiku-5-5", "Claude Haiku 5.5", false),
];
pub const EFFORTS: [&str; 4] = ["low", "medium", "high", "xhigh"];

enum Auth {
    ApiKey(String),
    /// OAuth access token from `ant auth print-credentials --access-token`.
    Bearer(String),
}

struct Turn {
    role: &'static str,
    text: String,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Backend {
    /// OpenCode with a free model: no key, no cost.
    OpenCode,
    /// The Claude API with the user's key.
    ClaudeApi,
}

pub(crate) struct OcState {
    pub exe: Option<std::path::PathBuf>,
    pub models: Vec<String>,
    pub models_rx: Option<Receiver<Result<Vec<String>, String>>>,
    pub model: String,
    /// OpenCode's session id, so follow-up questions keep the conversation.
    pub session: Option<String>,
    pub rx: Option<Receiver<crate::ask_opencode::OcEvent>>,
    pub child: std::sync::Arc<std::sync::Mutex<Option<std::process::Child>>>,
    /// The current answer already has a turn that new text parts append to.
    pub answer_open: bool,
    pub install_rx: Option<Receiver<crate::ask_opencode::InstallEvent>>,
    pub install_progress: u64,
    pub install_error: Option<String>,
}

pub(crate) struct AskState {
    pub backend: Backend,
    pub oc: OcState,
    /// Full API transcript, append-only (thinking blocks stay valid).
    messages: Vec<Value>,
    turns: Vec<Turn>,
    input: String,
    pending: Option<Receiver<Result<Value, String>>>,
    rounds: u32,
    model: usize,
    effort: usize,
    key_input: String,
    remember: bool,
    saved_key: Option<String>,
    status: Option<(String, bool)>,
    tokens_in: u64,
    tokens_out: u64,
    activity: Vec<String>,
}

/// Example questions shown on an empty chat.
const SUGGESTIONS: [&str; 4] = [
    "Who are my best Long-distance trainees, and which of them are Pace Chasers?",
    "Build me a Speed-heavy deck from my MLB SSRs for a Medium Turf race.",
    "Which of my support cards give a hint for a corner recovery skill?",
    "Which of my umas should I raise to Potential 5 first, and why?",
];

impl AskState {
    pub fn new(root: Option<&Path>) -> Self {
        let saved_key = crate::settings::get("api_key").and_then(|v| v.as_str().map(str::to_string)).filter(|k| !k.is_empty());
        let exe = crate::ask_opencode::find_exe(root);
        let saved = |k: &str| crate::settings::get(k).and_then(|v| v.as_str().map(str::to_string));
        let backend = match saved("ask_backend").as_deref() {
            Some("claude_api") => Backend::ClaudeApi,
            // Default to the free backend; if OpenCode isn't installed the tab offers the download.
            _ => Backend::OpenCode,
        };
        Self {
            backend,
            oc: OcState {
                exe,
                models: Vec::new(),
                models_rx: None,
                model: saved("opencode_model").unwrap_or_else(|| crate::ask_opencode::DEFAULT_MODEL.to_string()),
                session: None,
                rx: None,
                child: Default::default(),
                answer_open: false,
                install_rx: None,
                install_progress: 0,
                install_error: None,
            },
            messages: Vec::new(),
            turns: Vec::new(),
            input: String::new(),
            pending: None,
            rounds: 0,
            model: 0,
            effort: 2,
            key_input: String::new(),
            remember: false,
            saved_key,
            status: None,
            tokens_in: 0,
            tokens_out: 0,
            activity: Vec::new(),
        }
    }

    fn auth(&self) -> Option<Auth> {
        if let Ok(k) = std::env::var("ANTHROPIC_API_KEY")
            && !k.trim().is_empty()
        {
            return Some(Auth::ApiKey(k.trim().to_string()));
        }
        if !self.key_input.trim().is_empty() {
            return Some(Auth::ApiKey(self.key_input.trim().to_string()));
        }
        if let Some(k) = &self.saved_key {
            return Some(Auth::ApiKey(k.clone()));
        }
        ant_token().map(Auth::Bearer)
    }

    fn auth_label(&self) -> &'static str {
        if std::env::var("ANTHROPIC_API_KEY").is_ok_and(|k| !k.trim().is_empty()) {
            "ANTHROPIC_API_KEY"
        } else if !self.key_input.trim().is_empty() {
            "key entered below"
        } else if self.saved_key.is_some() {
            "saved key"
        } else {
            "no key set yet"
        }
    }
}

#[cfg(windows)]
pub(crate) fn hidden(cmd: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000) // CREATE_NO_WINDOW: no console flash from a GUI app
}
#[cfg(not(windows))]
pub(crate) fn hidden(cmd: &mut std::process::Command) -> &mut std::process::Command {
    cmd
}

fn ant_token() -> Option<String> {
    let out = hidden(std::process::Command::new("ant").args(["auth", "print-credentials", "--access-token"])).output().ok()?;
    let token = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !token.is_empty()).then_some(token)
}

/// POST the body with curl.exe; headers go through a temp file so the key isn't on a command line.
fn post(body: &Value, auth: &Auth, betas: &[&str]) -> Result<Value, String> {
    let tmp = std::env::temp_dir();
    let stamp = format!("umalord-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    let body_path = tmp.join(format!("{stamp}.json"));
    let head_path = tmp.join(format!("{stamp}.hdr"));
    let mut headers = vec!["content-type: application/json".to_string(), "anthropic-version: 2023-06-01".to_string()];
    let mut betas: Vec<&str> = betas.to_vec();
    match auth {
        Auth::ApiKey(k) => headers.push(format!("x-api-key: {k}")),
        Auth::Bearer(t) => {
            headers.push(format!("authorization: Bearer {t}"));
            betas.push("oauth-2025-04-20");
        }
    }
    if !betas.is_empty() {
        headers.push(format!("anthropic-beta: {}", betas.join(",")));
    }
    let result = (|| {
        std::fs::write(&body_path, body.to_string()).map_err(|e| e.to_string())?;
        std::fs::write(&head_path, headers.join("\n")).map_err(|e| e.to_string())?;
        let out = hidden(std::process::Command::new("curl.exe").args([
            "-sS",
            "--max-time",
            "900",
            "-X",
            "POST",
            API_URL,
            "-H",
            &format!("@{}", head_path.display()),
            "--data-binary",
            &format!("@{}", body_path.display()),
            "-w",
            "\n%{http_code}",
        ]))
        .output()
        .map_err(|e| format!("couldn't run curl.exe: {e}"))?;
        if !out.status.success() {
            return Err(format!("curl failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let (json_text, code) = text.rsplit_once('\n').unwrap_or((&text, "0"));
        let v: Value = serde_json::from_str(json_text).map_err(|e| format!("bad response ({code}): {e}"))?;
        if code.trim() != "200" {
            let msg = v["error"]["message"].as_str().unwrap_or("unknown error");
            return Err(format!("API error {}: {msg}", code.trim()));
        }
        Ok(v)
    })();
    let _ = std::fs::remove_file(&body_path);
    let _ = std::fs::remove_file(&head_path);
    result
}

// ---------------------------------------------------------------------------
// Tools

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "input_schema": {"type": "object", "properties": properties, "required": required},
    })
}

fn tools() -> Value {
    let target = json!({
        "distance": {"type": "string", "enum": ["Sprint", "Mile", "Medium", "Long"]},
        "surface": {"type": "string", "enum": ["Turf", "Dirt"]},
        "style": {"type": "string", "enum": ["Front", "Pace", "Late", "End"], "description": "Running style: Front Runner, Pace Chaser, Late Surger, End Closer"},
    });
    let mut find_umas = target.clone();
    find_umas["min_grade"] = json!({"type": "string", "enum": ["S", "A", "B", "C"], "description": "Minimum aptitude grade for the given filters (default A)"});
    let mut search = target;
    search["query"] = json!({"type": "string", "description": "Text to match in name or effect summary; may be empty"});
    search["owned_only"] = json!({"type": "boolean", "description": "Only skills this account can get now (default true)"});
    search["category"] = json!({"type": "string", "enum": ["Speed Boost", "Acceleration", "Recovery", "Passive", "Debuff", "Lane Effect", "Vision", "Unique", "Evolved"]});
    json!([
        tool("find_umas", "List the account's owned umas whose aptitudes fit a distance, surface and/or style, best fit first, with stars, potential and aptitude grades.", find_umas, &[]),
        tool("get_uma", "Full knowledge for one trainee outfit: base stats, growth rates, aptitudes, unique/innate/potential skills (with which are locked at the owned potential), evolved skills and their conditions, event skills, career objectives, and whether the account owns it. Query by name, outfit or id.", json!({"query": {"type": "string"}}), &["query"]),
        tool("get_card", "Full knowledge for one support card: support effects at the owned level and at max level, unique effect, skill hints, event skills, and every event with its outcomes. Query by uma name, card title or id.", json!({"query": {"type": "string"}}), &["query"]),
        tool("search_skills", "Search skills. Returns name, rarity, SP cost, effect, what it is restricted to, uma.guide's trigger summary, and where this account can get it.", search, &[]),
        tool("get_skill", "Full detail for one skill: effects, trigger conditions, cooldown, and every source (owned or not).", json!({"query": {"type": "string"}}), &["query"]),
        tool("find_races", "Find races/courses by race name or racecourse: track, surface, distance, direction, grade.", json!({"query": {"type": "string"}}), &["query"]),
        tool("list_notes", "List the project's markdown notes: insights.md (Claude's evidence-backed opinions), sources.md, plans/*.md (career plans), results/*.md (real results logs), account/*.md.", json!({}), &[]),
        tool("read_note", "Read one note file by its path from list_notes, e.g. insights.md or results/careers.md.", json!({"path": {"type": "string"}}), &["path"]),
        tool("glossary", "Look up a community term (e.g. 'MLB', 'spark', 'CM').", json!({"term": {"type": "string"}}), &["term"]),
    ])
}

fn parse_distance(v: &Value) -> Option<Distance> {
    Distance::ALL.into_iter().find(|d| v.as_str() == Some(d.label()))
}
fn parse_surface(v: &Value) -> Option<Surface> {
    Surface::ALL.into_iter().find(|d| v.as_str() == Some(d.label()))
}
fn parse_style(v: &Value) -> Option<Style> {
    Style::ALL.into_iter().find(|d| v.as_str() == Some(d.short()) || v.as_str() == Some(d.label()))
}

fn clip(s: String, max: usize) -> String {
    if s.len() <= max {
        s
    } else {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}\n…(truncated)", &s[..end])
    }
}

impl App {
    fn system_prompt(&self) -> String {
        let mut roster = String::new();
        for u in &self.roster.umas {
            roster.push_str(&format!("- uma {} [{}] {}★ P{}\n", u.name, u.outfit, u.stars, u.potential));
        }
        for c in &self.roster.cards {
            roster.push_str(&format!("- card {} {} {} [{}] LB{} Lv{}\n", c.rarity.label(), c.card_type.label(), c.uma, c.title, c.lb, c.level));
        }
        format!(
            "You are the umaLord assistant inside the user's desktop app for Uma Musume: Pretty Derby (Global/English server).\n\
             You answer questions about this specific account: which umas and support cards it owns, what to train and why, \
             decks, skills, parents and sparks, and goals such as Team Trials, Champions Meeting, parent runs and League of Heroes.\n\n\
             How to work:\n\
             - Use the tools for every game fact (aptitudes, growth, skill effects, card effects, events). Don't state game numbers from memory; if a tool can't confirm something, say it's unverified.\n\
             - Recommend owned umas and cards first. Read support effects at the owned level, not max level. A support card can't go in the deck of its own character, and a deck holds one card per character plus one borrowed friend card.\n\
             - Label claims as data (from the tools), simulation (from a plan's Simulation section), or opinion. Check insights.md and results/ for the account's real results before giving an opinion, and say how confident you are.\n\
             - You can't run race simulations yourself. When a question needs one, say so and suggest the app's Career tab: build the plan on its Plan page and press ▶ Simulate, which races it on the Race sim page.\n\
             - Be concise: lead with the answer, then the reasons. Use short markdown lists or tables.\n\n\
             The account's roster ({} umas, {} support cards):\n{roster}",
            self.roster.umas.len(),
            self.roster.cards.len()
        )
    }

    fn find_uma_id(&self, q: &str) -> Result<u32, String> {
        let ql = q.trim().to_lowercase();
        if let Ok(id) = ql.parse::<u32>()
            && self.kb.umas.contains_key(&id)
        {
            return Ok(id);
        }
        let mut hits: Vec<u32> = self
            .kb
            .umas
            .values()
            .filter(|u| format!("{} [{}]", u.name, u.outfit).to_lowercase().contains(&ql) || u.name.to_lowercase() == ql)
            .map(|u| u.id)
            .collect();
        if hits.is_empty() {
            hits = self.kb.umas.values().filter(|u| ql.contains(&u.name.to_lowercase())).map(|u| u.id).collect();
        }
        hits.sort_by_key(|id| (self.owned_uma(*id).is_none(), *id));
        match hits.as_slice() {
            [] => Err(format!("No uma matches '{q}'.")),
            [one] => Ok(*one),
            many if self.owned_uma(many[0]).is_some() && many.iter().filter(|i| self.owned_uma(**i).is_some()).count() == 1 => Ok(many[0]),
            many => Err(format!(
                "Several match '{q}': {}. Ask again with the outfit.",
                many.iter().map(|i| { let u = &self.kb.umas[i]; format!("{} [{}]{}", u.name, u.outfit, if self.owned_uma(*i).is_some() { " (owned)" } else { "" }) }).collect::<Vec<_>>().join("; ")
            )),
        }
    }

    fn find_card_id(&self, q: &str) -> Result<u32, String> {
        let ql = q.trim().to_lowercase();
        if let Ok(id) = ql.parse::<u32>()
            && self.kb.cards.contains_key(&id)
        {
            return Ok(id);
        }
        let mut hits: Vec<u32> = self
            .kb
            .cards
            .values()
            .filter(|c| format!("{} [{}] {} {}", c.uma, c.title, c.rarity, c.kind).to_lowercase().contains(&ql))
            .map(|c| c.id)
            .collect();
        hits.sort_by_key(|id| (self.owned_card(*id).is_none(), std::cmp::Reverse(*id)));
        let owned: Vec<u32> = hits.iter().copied().filter(|i| self.owned_card(*i).is_some()).collect();
        match (hits.as_slice(), owned.as_slice()) {
            ([], _) => Err(format!("No card matches '{q}'.")),
            ([one], _) | (_, [one]) => Ok(*one),
            (many, _) => Err(format!(
                "Several match '{q}': {}. Ask again with the title.",
                many.iter().take(15).map(|i| { let c = &self.kb.cards[i]; format!("{} {} {} [{}]{}", c.rarity, c.kind, c.uma, c.title, self.owned_card(*i).map_or(String::new(), |o| format!(" (owned LB{})", o.lb))) }).collect::<Vec<_>>().join("; ")
            )),
        }
    }

    fn find_skill_id(&self, q: &str) -> Result<u32, String> {
        let ql = q.trim().to_lowercase();
        if let Ok(id) = ql.parse::<u32>()
            && self.kb.skills.contains_key(&id)
        {
            return Ok(id);
        }
        if let Some(id) = self.kb.skill_order.iter().find(|i| self.kb.skills[i].name.to_lowercase() == ql) {
            return Ok(*id);
        }
        let hits: Vec<u32> = self.kb.skill_order.iter().copied().filter(|i| self.kb.skills[i].name.to_lowercase().contains(&ql)).collect();
        match hits.as_slice() {
            [] => Err(format!("No skill matches '{q}'.")),
            [one] => Ok(*one),
            many => Err(format!("Several match '{q}': {}.", many.iter().take(20).map(|i| self.kb.skills[i].name.clone()).collect::<Vec<_>>().join("; "))),
        }
    }

    fn skill_line(&self, id: u32) -> String {
        let Some(s) = self.kb.skills.get(&id) else { return format!("skill {id}") };
        let tags = self.kb.tag_line(&s.tags());
        format!(
            "{} ({}, {} SP{}): {}{}",
            s.name,
            s.tier(),
            s.sp,
            if tags.is_empty() { String::new() } else { format!(", only {tags}") },
            if s.summary.is_empty() { s.effect_text() } else { s.summary.clone() },
            if self.skill_available(id) { " [available to you]" } else { "" }
        )
    }

    pub(crate) fn run_tool(&self, name: &str, input: &Value) -> Result<String, String> {
        match name {
            "find_umas" => {
                let (d, s, st) = (parse_distance(&input["distance"]), parse_surface(&input["surface"]), parse_style(&input["style"]));
                let min = input["min_grade"].as_str().map_or(7, grade_rank);
                let mut rows: Vec<(u32, String)> = Vec::new();
                for u in &self.roster.umas {
                    let Some(info) = self.kb.umas.get(&u.id) else { continue };
                    let a = &info.aptitude;
                    let grades = [d.map(|x| a.distance(x)), s.map(|x| a.surface(x)), st.map(|x| a.style(x))];
                    if grades.iter().flatten().any(|g| grade_rank(g) < min) {
                        continue;
                    }
                    let score: u32 = grades.iter().flatten().map(|g| u32::from(grade_rank(g))).sum::<u32>() * 10 + u32::from(u.stars);
                    rows.push((score, format!(
                        "{} [{}] {}★ P{} | turf {} dirt {} | sprint {} mile {} medium {} long {} | front {} pace {} late {} end {}",
                        u.name, u.outfit, u.stars, u.potential, a.turf, a.dirt, a.sprint, a.mile, a.medium, a.long, a.front, a.pace, a.late, a.end
                    )));
                }
                rows.sort_by(|a, b| b.0.cmp(&a.0));
                Ok(if rows.is_empty() { "No owned uma fits.".into() } else { rows.into_iter().map(|r| r.1).collect::<Vec<_>>().join("\n") })
            }
            "get_uma" => {
                let id = self.find_uma_id(input["query"].as_str().unwrap_or_default())?;
                let i = &self.kb.umas[&id];
                let owned = self.owned_uma(id);
                let a = &i.aptitude;
                let mut out = format!(
                    "{} [{}] (id {id}), released {}\nOwned: {}\nAptitude: turf {} dirt {} | sprint {} mile {} medium {} long {} | front {} pace {} late {} end {}\nBase stats (spd/sta/pow/guts/wit): {:?}\nGrowth %: {:?}\n",
                    i.name, i.outfit, i.release,
                    owned.map_or("no".to_string(), |o| format!("yes, {}★, potential {}", o.stars, o.potential)),
                    a.turf, a.dirt, a.sprint, a.mile, a.medium, a.long, a.front, a.pace, a.late, a.end, i.base, i.growth
                );
                if let Some(u) = i.unique_skill {
                    out += &format!("Unique: {}\n", self.skill_line(u));
                }
                for s in &i.skills {
                    let locked = owned.is_some_and(|o| o.potential < s.potential);
                    out += &format!("{}{}: {}\n", if s.potential <= 1 { "Innate".to_string() } else { format!("Potential {}", s.potential) }, if locked { " (LOCKED for this account)" } else { "" }, self.skill_line(s.id));
                }
                for e in &i.evolved {
                    out += &format!("Evolves {} -> {} when: {}\n", self.kb.skill_name(e.base), self.skill_line(e.evolved), e.conditions.iter().map(|c| c.join(" + ")).collect::<Vec<_>>().join(" OR "));
                }
                if !i.event_skills.is_empty() {
                    out += &format!("Event skills: {}\n", i.event_skills.iter().map(|s| self.kb.skill_name(*s)).collect::<Vec<_>>().join(", "));
                }
                out += "Career objectives:\n";
                for o in &i.objectives {
                    out += &format!("- {}: {} ({})\n", o.when, o.race, o.goal);
                }
                out += &format!("{} career events (ask get_uma only for summary; event outcomes are in the Umas tab).\n", i.events.len());
                Ok(out)
            }
            "get_card" => {
                let id = self.find_card_id(input["query"].as_str().unwrap_or_default())?;
                let c = &self.kb.cards[&id];
                let owned = self.owned_card(id);
                let max = match c.rarity.as_str() { "SSR" => 50, "SR" => 45, _ => 40 };
                let level = owned.map_or(max, |o| u32::from(o.level));
                let mut out = format!(
                    "{} {} {} [{}] (id {id}), released {}\nOwned: {}\nEffects (at owned Lv{level} / at Lv{max}):\n",
                    c.rarity, c.kind, c.uma, c.title, c.release,
                    owned.map_or("no".to_string(), |o| format!("yes, LB{} Lv{}", o.lb, o.level))
                );
                for e in &c.effects {
                    let now = crate::knowledge::effect_at(&e.points, level);
                    let top = crate::knowledge::effect_at(&e.points, max);
                    if now.unwrap_or(0) != 0 || top.unwrap_or(0) != 0 {
                        out += &format!("- {}: {} / {}\n", e.name, now.unwrap_or(0), top.unwrap_or(0));
                    }
                }
                if let Some(u) = &c.unique {
                    out += &format!("Unique effect (from Lv{}{}): {} {}\n", u.level, if level >= u.level { ", active" } else { ", NOT active at owned level" }, u.condition, u.effects.iter().map(|e| format!("{} {}", e.name, e.value)).collect::<Vec<_>>().join(", "));
                }
                out += "Skill hints:\n";
                for s in &c.hints {
                    out += &format!("- {}\n", self.skill_line(*s));
                }
                if !c.event_skills.is_empty() {
                    out += &format!("Skills from events: {}\n", c.event_skills.iter().map(|s| self.kb.skill_name(*s)).collect::<Vec<_>>().join(", "));
                }
                out += "Events (options; '|' separates random alternative outcomes, odds unknown):\n";
                for e in &c.events {
                    let opts: Vec<String> = e.options.iter().map(|alts| alts.iter().map(|l| l.join(", ")).collect::<Vec<_>>().join(" | ")).collect();
                    out += &format!("- {} ({}): {}\n", e.title, e.kind, opts.join(" // "));
                }
                Ok(clip(out, 12_000))
            }
            "search_skills" => {
                let q = input["query"].as_str().unwrap_or_default().to_lowercase();
                let owned_only = input["owned_only"].as_bool().unwrap_or(true);
                let cat = input["category"].as_str();
                let (d, s, st) = (parse_distance(&input["distance"]), parse_surface(&input["surface"]), parse_style(&input["style"]));
                let hits: Vec<String> = self
                    .kb
                    .skill_order
                    .iter()
                    .filter(|id| {
                        let k = &self.kb.skills[id];
                        (q.is_empty() || k.name.to_lowercase().contains(&q) || k.summary.to_lowercase().contains(&q))
                            && cat.is_none_or(|c| k.category == c || k.tier() == c)
                            && k.tags().fits(st, d, s)
                            && (!owned_only || self.skill_available(**id))
                    })
                    .take(40)
                    .map(|id| self.skill_line(*id))
                    .collect();
                Ok(if hits.is_empty() { "No skills match.".into() } else { hits.join("\n") })
            }
            "get_skill" => {
                let id = self.find_skill_id(input["query"].as_str().unwrap_or_default())?;
                let k = &self.kb.skills[&id];
                let mut out = format!(
                    "{} ({}, {}, {} SP, wit check: {})\nEffect: {}\nTrigger (uma.guide): {}\nDescription: {}\nRaw conditions: {} | pre: {}\nBase duration {}s, cooldown {}s\nSources:\n",
                    k.name, k.tier(), k.category, k.sp, k.wit_check, k.effect_text(), k.summary, k.desc, k.conditions, k.pre, k.duration, k.cooldown
                );
                for src in self.kb.sources.get(&id).into_iter().flatten() {
                    let who = src
                        .uma()
                        .and_then(|u| self.kb.umas.get(&u).map(|i| format!("uma {} [{}]", i.name, i.outfit)))
                        .or_else(|| src.card().and_then(|c| self.kb.cards.get(&c).map(|i| format!("card {} {} [{}]", i.rarity, i.uma, i.title))))
                        .unwrap_or_default();
                    out += &format!("- {:?} {who}{}\n", src, if self.source_available(*src) { " (available to you)" } else { "" });
                }
                Ok(out)
            }
            "find_races" => {
                let q = input["query"].as_str().unwrap_or_default().to_lowercase();
                let mut seen = std::collections::HashSet::new();
                let hits: Vec<String> = self
                    .kb
                    .races
                    .iter()
                    .filter(|r| r.name.to_lowercase().contains(&q) || r.track.to_lowercase().contains(&q))
                    .filter(|r| seen.insert((r.name.clone(), r.distance)))
                    .take(25)
                    .map(|r| format!("{} ({}): {} {} {}m {} [{}]", r.name, r.grade, r.track, r.surface, r.distance, r.direction, r.category))
                    .collect();
                Ok(if hits.is_empty() { "No race matches.".into() } else { hits.join("\n") })
            }
            "list_notes" => {
                let root = self.root.as_ref().ok_or("umaLord folder not found")?;
                let mut files = vec!["insights.md".to_string(), "sources.md".to_string()];
                for dir in ["plans", "results", "account"] {
                    for e in std::fs::read_dir(root.join(dir)).into_iter().flatten().flatten() {
                        let n = e.file_name().to_string_lossy().to_string();
                        if n.ends_with(".md") && !n.starts_with('_') {
                            files.push(format!("{dir}/{n}"));
                        }
                    }
                }
                Ok(files.join("\n"))
            }
            "read_note" => {
                let root = self.root.as_ref().ok_or("umaLord folder not found")?;
                let rel = input["path"].as_str().unwrap_or_default().replace('\\', "/");
                let allowed = rel.ends_with(".md")
                    && !rel.contains("..")
                    && (["insights.md", "sources.md", "CLAUDE.md"].contains(&rel.as_str()) || ["plans/", "results/", "account/", "knowledge/"].iter().any(|d| rel.starts_with(d)));
                if !allowed {
                    return Err(format!("Not a readable note: {rel}"));
                }
                let text = std::fs::read_to_string(root.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
                Ok(clip(text.split("<!-- umalord-plan").next().unwrap_or("").to_string(), 20_000))
            }
            "glossary" => {
                let q = input["term"].as_str().unwrap_or_default().to_lowercase();
                let hits: Vec<String> = self
                    .kb
                    .glossary
                    .iter()
                    .filter(|g| g.term.to_lowercase().contains(&q) || g.aliases.iter().any(|a| a.to_lowercase().contains(&q)))
                    .take(8)
                    .map(|g| format!("{}: {}", g.term, g.description))
                    .collect();
                Ok(if hits.is_empty() { "Not in the glossary.".into() } else { hits.join("\n") })
            }
            other => Err(format!("Unknown tool {other}")),
        }
    }

    fn send_request(&mut self, ctx: &egui::Context) {
        let Some(auth) = self.ask.auth() else {
            self.ask.status = Some(("No API key. Set ANTHROPIC_API_KEY, enter a key below, or log in with the ant CLI.".into(), true));
            return;
        };
        let (model, _, fallbacks) = MODELS[self.ask.model];
        let mut body = json!({
            "model": model,
            "max_tokens": 16000,
            "system": self.system_prompt(),
            "tools": tools(),
            "messages": self.ask.messages,
            "output_config": {"effort": EFFORTS[self.ask.effort]},
            "cache_control": {"type": "ephemeral"},
        });
        let mut betas = Vec::new();
        if fallbacks {
            // Server-side refusal fallback: a declined request is re-run on Anthropic's recommended model.
            body["fallbacks"] = json!("default");
            betas.push("server-side-fallback-2026-07-01");
        }
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(post(&body, &auth, &betas));
            ctx.request_repaint();
        });
        self.ask.pending = Some(rx);
        self.ask.status = None;
    }

    pub(crate) fn ask_busy(&self) -> bool {
        self.ask.pending.is_some() || self.ask.oc.rx.is_some()
    }

    fn send_opencode(&mut self, ctx: &egui::Context, q: String) {
        let (Some(exe), Some(root)) = (self.ask.oc.exe.clone(), self.root.clone()) else {
            self.ask.status = Some(("OpenCode isn't installed: tools/opencode/opencode.exe is missing.".into(), true));
            return;
        };
        self.write_digest();
        let ctx = ctx.clone();
        self.ask.oc.answer_open = false;
        self.ask.oc.rx = Some(crate::ask_opencode::ask(
            exe,
            root,
            self.ask.oc.model.clone(),
            self.ask.oc.session.clone(),
            q,
            self.ask.oc.child.clone(),
            move || ctx.request_repaint(),
        ));
        self.ask.status = None;
    }

    pub(crate) fn start_opencode_install(&mut self, ctx: &egui::Context) {
        let Some(root) = self.root.clone() else { return };
        let ctx = ctx.clone();
        self.ask.oc.install_error = None;
        self.ask.oc.install_progress = 0;
        self.ask.oc.install_rx = Some(crate::ask_opencode::install(crate::ask_opencode::install_dir(&root), move || ctx.request_repaint()));
    }

    /// Download card shown while OpenCode isn't installed.
    fn opencode_install_ui(&mut self, ui: &mut egui::Ui) {
        use crate::ask_opencode::{RELEASE, ZIP_SIZE};
        let p = pal(ui);
        let mb = |b: u64| b as f32 / 1_048_576.0;
        ui.label(semibold("OpenCode isn't installed yet", 16.0));
        ui.label(RichText::new("It answers your questions for free, with no key and no account.").color(p.weak));
        ui.add_space(6.0);
        if self.ask.oc.install_rx.is_some() {
            let done = self.ask.oc.install_progress;
            let bar = egui::ProgressBar::new((done as f32 / ZIP_SIZE as f32).min(1.0))
                .text(format!("Downloading… {:.1} / {:.1} MB", mb(done), mb(ZIP_SIZE)))
                .desired_width(ui.available_width().min(420.0));
            ui.add(bar);
        } else {
            let button = crate::ui_util::primary_button(ui, format!("⬇ Download OpenCode ({:.0} MB)", mb(ZIP_SIZE)), 14.0)
                .min_size(vec2(240.0, 36.0));
            if ui.add_enabled(self.root.is_some(), button).clicked() {
                let ctx = ui.ctx().clone();
                self.start_opencode_install(&ctx);
            }
        }
        if let Some(err) = &self.ask.oc.install_error {
            ui.label(RichText::new(err).color(WARN));
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "Official release {RELEASE} from github.com/anomalyco/opencode. The app checks its SHA-256 against the published one before installing it to tools\\opencode."
            ))
            .size(11.5)
            .color(p.weak),
        );
    }

    fn opencode_poll(&mut self, ctx: &egui::Context) {
        use crate::ask_opencode::{InstallEvent, OcEvent};
        if let Some(rx) = &self.ask.oc.install_rx {
            for e in rx.try_iter().collect::<Vec<_>>() {
                match e {
                    InstallEvent::Progress(n) => self.ask.oc.install_progress = n,
                    InstallEvent::Done(Ok(exe)) => {
                        self.ask.oc.install_rx = None;
                        self.ask.oc.models_rx = Some(crate::ask_opencode::list_models(exe.clone()));
                        self.ask.oc.exe = Some(exe);
                        self.ask.status = Some(("OpenCode is installed. Ask away.".into(), false));
                    }
                    InstallEvent::Done(Err(e)) => {
                        self.ask.oc.install_rx = None;
                        self.ask.oc.install_error = Some(e);
                    }
                }
            }
        }
        if let Some(rx) = &self.ask.oc.models_rx
            && let Ok(result) = rx.try_recv()
        {
            self.ask.oc.models_rx = None;
            match result {
                Ok(models) => {
                    if !models.contains(&self.ask.oc.model) {
                        let fallback = models.iter().find(|m| m.as_str() == crate::ask_opencode::DEFAULT_MODEL).or(models.first());
                        self.ask.oc.model = fallback.cloned().unwrap_or_default();
                    }
                    self.ask.oc.models = models;
                }
                Err(e) => self.ask.status = Some((format!("Couldn't list OpenCode models: {e}"), true)),
            }
        }
        let Some(rx) = &self.ask.oc.rx else { return };
        let events: Vec<OcEvent> = rx.try_iter().collect();
        for e in events {
            match e {
                OcEvent::Session(s) => self.ask.oc.session = Some(s),
                OcEvent::Tool(t) => self.ask.activity.push(t),
                OcEvent::Tokens(i, o) => {
                    self.ask.tokens_in += i;
                    self.ask.tokens_out += o;
                }
                OcEvent::Text(t) => match self.ask.turns.last_mut() {
                    Some(last) if self.ask.oc.answer_open && last.role == "assistant" => {
                        last.text.push_str("\n\n");
                        last.text.push_str(&t);
                    }
                    _ => {
                        self.ask.turns.push(Turn { role: "assistant", text: t });
                        self.ask.oc.answer_open = true;
                    }
                },
                OcEvent::Error(msg) => self.ask.status = Some((format!("OpenCode: {msg}"), true)),
                OcEvent::Done(result) => {
                    self.ask.oc.rx = None;
                    if let Err(msg) = result {
                        self.ask.status = Some((msg, true));
                    } else if !self.ask.oc.answer_open && self.ask.status.is_none() {
                        self.ask.status = Some(("OpenCode finished without an answer. Try again or pick another model.".into(), true));
                    }
                    ctx.request_repaint();
                    return;
                }
            }
        }
    }

    pub(crate) fn ask_poll(&mut self, ctx: &egui::Context) {
        self.opencode_poll(ctx);
        let Some(rx) = &self.ask.pending else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.ask.pending = None;
        let resp = match result {
            Ok(v) => v,
            Err(e) => {
                self.ask.status = Some((e, true));
                return;
            }
        };
        self.ask.tokens_in += resp["usage"]["input_tokens"].as_u64().unwrap_or(0)
            + resp["usage"]["cache_read_input_tokens"].as_u64().unwrap_or(0)
            + resp["usage"]["cache_creation_input_tokens"].as_u64().unwrap_or(0);
        self.ask.tokens_out += resp["usage"]["output_tokens"].as_u64().unwrap_or(0);

        let content = resp["content"].as_array().cloned().unwrap_or_default();
        // Echo the assistant turn back unchanged, except blocks before a fallback boundary that
        // the API says to omit, and the fallback marker itself.
        let boundary = content.iter().rposition(|b| b["type"] == "fallback");
        let echoed: Vec<Value> = content
            .iter()
            .enumerate()
            .filter(|(i, b)| {
                let t = b["type"].as_str().unwrap_or_default();
                t != "fallback" && !(boundary.is_some_and(|f| *i < f) && matches!(t, "thinking" | "redacted_thinking" | "tool_use"))
            })
            .map(|(_, b)| b.clone())
            .collect();
        let text: String = echoed.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n\n");
        if !text.trim().is_empty() {
            self.ask.turns.push(Turn { role: "assistant", text });
        }
        if !echoed.is_empty() {
            self.ask.messages.push(json!({"role": "assistant", "content": echoed}));
        }

        match resp["stop_reason"].as_str().unwrap_or_default() {
            "tool_use" => {
                let calls: Vec<Value> = echoed.iter().filter(|b| b["type"] == "tool_use").cloned().collect();
                let mut results = Vec::new();
                for call in calls {
                    let name = call["name"].as_str().unwrap_or_default();
                    self.ask.activity.push(format!("{name} {}", call["input"]));
                    let (text, is_error) = match self.run_tool(name, &call["input"]) {
                        Ok(t) => (t, false),
                        Err(e) => (e, true),
                    };
                    results.push(json!({"type": "tool_result", "tool_use_id": call["id"], "content": text, "is_error": is_error}));
                }
                self.ask.messages.push(json!({"role": "user", "content": results}));
                self.ask.rounds += 1;
                if self.ask.rounds > MAX_TOOL_ROUNDS {
                    self.ask.status = Some(("Stopped after many tool calls. Ask a narrower question.".into(), true));
                    // Keep the transcript valid: the next user message follows the tool results.
                    return;
                }
                self.send_request(ctx);
            }
            "refusal" => {
                let why = resp["stop_details"]["explanation"].as_str().unwrap_or("The model declined this request.");
                self.ask.status = Some((format!("Declined: {why}"), true));
            }
            "max_tokens" => {
                self.ask.status = Some(("The answer hit the length limit and is cut off.".into(), true));
            }
            _ => {
                let served = resp["model"].as_str().unwrap_or_default();
                if served != MODELS[self.ask.model].0 && !served.is_empty() {
                    self.ask.status = Some((format!("Answered by {served} (fallback)."), false));
                }
            }
        }
    }

    pub(crate) fn submit_question(&mut self, ctx: &egui::Context, q: String) {
        self.ask.turns.push(Turn { role: "user", text: q.clone() });
        if self.ask.backend == Backend::OpenCode {
            self.ask.activity.clear();
            self.send_opencode(ctx, q);
            return;
        }
        // After a refusal or a stopped tool loop the transcript already ends with a
        // user message; add the question to it rather than starting a second one.
        match self.ask.messages.last_mut() {
            Some(last) if last["role"] == "user" => {
                if let Some(blocks) = last["content"].as_array_mut() {
                    blocks.push(json!({"type": "text", "text": q}));
                }
            }
            _ => self.ask.messages.push(json!({"role": "user", "content": [{"type": "text", "text": q}]})),
        }
        self.ask.rounds = 0;
        self.ask.activity.clear();
        self.send_request(ctx);
    }

    pub(crate) fn ask_tab(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        egui::Panel::right("ask_settings").resizable(false).exact_size(300.0).frame(crate::side_frame()).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.ask_settings(ui));
        });
        egui::Panel::bottom("ask_input").resizable(false).frame(egui::Frame::new().inner_margin(Margin::symmetric(18, 10))).show(ui, |ui| {
            let busy = self.ask_busy();
            egui::Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border)).corner_radius(CornerRadius::same(14)).inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let edit = ui.add_sized(
                        [ui.available_width() - 96.0, 56.0],
                        egui::TextEdit::multiline(&mut self.ask.input)
                            .hint_text("Ask about your account…  (Enter to send, Shift+Enter for a new line)")
                            .frame(egui::Frame::new()),
                    );
                    let enter = edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                    if self.ask.oc.rx.is_some() {
                        let stop = egui::Button::new(semibold("■ Stop", 14.0)).min_size(vec2(84.0, 40.0));
                        if ui.add(stop).clicked()
                            && let Some(mut child) = self.ask.oc.child.lock().unwrap_or_else(|p| p.into_inner()).take()
                        {
                            let _ = child.kill();
                        }
                        return;
                    }
                    let send = crate::ui_util::primary_button(ui, "Send ➤", 14.0).min_size(vec2(84.0, 40.0));
                    if (ui.add_enabled(!busy, send).clicked() || (enter && !busy)) && !self.ask.input.trim().is_empty() {
                        let q = self.ask.input.trim().to_string();
                        self.ask.input.clear();
                        let ctx = ui.ctx().clone();
                        self.submit_question(&ctx, q);
                    }
                });
            });
        });
        let mut suggestion = None;
        egui::CentralPanel::default().frame(crate::page_frame()).show(ui, |ui| {
            crate::ui_util::page_header(
                ui,
                "💬 Ask",
                "Ask in plain English, like you'd ask a friend who knows the game. Answers come from your own roster and the game data.",
                |_| {},
            );
            egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                ui.set_max_width(920.0);
                let needs_install = self.ask.backend == Backend::OpenCode && self.ask.oc.exe.is_none();
                if self.ask.turns.is_empty() && needs_install {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.set_max_width(520.0);
                        crate::ui_util::card(ui, |ui| self.opencode_install_ui(ui));
                    });
                } else if self.ask.turns.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(24.0);
                        ui.label(RichText::new("💬").size(44.0).color(p.weak));
                        ui.label(semibold("What do you want to know?", 20.0));
                        let who = if self.ask.backend == Backend::OpenCode { "A free model (via OpenCode)" } else { "Claude" };
                        ui.label(
                            RichText::new(format!("{who} looks things up in your roster, the uma.guide data, your plans and results, and labels what is data and what is opinion."))
                                .color(p.weak),
                        );
                        ui.add_space(16.0);
                        ui.label(RichText::new("Try one of these:").size(12.5).color(p.weak));
                        for s in SUGGESTIONS {
                            let b = egui::Button::new(RichText::new(s).size(13.5)).corner_radius(CornerRadius::same(12)).min_size(vec2(560.0, 34.0));
                            if ui.add(b).clicked() {
                                suggestion = Some(s.to_string());
                            }
                        }
                    });
                }
                for turn in &self.ask.turns {
                    ui.add_space(10.0);
                    if turn.role == "user" {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            egui::Frame::new()
                                .fill(p.accent.gamma_multiply(0.18))
                                .stroke(Stroke::new(1.0, p.accent.gamma_multiply(0.45)))
                                .corner_radius(CornerRadius { nw: 14, ne: 4, sw: 14, se: 14 })
                                .inner_margin(Margin::symmetric(14, 10))
                                .show(ui, |ui| {
                                    ui.set_max_width(620.0);
                                    ui.label(&turn.text);
                                });
                        });
                    } else {
                        ui.horizontal_top(|ui| {
                            ui.label(RichText::new("✳").size(20.0).color(Color32::from_rgb(217, 119, 87)));
                            egui::Frame::new()
                                .fill(p.surface)
                                .stroke(Stroke::new(1.0, p.border))
                                .corner_radius(CornerRadius { nw: 4, ne: 14, sw: 14, se: 14 })
                                .inner_margin(Margin::symmetric(16, 12))
                                .show(ui, |ui| {
                                    ui.set_max_width(780.0);
                                    egui_commonmark::CommonMarkViewer::new().show(ui, &mut self.md_cache, &turn.text);
                                });
                        });
                    }
                }
                if self.ask_busy() {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        let text = self.ask.activity.last().map_or("Thinking…".to_string(), |a| format!("Looking up: {}", truncate(a, 80)));
                        ui.label(RichText::new(text).color(p.weak));
                    });
                }
                if let Some((msg, err)) = &self.ask.status {
                    ui.add_space(8.0);
                    ui.label(RichText::new(msg).color(if *err { WARN } else { GOOD }));
                }
                ui.add_space(8.0);
            });
        });
        if let Some(q) = suggestion {
            let ctx = ui.ctx().clone();
            self.submit_question(&ctx, q);
        }
    }

    fn ask_settings(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let busy = self.ask_busy();
        let s = &mut self.ask;
        filter_heading(ui, "Answered by");
        let before = s.backend;
        ui.add_enabled_ui(!busy, |ui| {
            ui.radio_value(&mut s.backend, Backend::OpenCode, "🆓 OpenCode · free models");
            ui.radio_value(&mut s.backend, Backend::ClaudeApi, "🔑 Claude API · your key");
        });
        if s.backend != before {
            crate::settings::set("ask_backend", json!(if s.backend == Backend::OpenCode { "opencode" } else { "claude_api" }));
        }

        if s.backend == Backend::OpenCode && s.oc.exe.is_none() {
            ui.add_space(8.0);
            if s.turns.is_empty() {
                // The chat area shows the download card.
                ui.label(RichText::new("OpenCode isn't installed yet. Use the download button on the left.").size(12.5).color(p.weak));
            } else {
                self.opencode_install_ui(ui);
            }
        } else if s.backend == Backend::OpenCode {
            Self::opencode_settings(ui, s);
        } else {
            Self::claude_api_settings(ui, s);
        }
        let s = &mut self.ask;

        filter_heading(ui, "This conversation");
        ui.label(RichText::new(format!("{} tokens in · {} out", s.tokens_in, s.tokens_out)).size(12.5).color(p.weak));
        if s.backend == Backend::OpenCode {
            ui.label(RichText::new("Free models cost nothing; the count is for interest.").size(11.5).color(p.weak));
        }
        if ui.add_enabled(!busy, egui::Button::new("🗑 New conversation")).clicked() {
            s.messages.clear();
            s.turns.clear();
            s.activity.clear();
            s.status = None;
            s.tokens_in = 0;
            s.tokens_out = 0;
            s.oc.session = None;
        }
        if !s.activity.is_empty() {
            ui.add_space(10.0);
            egui::CollapsingHeader::new(format!("Lookups ({})", s.activity.len())).show(ui, |ui| {
                for a in &s.activity {
                    ui.label(RichText::new(truncate(a, 120)).small().monospace());
                }
            });
        }
    }

    fn opencode_settings(ui: &mut egui::Ui, s: &mut AskState) {
        let p = pal(ui);
        let Some(exe) = s.oc.exe.clone() else { return };
        if s.oc.models.is_empty() && s.oc.models_rx.is_none() {
            s.oc.models_rx = Some(crate::ask_opencode::list_models(exe.clone()));
        }
        filter_heading(ui, "Model");
        if s.oc.models_rx.is_some() && s.oc.models.is_empty() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("Loading models…").color(p.weak));
            });
        }
        let before = s.oc.model.clone();
        let models = &s.oc.models;
        let items = || {
            models
                .iter()
                .map(|m| {
                    let text = format!("{}{}", if crate::ask_opencode::is_free(m) { "🆓 " } else { "" }, m.trim_start_matches("opencode/"));
                    (Some(m.clone()), text, crate::ask_opencode::model_note(m).to_string())
                })
                .collect()
        };
        let current = s.oc.model.clone();
        if let Some(m) = crate::ui_util::search_combo(ui, "oc_model", current.trim_start_matches("opencode/"), 250.0, items, Some(&current)) {
            s.oc.model = m;
        }
        if s.oc.model != before {
            crate::settings::set("opencode_model", json!(s.oc.model));
        }
        let note = crate::ask_opencode::model_note(&s.oc.model);
        let safe = note.starts_with("keeps nothing");
        badge(ui, format!("{} {note}", if safe { "🔒" } else { "ℹ" }), if safe { p.accent } else { WARN });
        ui.add_space(4.0);
        if ui.small_button("🔄 Refresh models").clicked() && s.oc.models_rx.is_none() {
            s.oc.models.clear();
            s.oc.models_rx = Some(crate::ask_opencode::list_models(exe));
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "OpenCode (opencode.ai) answers from your files: the roster digest, uma.guide data, plans and results. It is read-only: \
                 it can't edit files or run commands. Free models are offered for a limited time and are weaker than Claude.",
            )
            .size(11.5)
            .color(p.weak),
        );
    }

    fn claude_api_settings(ui: &mut egui::Ui, s: &mut AskState) {
        let p = pal(ui);
        filter_heading(ui, "Model");
        for (i, (_, label, _)) in MODELS.iter().enumerate() {
            ui.radio_value(&mut s.model, i, *label);
        }
        filter_heading(ui, "Effort");
        ui.horizontal(|ui| {
            for (i, e) in EFFORTS.iter().enumerate() {
                ui.selectable_value(&mut s.effort, i, *e);
            }
        });
        ui.label(RichText::new("Higher effort thinks longer and costs more.").size(12.0).color(p.weak));

        filter_heading(ui, "API key");
        let has_key = !s.auth_label().starts_with("none");
        badge(ui, format!("{} {}", if has_key { "✔" } else { "✖" }, s.auth_label()), if has_key { p.accent } else { WARN });
        ui.add_space(4.0);
        ui.add(egui::TextEdit::singleline(&mut s.key_input).password(true).hint_text("sk-ant-…").desired_width(f32::INFINITY));
        ui.checkbox(&mut s.remember, "Remember on this PC");
        if s.remember && !s.key_input.trim().is_empty() && ui.button("💾 Save key").clicked() {
            let ok = crate::settings::set("api_key", json!(s.key_input.trim()));
            if ok {
                s.saved_key = Some(s.key_input.trim().to_string());
            }
            s.status = Some((if ok { "Key saved.".into() } else { "Couldn't save the key.".into() }, !ok));
        }
        if s.saved_key.is_some() && ui.small_button("Forget saved key").clicked() {
            crate::settings::set("api_key", Value::Null);
            s.saved_key = None;
        }
        ui.label(RichText::new("Get a key at console.anthropic.com. Saved keys are stored in plain text under %APPDATA%\\umaLord.").size(11.5).color(p.weak));
    }
}
