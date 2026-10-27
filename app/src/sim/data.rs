//! Moomoolator's Global data, as `tools/update_knowledge.py` saves it in `knowledge/data/sim/`:
//! skill definitions with raw activation conditions, skill groups, courses and outfits.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use serde::Deserialize;

use super::course::{Course, RawCourse};

pub const SIM_DIR: &str = "knowledge/data/sim";

#[derive(Deserialize, Clone, Debug)]
pub struct RawEffect {
    #[serde(rename = "type")]
    pub kind: i32,
    pub modifier: f64,
    #[serde(default)]
    pub target: i32,
    #[serde(default)]
    pub scaling: Option<i32>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SkillAlternative {
    #[serde(default)]
    pub precondition: String,
    #[serde(default)]
    pub condition: String,
    #[serde(rename = "baseDuration", default)]
    pub base_duration: f64,
    #[serde(rename = "durationScaling", default)]
    pub duration_scaling: Option<i32>,
    #[serde(default)]
    pub effects: Vec<RawEffect>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SkillDef {
    pub rarity: i32,
    pub alternatives: Vec<SkillAlternative>,
}

#[derive(Deserialize, Clone, Debug)]
struct RawMeta {
    #[serde(rename = "groupId")]
    group_id: String,
    #[serde(rename = "iconId", default)]
    icon_id: String,
}

#[derive(Clone, Debug)]
pub struct SkillMeta {
    pub group_id: u32,
    /// Icon ids starting with 3 are debuffs, which a build may hold several copies of.
    pub debuff: bool,
}

#[derive(Default)]
pub struct SimData {
    pub skills: HashMap<u32, SkillDef>,
    pub meta: HashMap<u32, SkillMeta>,
    pub names: HashMap<u32, String>,
    pub courses: BTreeMap<u32, Course>,
    pub tracks: BTreeMap<u32, String>,
    /// Skills fast-forwarded from JP: in the data, but not on Global yet.
    pub not_in_game: HashSet<u32>,
}

fn read<T: for<'de> Deserialize<'de>>(dir: &Path, file: &str) -> Result<T, String> {
    let text = std::fs::read_to_string(dir.join(file)).map_err(|e| format!("{file}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{file}: {e}"))
}

fn ids<V>(map: HashMap<String, V>) -> impl Iterator<Item = (u32, V)> {
    map.into_iter()
        .filter_map(|(k, v)| Some((k.parse().ok()?, v)))
}

impl SimData {
    pub fn load(root: &Path) -> Result<SimData, String> {
        let dir = root.join(SIM_DIR);
        let skills: HashMap<String, SkillDef> = read(&dir, "skill_data.json")?;
        let meta: HashMap<String, RawMeta> = read(&dir, "skill_meta.json")?;
        let courses: HashMap<String, RawCourse> = read(&dir, "course_data.json")?;
        let names: HashMap<String, Vec<String>> = read(&dir, "skillnames.json").unwrap_or_default();
        let tracks: HashMap<String, Vec<String>> =
            read(&dir, "tracknames.json").unwrap_or_default();
        #[derive(Deserialize, Default)]
        struct NotInGame {
            #[serde(default)]
            skills: Vec<String>,
        }
        let nig: NotInGame = read(&dir, "not-in-game.json").unwrap_or_default();
        Ok(SimData {
            skills: ids(skills).collect(),
            meta: ids(meta)
                .filter_map(|(id, m)| {
                    Some((
                        id,
                        SkillMeta {
                            group_id: m.group_id.parse().ok()?,
                            debuff: m.icon_id.starts_with('3'),
                        },
                    ))
                })
                .collect(),
            names: ids(names)
                .filter_map(|(id, n)| Some((id, n.into_iter().next()?)))
                .collect(),
            courses: ids(courses)
                .map(|(id, c)| (id, Course::from_raw(&c)))
                .collect(),
            tracks: ids(tracks)
                .filter_map(|(id, n)| Some((id, n.into_iter().rev().find(|s| !s.is_empty())?)))
                .collect(),
            not_in_game: nig.skills.iter().filter_map(|s| s.parse().ok()).collect(),
        })
    }

    pub fn group_of(&self, id: u32) -> u32 {
        self.meta.get(&id).map_or(id / 10, |m| m.group_id)
    }

    pub fn is_debuff(&self, id: u32) -> bool {
        self.meta.get(&id).is_some_and(|m| m.debuff)
    }

    pub fn skill_name(&self, id: u32) -> String {
        self.names
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("Skill {id}"))
    }

    /// "Nakayama Turf 2500m (inner)".
    pub fn course_name(&self, id: u32) -> String {
        let Some(c) = self.courses.get(&id) else {
            return format!("Course {id}");
        };
        let track = self
            .tracks
            .get(&c.race_track_id)
            .cloned()
            .unwrap_or_else(|| format!("Track {}", c.race_track_id));
        let layout = match c.layout {
            2 => " (inner)",
            3 => " (outer)",
            4 => " (outer→inner)",
            _ => "",
        };
        format!(
            "{track} {} {}m{layout}",
            if c.surface == 2 { "Dirt" } else { "Turf" },
            c.distance
        )
    }

    /// The unique skill id an outfit has at a star count (uniqueSkillForUma in HorseDefTypes.ts):
    /// 1xxxx1 from 3★, 1xxxx-style 10xxx1 below.
    pub fn unique_skill(&self, outfit: u32, stars: u8) -> Option<u32> {
        let s = outfit.to_string();
        if s.len() < 4 {
            return None;
        }
        let i: u32 = s[1..s.len() - 2].parse().ok()?;
        let v: u32 = s[s.len() - 2..].parse().ok()?;
        let sid = 10_000 * (1 + 9 * u32::from(stars > 2)) + 10_000 * v.checked_sub(1)? + i * 10 + 1;
        self.skills.contains_key(&sid).then_some(sid)
    }
}
