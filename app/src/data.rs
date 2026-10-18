//! Roster data, parsed straight from the markdown tables in `account/`.
//!
//! The markdown files stay the single source of truth: Claude keeps them up to
//! date, and this app only reads them. Columns are found by header name, so
//! reordering or adding columns in the markdown doesn't break parsing.

use std::path::{Path, PathBuf};

pub const CARDS_FILE: &str = "account/support_cards.md";
pub const UMAS_FILE: &str = "account/umas.md";

/// An empty roster, used when the umaLord folder can't be found at runtime. (Not the account
/// itself: `account/` is the user's data and stays out of the source.)
const EMBEDDED_CARDS: &str = include_str!("../templates/account/support_cards.md");
const EMBEDDED_UMAS: &str = include_str!("../templates/account/umas.md");

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum CardType {
    Speed,
    Stamina,
    Power,
    Guts,
    Wit,
    Pal,
    Group,
}

impl CardType {
    pub const ALL: [CardType; 7] = [
        CardType::Speed,
        CardType::Stamina,
        CardType::Power,
        CardType::Guts,
        CardType::Wit,
        CardType::Pal,
        CardType::Group,
    ];

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "speed" => Some(Self::Speed),
            "stamina" => Some(Self::Stamina),
            "power" => Some(Self::Power),
            "guts" => Some(Self::Guts),
            "wit" | "wisdom" | "int" => Some(Self::Wit),
            "pal" | "friend" => Some(Self::Pal),
            "group" => Some(Self::Group),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Speed => "Speed",
            Self::Stamina => "Stamina",
            Self::Power => "Power",
            Self::Guts => "Guts",
            Self::Wit => "Wit",
            Self::Pal => "Pal",
            Self::Group => "Group",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Rarity {
    R,
    SR,
    SSR,
}

impl Rarity {
    /// Game card IDs encode rarity in the leading digit: 3xxxx SSR, 2xxxx SR, 1xxxx R.
    pub fn from_id(id: u32) -> Option<Self> {
        match id / 10_000 {
            3 => Some(Self::SSR),
            2 => Some(Self::SR),
            1 => Some(Self::R),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::R => "R",
            Self::SR => "SR",
            Self::SSR => "SSR",
        }
    }

    /// Level cap at a given limit break (LB4 = MLB).
    pub fn max_level(self, lb: u8) -> u8 {
        let base = match self {
            Self::SSR => 30,
            Self::SR => 25,
            Self::R => 20,
        };
        base + 5 * lb.min(4)
    }
}

#[derive(Clone, Debug)]
pub struct SupportCard {
    pub id: u32,
    pub title: String,
    pub uma: String,
    pub card_type: CardType,
    pub rarity: Rarity,
    pub lb: u8,
    pub level: u8,
    pub notes: String,
    pub url: String,
}

impl SupportCard {
    pub fn is_mlb(&self) -> bool {
        self.lb >= 4
    }

    pub fn display_name(&self) -> String {
        format!("{} [{}]", self.uma, self.title)
    }
}

#[derive(Clone, Debug)]
pub struct Uma {
    pub id: u32,
    pub name: String,
    pub outfit: String,
    pub stars: u8,
    pub potential: u8,
    pub notes: String,
    pub url: String,
}

impl Uma {
    pub fn display_name(&self) -> String {
        format!("{} [{}]", self.name, self.outfit)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Roster {
    pub cards: Vec<SupportCard>,
    pub umas: Vec<Uma>,
    pub server: Option<String>,
    /// The profile's name (`- Profile:` in umas.md): whose account this is.
    pub profile: Option<String>,
    pub cards_updated: Option<String>,
    /// Rows that couldn't be parsed or fail a sanity check, e.g. a level above the LB cap.
    pub warnings: Vec<String>,
}

/// Where the roster was loaded from.
#[derive(Clone, Debug)]
pub enum Source {
    Folder(PathBuf),
    Embedded,
}

impl Roster {
    /// Load from the umaLord folder when given, else from the build-time snapshot.
    pub fn load(root: Option<&Path>) -> (Roster, Source) {
        if let Some(root) = root {
            let cards = std::fs::read_to_string(root.join(CARDS_FILE));
            let umas = std::fs::read_to_string(root.join(UMAS_FILE));
            if let (Ok(cards), Ok(umas)) = (cards, umas) {
                return (Self::parse(&cards, &umas), Source::Folder(root.to_path_buf()));
            }
        }
        (Self::parse(EMBEDDED_CARDS, EMBEDDED_UMAS), Source::Embedded)
    }

    pub fn parse(cards_md: &str, umas_md: &str) -> Roster {
        let mut roster = Roster {
            server: meta_value(umas_md, "Server"),
            profile: meta_line(umas_md, "Profile"),
            cards_updated: meta_value(cards_md, "Last updated"),
            ..Default::default()
        };
        parse_cards(cards_md, &mut roster);
        parse_umas(umas_md, &mut roster);
        roster
    }

    /// Nothing recorded yet: a new install, or after "Start over".
    pub fn is_empty(&self) -> bool {
        self.umas.is_empty() && self.cards.is_empty()
    }

    /// The test roster: a real account's files, kept in `app/tests/fixtures/`.
    #[cfg(test)]
    pub fn sample() -> Roster {
        Self::parse(SAMPLE_CARDS, SAMPLE_UMAS)
    }
}

#[cfg(test)]
pub const SAMPLE_CARDS: &str = include_str!("../tests/fixtures/support_cards.md");
#[cfg(test)]
pub const SAMPLE_UMAS: &str = include_str!("../tests/fixtures/umas.md");

/// Finds the umaLord folder: the one containing `account/support_cards.md`, or the game data
/// (`knowledge/data`) on a new install, before the app has written the account files.
/// Checks an explicit path, `UMALORD_ROOT`, the current dir, the exe's dir and their
/// ancestors, and finally the folder this crate was built from.
pub fn find_root(explicit: Option<PathBuf>) -> Option<PathBuf> {
    let mut starts: Vec<PathBuf> = Vec::new();
    starts.extend(explicit);
    starts.extend(std::env::var_os("UMALORD_ROOT").map(PathBuf::from));
    starts.extend(std::env::current_dir().ok());
    starts.extend(std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)));
    starts.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    starts
        .iter()
        .flat_map(|start| start.ancestors())
        .find(|dir| dir.join(CARDS_FILE).is_file() || dir.join("knowledge").join("data").join("skills.json").is_file())
        .map(Path::to_path_buf)
}

// ---------------------------------------------------------------------------
// Markdown parsing

pub(crate) struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    fn col(&self, name: &str) -> Option<usize> {
        self.headers.iter().position(|h| h.eq_ignore_ascii_case(name))
    }

    fn has_cols(&self, names: &[&str]) -> bool {
        names.iter().all(|n| self.col(n).is_some())
    }
}

fn split_row(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);
    line.split('|').map(|c| c.trim().to_string()).collect()
}

pub(crate) fn is_separator(line: &str) -> bool {
    let cells = split_row(line);
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

/// Every GitHub-style table in the document: a header row, a separator row, then body rows.
pub(crate) fn tables(md: &str) -> Vec<Table> {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        if lines[i].trim_start().starts_with('|') && is_separator(lines[i + 1]) {
            let headers = split_row(lines[i]);
            let mut rows = Vec::new();
            i += 2;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                rows.push(split_row(lines[i]));
                i += 1;
            }
            out.push(Table { headers, rows });
        } else {
            i += 1;
        }
    }
    out
}

/// `[text](url)` → `(text, url)`; anything else → `(cell, "")`.
fn parse_link(cell: &str) -> (String, String) {
    if let Some(rest) = cell.strip_prefix('[')
        && let Some((text, rest)) = rest.split_once("](")
        && let Some(url) = rest.strip_suffix(')')
    {
        return (text.to_string(), url.to_string());
    }
    (cell.to_string(), String::new())
}

/// Value of a `- Key: value` bullet, minus markdown bold and any trailing clause after a comma.
fn meta_value(md: &str, key: &str) -> Option<String> {
    let prefix = format!("- {key}:");
    md.lines().find_map(|line| {
        let value = line.trim().strip_prefix(&prefix)?.trim().replace("**", "");
        let value = value.split([',', '.']).next().unwrap_or(&value).trim().to_string();
        (!value.is_empty() && value != "—").then_some(value)
    })
}

/// The whole value of a `- Key: value` bullet, unless it's empty or "—".
fn meta_line(md: &str, key: &str) -> Option<String> {
    let prefix = format!("- {key}:");
    md.lines().find_map(|line| {
        let value = line.trim().strip_prefix(&prefix)?.trim().replace("**", "");
        (!value.is_empty() && value != "—").then_some(value)
    })
}

/// Set a `- Key: value` bullet, adding it at the top of the bullets if it's missing.
pub fn set_meta(md: &str, key: &str, value: &str) -> String {
    let newline = if md.contains("\r\n") { "\r\n" } else { "\n" };
    let prefix = format!("- {key}:");
    let mut lines: Vec<String> = md.lines().map(String::from).collect();
    let line = format!("{prefix} {}", if value.trim().is_empty() { "—" } else { value.trim() });
    match lines.iter().position(|l| l.trim_start().starts_with(&prefix)) {
        Some(i) => lines[i] = line,
        None => {
            let at = lines.iter().position(|l| l.starts_with("- ")).unwrap_or_else(|| lines.iter().position(|l| l.starts_with("# ")).map_or(0, |i| i + 2).min(lines.len()));
            lines.insert(at, line);
        }
    }
    let mut text = lines.join(newline);
    if md.ends_with('\n') {
        text.push_str(newline);
    }
    text
}

fn cell<'a>(row: &'a [String], idx: Option<usize>) -> &'a str {
    idx.and_then(|i| row.get(i)).map(String::as_str).unwrap_or("")
}

fn parse_cards(md: &str, roster: &mut Roster) {
    for table in tables(md) {
        if !table.has_cols(&["ID", "Card", "Type", "LB", "Level"]) {
            continue;
        }
        let (c_id, c_card, c_uma, c_type) = (table.col("ID"), table.col("Card"), table.col("Uma"), table.col("Type"));
        let (c_lb, c_level, c_notes) = (table.col("LB"), table.col("Level"), table.col("Notes"));

        for row in &table.rows {
            let (id_text, url) = parse_link(cell(row, c_id));
            let title = cell(row, c_card).to_string();
            let Ok(id) = id_text.parse::<u32>() else {
                roster.warnings.push(format!("Card row with unreadable ID: {}", row.join(" | ")));
                continue;
            };
            let Some(rarity) = Rarity::from_id(id) else {
                roster.warnings.push(format!("{id} {title}: ID doesn't encode a rarity"));
                continue;
            };
            let Some(card_type) = CardType::parse(cell(row, c_type)) else {
                roster.warnings.push(format!("{id} {title}: unknown type '{}'", cell(row, c_type)));
                continue;
            };
            let (Ok(lb), Ok(level)) = (cell(row, c_lb).parse::<u8>(), cell(row, c_level).parse::<u8>()) else {
                roster.warnings.push(format!("{id} {title}: unreadable LB or level"));
                continue;
            };
            if lb > 4 || level > rarity.max_level(lb) {
                roster.warnings.push(format!(
                    "{id} {title}: Lv{level} is above the {} cap of {} at LB{lb}",
                    rarity.label(),
                    rarity.max_level(lb)
                ));
            }
            roster.cards.push(SupportCard {
                id,
                title,
                uma: cell(row, c_uma).to_string(),
                card_type,
                rarity,
                lb: lb.min(4),
                level,
                notes: cell(row, c_notes).to_string(),
                url,
            });
        }
    }
}

fn parse_umas(md: &str, roster: &mut Roster) {
    for table in tables(md) {
        if !table.has_cols(&["ID", "Uma", "Stars", "Potential"]) {
            continue;
        }
        let (c_id, c_uma, c_stars, c_pot, c_notes) = (
            table.col("ID"),
            table.col("Uma"),
            table.col("Stars"),
            table.col("Potential"),
            table.col("Notes"),
        );
        let c_outfit = table.headers.iter().position(|h| h.to_ascii_lowercase().starts_with("outfit"));

        for row in &table.rows {
            let (id_text, url) = parse_link(cell(row, c_id));
            let name = cell(row, c_uma).to_string();
            let Ok(id) = id_text.parse::<u32>() else {
                roster.warnings.push(format!("Uma row with unreadable ID: {}", row.join(" | ")));
                continue;
            };
            let (Ok(stars), Ok(potential)) = (cell(row, c_stars).parse::<u8>(), cell(row, c_pot).parse::<u8>()) else {
                roster.warnings.push(format!("{id} {name}: unreadable stars or potential"));
                continue;
            };
            if !(1..=5).contains(&stars) || !(1..=5).contains(&potential) {
                roster.warnings.push(format!("{id} {name}: stars and potential must be 1–5"));
            }
            roster.umas.push(Uma {
                id,
                name,
                outfit: cell(row, c_outfit).to_string(),
                stars,
                potential,
                notes: cell(row, c_notes).to_string(),
                url,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Editing the card list

/// A support card row to add to (or update in) `account/support_cards.md`.
#[derive(Clone, Debug)]
pub struct CardRow {
    pub id: u32,
    pub title: String,
    pub uma: String,
    pub card_type: CardType,
    pub lb: u8,
    pub level: u8,
    /// Kept as is when empty and the card is already listed.
    pub notes: String,
}

pub fn card_url(id: u32) -> String {
    format!("https://uma.guide/support-cards/detail?id={id}")
}

fn type_order(t: &str) -> usize {
    CardType::parse(t).map_or(9, |t| CardType::ALL.iter().position(|x| *x == t).unwrap_or(9))
}

/// Add a card to its rarity's table, in type → LB → level order, or update the LB, level (and
/// notes, if given) of a listed one. Also stamps "Last updated" and refreshes the totals line
/// and the summary table. Returns the new text and whether the card was already listed.
pub fn upsert_card(md: &str, row: &CardRow, today: &str) -> Result<(String, bool), String> {
    let newline = if md.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = md.lines().map(String::from).collect();
    let rarity = Rarity::from_id(row.id).ok_or("That ID doesn't encode a rarity.")?;
    let cap = rarity.max_level(row.lb);
    if row.lb > 4 || row.level == 0 || row.level > cap {
        return Err(format!("Level must be 1–{cap} for {} at LB{}.", rarity.label(), row.lb));
    }
    let marker = format!("[{}](", row.id);
    let cell_for = |headers: &[String], row: &CardRow| -> Vec<String> {
        headers
            .iter()
            .map(|h| match h.to_ascii_lowercase().as_str() {
                "id" => format!("[{}]({})", row.id, card_url(row.id)),
                "card" => row.title.clone(),
                "uma" => row.uma.clone(),
                "type" => row.card_type.label().to_string(),
                "lb" => row.lb.to_string(),
                "level" => row.level.to_string(),
                "notes" => row.notes.clone(),
                _ => String::new(),
            })
            .collect()
    };
    let render = |cells: &[String]| format!("| {} |", cells.join(" | ")).replace("|  |", "| |");
    // Header of the table a body line belongs to.
    let header_above = |lines: &[String], i: usize| -> Option<Vec<String>> {
        (0..i).rev().find(|&k| k + 1 < lines.len() && lines[k].trim_start().starts_with('|') && is_separator(&lines[k + 1])).map(|k| split_row(&lines[k]))
    };
    let existing = lines.iter().position(|l| l.trim_start().starts_with('|') && split_row(l).first().is_some_and(|c| c.starts_with(&marker)));
    let updated = existing.is_some();
    if let Some(i) = existing {
        let headers = header_above(&lines, i).ok_or("Couldn't find the card table's header.")?;
        let mut cells = split_row(&lines[i]);
        cells.resize(headers.len(), String::new());
        for (k, h) in headers.iter().enumerate() {
            match h.to_ascii_lowercase().as_str() {
                "lb" => cells[k] = row.lb.to_string(),
                "level" => cells[k] = row.level.to_string(),
                "notes" if !row.notes.trim().is_empty() => cells[k] = row.notes.trim().to_string(),
                _ => {}
            }
        }
        lines[i] = render(&cells);
    } else {
        let heading = format!("## {}", rarity.label());
        let section = match lines.iter().position(|l| l.trim() == heading) {
            Some(s) => s,
            None => {
                if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                    lines.push(String::new());
                }
                lines.push(heading);
                lines.push(String::new());
                lines.push("| ID | Card | Uma | Type | LB | Level | Notes |".into());
                lines.push("|---|---|---|---|---|---|---|".into());
                lines.len() - 3
            }
        };
        let header_at = (section..lines.len())
            .find(|&k| k + 1 < lines.len() && lines[k].trim_start().starts_with('|') && is_separator(&lines[k + 1]))
            .ok_or("Couldn't find the card table under the rarity heading.")?;
        let headers = split_row(&lines[header_at]);
        let col = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
        let (c_type, c_lb, c_level) = (col("Type"), col("LB"), col("Level"));
        let key = |cells: &[String]| {
            let get = |c: Option<usize>| c.and_then(|i| cells.get(i)).cloned().unwrap_or_default();
            (type_order(&get(c_type)), std::cmp::Reverse(get(c_lb).parse::<u8>().unwrap_or(0)), std::cmp::Reverse(get(c_level).parse::<u8>().unwrap_or(0)))
        };
        let new_cells = cell_for(&headers, row);
        let new_key = key(&new_cells);
        let mut at = header_at + 2;
        while at < lines.len() && lines[at].trim_start().starts_with('|') {
            if key(&split_row(&lines[at])) > new_key {
                break;
            }
            at += 1;
        }
        lines.insert(at, render(&new_cells));
    }
    // "Last updated" gets today's date; the rest of that line stays.
    if let Some(l) = lines.iter_mut().find(|l| l.trim_start().starts_with("- Last updated:")) {
        let rest = l.split_once(',').map(|(_, r)| format!(",{r}")).unwrap_or_default();
        *l = format!("- Last updated: {today}{rest}");
    }
    let mut text = lines.join(newline);
    if md.ends_with('\n') {
        text.push_str(newline);
    }
    Ok((refresh_card_summary(&text), updated))
}

/// Recount the "- Totals:" line and the "Summary by type" table from the card tables.
pub fn refresh_card_summary(md: &str) -> String {
    let mut roster = Roster::default();
    parse_cards(md, &mut roster);
    let count = |r: Rarity, f: &dyn Fn(&SupportCard) -> bool| roster.cards.iter().filter(|c| c.rarity == r && f(c)).count();
    let newline = if md.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = md.lines().map(String::from).collect();
    if let Some(l) = lines.iter_mut().find(|l| l.trim_start().starts_with("- Totals:")) {
        let mut parts = Vec::new();
        for r in [Rarity::SSR, Rarity::SR, Rarity::R] {
            let n = count(r, &|_| true);
            if n > 0 {
                parts.push(format!("{n} {} ({} MLB)", r.label(), count(r, &|c| c.is_mlb())));
            }
        }
        let list = match parts.len() {
            0 => String::new(),
            1 => parts[0].clone(),
            n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
        };
        *l = format!("- Totals: {} cards. {list}.", roster.cards.len());
    }
    let summary = tables(md).into_iter().position(|t| t.headers.first().is_some_and(|h| h == "Type") && t.headers.iter().any(|h| h.starts_with("SSR")));
    if summary.is_some() {
        for line in lines.iter_mut() {
            let cells = split_row(line);
            let Some(t) = cells.first().and_then(|c| CardType::parse(c)) else { continue };
            if cells.len() != 6 || !line.trim_start().starts_with('|') {
                continue;
            }
            let of = |r: Rarity, f: &dyn Fn(&SupportCard) -> bool| count(r, &|c| c.card_type == t && f(c));
            let sr_dash = |n: usize| if n == 0 && matches!(t, CardType::Pal | CardType::Group) { "—".to_string() } else { n.to_string() };
            *line = format!(
                "| {} | {} | {} | {} | {} | {} |",
                t.label(),
                of(Rarity::SSR, &|c| c.lb >= 4),
                of(Rarity::SSR, &|c| (1..4).contains(&c.lb)),
                of(Rarity::SSR, &|c| c.lb == 0),
                sr_dash(of(Rarity::SR, &|c| c.lb >= 4)),
                sr_dash(of(Rarity::SR, &|c| c.lb < 4)),
            );
        }
    }
    let mut text = lines.join(newline);
    if md.ends_with('\n') {
        text.push_str(newline);
    }
    text
}

// ---------------------------------------------------------------------------
// Editing the uma list

/// An uma row to add to (or update in) `account/umas.md`.
#[derive(Clone, Debug)]
pub struct UmaRow {
    pub id: u32,
    pub name: String,
    pub outfit: String,
    pub stars: u8,
    pub potential: u8,
}

pub fn uma_url(id: u32) -> String {
    format!("https://uma.guide/characters/{id}")
}

/// The table line of the row whose ID cell links `id`, if any.
fn row_index(lines: &[String], id: u32) -> Option<usize> {
    let marker = format!("[{id}](");
    lines.iter().position(|l| l.trim_start().starts_with('|') && split_row(l).first().is_some_and(|c| c.starts_with(&marker)))
}

fn stamp_updated(lines: &mut [String], today: &str) {
    if let Some(l) = lines.iter_mut().find(|l| l.trim_start().starts_with("- Last updated:")) {
        let rest = l.split_once(',').map(|(_, r)| format!(",{r}")).unwrap_or_default();
        *l = format!("- Last updated: {today}{rest}");
    }
}

fn join_like(md: &str, lines: &[String]) -> String {
    let newline = if md.contains("\r\n") { "\r\n" } else { "\n" };
    let mut text = lines.join(newline);
    if md.ends_with('\n') {
        text.push_str(newline);
    }
    text
}

/// Add an uma to the table, which runs from 5★ down and by ID within a star count, or change
/// a listed one's stars and potential (it moves to its new place). Stamps "Last updated" and
/// recounts the totals line. Returns the new text and whether she was already listed.
pub fn upsert_uma(md: &str, row: &UmaRow, today: &str) -> Result<(String, bool), String> {
    if !(1..=5).contains(&row.stars) || !(1..=5).contains(&row.potential) {
        return Err("Stars and potential run 1–5.".into());
    }
    let mut lines: Vec<String> = md.lines().map(String::from).collect();
    let header_at = (0..lines.len())
        .find(|&k| k + 1 < lines.len() && lines[k].trim_start().starts_with('|') && is_separator(&lines[k + 1]) && {
            let h = split_row(&lines[k]);
            ["ID", "Uma", "Stars", "Potential"].iter().all(|n| h.iter().any(|x| x.eq_ignore_ascii_case(n)))
        })
        .ok_or("Couldn't find the uma table in account/umas.md.")?;
    let headers = split_row(&lines[header_at]);
    let col = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
    let (c_stars, c_id) = (col("Stars"), col("ID"));
    let existing = row_index(&lines, row.id);
    let mut cells: Vec<String> = headers
        .iter()
        .map(|h| match h.to_ascii_lowercase().as_str() {
            "id" => format!("[{}]({})", row.id, uma_url(row.id)),
            "uma" => row.name.clone(),
            "stars" => row.stars.to_string(),
            "potential" => row.potential.to_string(),
            h if h.starts_with("outfit") => row.outfit.clone(),
            _ => String::new(),
        })
        .collect();
    if let Some(i) = existing {
        // Keep whatever else the row says (notes, an edited outfit name).
        let old = split_row(&lines[i]);
        for (k, h) in headers.iter().enumerate() {
            if !matches!(h.to_ascii_lowercase().as_str(), "stars" | "potential") && let Some(v) = old.get(k).filter(|v| !v.is_empty()) {
                cells[k] = v.clone();
            }
        }
        lines.remove(i);
    }
    let key = |cells: &[String]| {
        let get = |c: Option<usize>| c.and_then(|i| cells.get(i)).cloned().unwrap_or_default();
        (std::cmp::Reverse(get(c_stars).parse::<u8>().unwrap_or(0)), parse_link(&get(c_id)).0.parse::<u32>().unwrap_or(0))
    };
    let new_key = key(&cells);
    let mut at = header_at + 2;
    while at < lines.len() && lines[at].trim_start().starts_with('|') {
        if key(&split_row(&lines[at])) > new_key {
            break;
        }
        at += 1;
    }
    lines.insert(at, format!("| {} |", cells.join(" | ")).replace("|  |", "| |"));
    stamp_updated(&mut lines, today);
    Ok((refresh_uma_summary(&join_like(md, &lines)), existing.is_some()))
}

/// Recount the "- Totals:" line of umas.md: "57 trainees. 12 at 5★, 23 at 4★. 42 are at Potential 5."
pub fn refresh_uma_summary(md: &str) -> String {
    let mut roster = Roster::default();
    parse_umas(md, &mut roster);
    let mut lines: Vec<String> = md.lines().map(String::from).collect();
    if let Some(l) = lines.iter_mut().find(|l| l.trim_start().starts_with("- Totals:")) {
        let n = roster.umas.len();
        let groups: Vec<String> = (1..=5u8).rev().filter_map(|s| {
            let k = roster.umas.iter().filter(|u| u.stars == s).count();
            (k > 0).then(|| format!("{k} at {s}★"))
        }).collect();
        let p5 = roster.umas.iter().filter(|u| u.potential >= 5).count();
        *l = if n == 0 { "- Totals: 0 trainees.".to_string() } else { format!("- Totals: {n} trainees. {}. {p5} are at Potential 5.", groups.join(", ")) };
    }
    join_like(md, &lines)
}

/// Take a card or uma off its list: the row whose ID cell links `id`. Stamps "Last updated" and
/// recounts the totals. Returns the new text, or None if it wasn't listed.
pub fn remove_row(md: &str, id: u32, today: &str) -> Option<String> {
    let mut lines: Vec<String> = md.lines().map(String::from).collect();
    let i = row_index(&lines, id)?;
    lines.remove(i);
    stamp_updated(&mut lines, today);
    let text = join_like(md, &lines);
    Some(if text.contains("## Summary by type") { refresh_card_summary(&text) } else { refresh_uma_summary(&text) })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMBEDDED_CARDS: &str = SAMPLE_CARDS;
    const EMBEDDED_UMAS: &str = SAMPLE_UMAS;

    /// Recounting the real file's totals and summary changes nothing, so edits keep its format.
    #[test]
    fn summary_refresh_is_stable_on_the_real_file() {
        assert_eq!(refresh_card_summary(EMBEDDED_CARDS), EMBEDDED_CARDS);
        assert_eq!(refresh_uma_summary(EMBEDDED_UMAS), EMBEDDED_UMAS);
        // Adding a card it already has at the same LB and level leaves the tables as they were.
        let first = Roster::parse(EMBEDDED_CARDS, EMBEDDED_UMAS).cards[0].clone();
        let row = CardRow { id: first.id, title: first.title, uma: first.uma, card_type: first.card_type, lb: first.lb, level: first.level, notes: String::new() };
        let (out, updated) = upsert_card(EMBEDDED_CARDS, &row, "2026-10-15").unwrap();
        assert!(updated);
        let strip = |s: &str| s.lines().filter(|l| !l.starts_with("- Last updated")).collect::<Vec<_>>().join("\n");
        assert_eq!(strip(&out), strip(EMBEDDED_CARDS));
        // The same for an uma.
        let u = Roster::sample().umas[0].clone();
        let (out, updated) = upsert_uma(EMBEDDED_UMAS, &UmaRow { id: u.id, name: u.name, outfit: u.outfit, stars: u.stars, potential: u.potential }, "2026-10-15").unwrap();
        assert!(updated);
        assert_eq!(strip(&out), strip(EMBEDDED_UMAS));
    }

    /// The blank files a new profile starts from parse, and fill and empty again cleanly.
    #[test]
    fn umas_and_cards_from_blank() {
        let umas = include_str!("../templates/account/umas.md");
        let cards = include_str!("../templates/account/support_cards.md");
        assert!(Roster::parse(cards, umas).is_empty());
        let oguri = UmaRow { id: 100601, name: "Oguri Cap".into(), outfit: "Starlight Beat".into(), stars: 3, potential: 1 };
        let (umas, added) = upsert_uma(umas, &oguri, "2026-10-18").unwrap();
        assert!(!added);
        let (umas, _) = upsert_uma(&umas, &UmaRow { id: 100101, name: "Special Week".into(), outfit: "Special Dreamer".into(), stars: 4, potential: 5 }, "2026-10-18").unwrap();
        // Raising Oguri to 5★ moves her to the top.
        let (umas, updated) = upsert_uma(&umas, &UmaRow { stars: 5, ..oguri }, "2026-10-18").unwrap();
        assert!(updated);
        let r = Roster::parse("", &umas);
        assert_eq!(r.umas.iter().map(|u| (u.id, u.stars)).collect::<Vec<_>>(), vec![(100601, 5), (100101, 4)]);
        assert!(umas.contains("- Totals: 2 trainees. 1 at 5★, 1 at 4★. 1 are at Potential 5."), "{umas}");
        assert!(umas.contains("- Last updated: 2026-10-18"));
        let row = CardRow { id: 30028, title: "Fire at My Heels".into(), uma: "Kitasan Black".into(), card_type: CardType::Speed, lb: 4, level: 50, notes: String::new() };
        let (cards, _) = upsert_card(cards, &row, "2026-10-18").unwrap();
        assert!(cards.contains("- Totals: 1 cards. 1 SSR (1 MLB)."), "{cards}");
        let cards = remove_row(&cards, 30028, "2026-10-18").unwrap();
        assert!(cards.contains("- Totals: 0 cards.") && Roster::parse(&cards, "").cards.is_empty(), "{cards}");
        let umas = remove_row(&umas, 100601, "2026-10-18").unwrap();
        assert_eq!(Roster::parse("", &umas).umas.len(), 1);
        assert!(remove_row(&umas, 100601, "x").is_none());
        let named = set_meta(&umas, "Profile", "Sam's account");
        assert_eq!(Roster::parse("", &named).profile.as_deref(), Some("Sam's account"));
    }

    #[test]
    fn adds_and_updates_cards() {
        let md = "- Last updated: 2026-10-15, from screenshots.\n- Totals: 1 cards. 1 SSR (1 MLB).\n\n## Summary by type\n\n| Type | SSR MLB | SSR LB1–3 | SSR LB0 | SR MLB | SR below MLB |\n|---|---|---|---|---|---|\n| Speed | 1 | 0 | 0 | 0 | 0 |\n| Wit | 0 | 0 | 0 | 0 | 0 |\n\n## SSR\n\n| ID | Card | Uma | Type | LB | Level | Notes |\n|---|---|---|---|---|---|---|\n| [30076](x) | Searching for Unseen Sights | Silence Suzuka | Speed | 4 | 50 | |\n| [30010](x) | Wave of Gratitude | Fine Motion | Wit | 4 | 50 | |\n";
        let row = CardRow { id: 30028, title: "Fire at My Heels".into(), uma: "Kitasan Black".into(), card_type: CardType::Speed, lb: 2, level: 40, notes: String::new() };
        let (out, updated) = upsert_card(md, &row, "2026-10-17").unwrap();
        assert!(!updated);
        let r = Roster::parse(&out, "");
        let ids: Vec<u32> = r.cards.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![30076, 30028, 30010], "speed cards together, MLB first:\n{out}");
        assert!(out.contains("- Last updated: 2026-10-17, from screenshots."));
        assert!(out.contains("- Totals: 3 cards. 3 SSR (2 MLB)."), "{out}");
        assert!(out.contains("| Speed | 1 | 1 | 0 | 0 | 0 |"), "{out}");
        // A new copy: same card, higher LB.
        let row = CardRow { lb: 4, level: 50, ..row };
        let (out, updated) = upsert_card(&out, &row, "2026-10-17").unwrap();
        assert!(updated);
        let kita = Roster::parse(&out, "").cards.into_iter().find(|c| c.id == 30028).unwrap();
        assert_eq!((kita.lb, kita.level), (4, 50));
        // An SR when there's no SR section yet gets one.
        let sr = CardRow { id: 20039, title: "An Annoying Supervisor".into(), uma: "Vodka".into(), card_type: CardType::Speed, lb: 0, level: 25, notes: "new".into() };
        let (out, _) = upsert_card(&out, &sr, "2026-10-17").unwrap();
        assert!(out.contains("## SR") && Roster::parse(&out, "").cards.iter().any(|c| c.id == 20039 && c.notes == "new"));
        assert!(upsert_card(&out, &CardRow { level: 51, ..sr }, "x").is_err(), "level above the cap");
    }

    const CARDS: &str = "\
- Last updated: 2026-10-15, from screenshots.

| Rarity | LB0 | LB1 |
|---|---|---|
| SSR | 30 | 35 |

## SSR

| ID | Card | Uma | Type | LB | Level | Notes |
|---|---|---|---|---|---|---|
| [30076](https://uma.guide/support-cards/detail?id=30076) | Searching for Unseen Sights | Silence Suzuka | Speed | 4 | 50 | |
| [20046](https://uma.guide/support-cards/detail?id=20046) | Reckless Witch Training | Sweep Tosho | Wit | 0 | 25 | note |
| [30001](https://x) | Over Cap | Special Week | Guts | 0 | 35 | |
";

    const UMAS: &str = "\
- Server: **Global (English)**. Use Global data.

| ID | Uma | Outfit / title | Stars | Potential | Notes |
|---|---|---|---|---|---|
| [100601](https://uma.guide/characters/100601) | Oguri Cap | Starlight Beat | 5 | 5 | |
";

    #[test]
    fn parses_cards_by_header() {
        let r = Roster::parse(CARDS, UMAS);
        assert_eq!(r.cards.len(), 3, "the LB-cap table must not be read as cards");
        let suzuka = &r.cards[0];
        assert_eq!((suzuka.id, suzuka.rarity, suzuka.card_type, suzuka.lb, suzuka.level), (30076, Rarity::SSR, CardType::Speed, 4, 50));
        assert_eq!(suzuka.url, "https://uma.guide/support-cards/detail?id=30076");
        assert_eq!(r.cards[1].rarity, Rarity::SR);
        assert_eq!(r.cards[1].notes, "note");
        assert_eq!(r.cards_updated.as_deref(), Some("2026-10-15"));
    }

    #[test]
    fn flags_level_above_lb_cap() {
        let r = Roster::parse(CARDS, UMAS);
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].contains("Over Cap"));
    }

    #[test]
    fn parses_umas_and_server() {
        let r = Roster::parse(CARDS, UMAS);
        assert_eq!(r.umas.len(), 1);
        assert_eq!((r.umas[0].stars, r.umas[0].potential), (5, 5));
        assert_eq!(r.umas[0].outfit, "Starlight Beat");
        assert_eq!(r.server.as_deref(), Some("Global (English)"));
    }

    #[test]
    fn level_caps() {
        assert_eq!(Rarity::SSR.max_level(0), 30);
        assert_eq!(Rarity::SSR.max_level(4), 50);
        assert_eq!(Rarity::SR.max_level(4), 45);
        assert_eq!(Rarity::R.max_level(2), 30);
    }

    /// The account files must parse cleanly (they may be empty); a warning here means a bad row.
    #[test]
    fn account_files_parse_cleanly() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let live = Roster::load(Some(root)).0;
        assert!(live.warnings.is_empty(), "{:#?}", live.warnings);
        let r = Roster::parse(EMBEDDED_CARDS, EMBEDDED_UMAS);
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
        assert!(!r.cards.is_empty() && !r.umas.is_empty());
        let mut ids: Vec<u32> = r.cards.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), r.cards.len(), "duplicate card IDs");
    }
}
