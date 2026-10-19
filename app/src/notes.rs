//! What the app reads from the community knowledge files: the skill tier list
//! (`knowledge/data/skill_tiers.json`, from the community skills spreadsheet) and, from
//! `knowledge/mechanics.md`, the sourced glossary (Help) and the "Did you know?" tips (Home).

use std::collections::HashMap;

use serde::Deserialize;

/// One skill's tiers on the community list: Team Trials and Champions Meeting.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SkillTier {
    pub id: u32,
    #[serde(default)]
    pub tt: String,
    #[serde(default)]
    pub cm: String,
    #[serde(default)]
    pub score_per_sp: Option<f64>,
    /// The style or distance the sheet files it under ("Pace Chaser", "Mile"), if any.
    #[serde(default, rename = "for")]
    pub target: String,
    #[serde(default)]
    pub why: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SkillTiers {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub authors: String,
    #[serde(default)]
    pub sheet_updated: String,
    #[serde(default)]
    pub caveat: String,
    /// Symbol → meaning.
    #[serde(default)]
    pub scale: HashMap<String, String>,
    #[serde(default)]
    skills: Vec<SkillTier>,
    #[serde(skip)]
    by_id: HashMap<u32, usize>,
}

impl SkillTiers {
    pub fn load(path: &std::path::Path) -> Option<SkillTiers> {
        let mut t: SkillTiers = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        t.by_id = t.skills.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
        Some(t)
    }

    pub fn get(&self, id: u32) -> Option<&SkillTier> {
        self.by_id.get(&id).map(|i| &self.skills[*i])
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// A line for hovers and the assistant: both tiers spelled out, the reason, and the source.
    pub fn explain(&self, t: &SkillTier) -> String {
        let mean = |s: &str| self.scale.get(s).map_or(String::new(), |m| format!(" ({m})"));
        let target = if t.target.is_empty() { String::new() } else { format!(", filed under {}", t.target) };
        let score = t.score_per_sp.map(|s| format!(" Team Trials points per SP: {s:.2}.")).unwrap_or_default();
        format!(
            "Community tier list{target}: Team Trials {}{}, Champions Meeting {}{}. Why: {}.{score} Source: {}, sheet updated {}. Community opinion.",
            t.tt,
            mean(&t.tt),
            t.cm,
            mean(&t.cm),
            if t.why.is_empty() { "no reason given" } else { t.why.trim_end_matches('.') },
            self.authors,
            self.sheet_updated
        )
    }
}

/// A tier mark's place on the sheet's scale, best first (unknown marks last).
pub fn tier_rank(mark: &str) -> u8 {
    ["⍟", "◎", "◯", "▲", "△", "✕"].iter().position(|m| *m == mark).map_or(8, |i| i as u8)
}

/// A glossary term from `mechanics.md`.
#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub term: String,
    pub meaning: String,
    pub source: String,
}

/// A "Did you know?" tip from `mechanics.md`.
#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub text: String,
    pub source: String,
}

/// The text of one `## ` section, without its heading.
fn section<'a>(md: &'a str, heading: &str) -> Option<&'a str> {
    let start = md.find(&format!("\n## {heading}\n"))? + heading.len() + 5;
    let rest = &md[start..];
    Some(rest.find("\n## ").map_or(rest, |end| &rest[..end]))
}

/// The Glossary table: Term | Meaning | Source.
pub fn parse_terms(md: &str) -> Vec<Term> {
    // A Windows checkout (and the release zip built from one) has CRLF line endings.
    let md = md.replace("\r\n", "\n");
    let Some(text) = section(&md, "Glossary") else { return Vec::new() };
    let Some(table) = crate::data::tables(text).into_iter().next() else { return Vec::new() };
    let col = |name: &str| table.headers.iter().position(|h| h.eq_ignore_ascii_case(name));
    let (Some(t), Some(m)) = (col("Term"), col("Meaning")) else { return Vec::new() };
    let s = col("Source");
    table
        .rows
        .iter()
        .filter_map(|r| {
            let term = r.get(t)?.trim().to_string();
            let meaning = r.get(m)?.trim().to_string();
            (!term.is_empty() && !meaning.is_empty()).then(|| Term { term, meaning, source: s.and_then(|i| r.get(i)).map(|x| x.trim().to_string()).unwrap_or_default() })
        })
        .collect()
}

/// The "Did you know?" list: "- tip [source]".
pub fn parse_tips(md: &str) -> Vec<Tip> {
    let md = md.replace("\r\n", "\n");
    let Some(text) = section(&md, "Did you know?") else { return Vec::new() };
    text.lines()
        .filter_map(|l| l.trim().strip_prefix("- "))
        .filter_map(|l| {
            let (text, source) = match l.rfind(" [") {
                Some(i) if l.ends_with(']') => (&l[..i], &l[i + 2..l.len() - 1]),
                _ => (l, ""),
            };
            (!text.trim().is_empty()).then(|| Tip { text: text.trim().to_string(), source: source.trim().to_string() })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_glossary_and_tips() {
        let md = "# M\n\n## Glossary\n\nIntro.\n\n| Term | Meaning | Source |\n|---|---|---|\n| Lengths (L) | 2.5 m. | GameTora |\n| Rushed | Burns HP. | GameTora, Race Mechanics |\n\n## Did you know?\n\nIntro.\n\n- Past 1200 a stat counts half. [GameTora, Race Mechanics Handbook]\n- No source here\n\n## Next\n";
        let terms = parse_terms(md);
        assert_eq!(terms.len(), 2);
        assert_eq!(terms[1], Term { term: "Rushed".into(), meaning: "Burns HP.".into(), source: "GameTora, Race Mechanics".into() });
        let tips = parse_tips(md);
        assert_eq!(tips, vec![Tip { text: "Past 1200 a stat counts half.".into(), source: "GameTora, Race Mechanics Handbook".into() }, Tip { text: "No source here".into(), source: String::new() }]);
        // The same file with Windows line endings, as a CRLF checkout has it.
        let crlf = md.replace('\n', "\r\n");
        assert_eq!(parse_terms(&crlf), terms);
        assert_eq!(parse_tips(&crlf), tips);
    }

    /// The real files: every glossary term and tip has a source; the tier list covers the
    /// common skills.
    #[test]
    fn real_notes_are_sourced() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let md = std::fs::read_to_string(root.join("knowledge/mechanics.md")).unwrap();
        let terms = parse_terms(&md);
        assert!(terms.len() >= 40, "{} terms", terms.len());
        assert!(terms.iter().all(|t| !t.source.is_empty()), "a term without a source");
        let tips = parse_tips(&md);
        assert!(tips.len() >= 15 && tips.iter().all(|t| !t.source.is_empty()), "every tip needs a source");
        let tiers = SkillTiers::load(&root.join("knowledge/data/skill_tiers.json")).expect("skill_tiers.json");
        assert!(tiers.len() > 400);
        let corners = tiers.get(201322).expect("Pace Chaser Corners ○");
        assert_eq!((corners.cm.as_str(), corners.target.as_str()), ("⍟", "Pace Chaser"));
        assert!(tiers.explain(corners).contains("Community opinion"));
    }
}
