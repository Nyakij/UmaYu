//! The generated part of `knowledge/racecourses.md`: every course a Champions Meeting has used or
//! is expected to use, from the simulator's course data (Moomoolator's `course_data.json`) and
//! the event presets, with a stamina estimate per style from the HP calculator (`sim::hpcalc`).
//!
//! `cargo test --release course_notes -- --ignored` rewrites the block between the markers. The
//! app itself only uses `corner_numbers`; the generator runs from that test.
#![cfg_attr(not(test), allow(dead_code, unused_imports))]

use std::fmt::Write;

use crate::App;
use crate::sim::course::{Course, phase_start};

pub(crate) const BEGIN: &str =
    "<!-- generated: course notes (cargo test --release course_notes -- --ignored) -->";
pub(crate) const END: &str = "<!-- end of generated course notes -->";

const STYLES: [&str; 4] = ["Front", "Pace", "Late", "End"];
const STATS: [&str; 5] = ["Speed", "Stamina", "Power", "Guts", "Wit"];

fn m(x: f64) -> String {
    format!("{x:.0}")
}

/// Corner numbers as skill conditions count them: the last corner is 4, then backwards, looping
/// after 1 (GameTora's Race Mechanics Handbook, "Corner Numbering").
pub(crate) fn corner_numbers(n: usize) -> Vec<u8> {
    (0..n).map(|i| 4 - ((n - 1 - i) % 4) as u8).collect()
}

fn course_section(app: &App, id: u32, c: &Course, out: &mut String) {
    let data = app.sim_data.as_ref().expect("sim data");
    let d = c.distance;
    let direction = match c.turn {
        1 => "right-handed (clockwise)",
        2 => "left-handed (counter-clockwise)",
        _ => "straight",
    };
    let category = ["", "Sprint", "Mile", "Medium", "Long"]
        .get(c.distance_type.clamp(0, 4) as usize)
        .copied()
        .unwrap_or("");
    let surface = if c.surface == 2 { "dirt" } else { "turf" };
    let _ = writeln!(out, "### {} (course {id})\n", data.course_name(id));
    let _ = writeln!(out, "- {category}, {surface}, {direction}, {} m.", m(d));
    let _ = writeln!(
        out,
        "- Phases: early 0–{}, mid {}–{}, late {}–{}, last spurt {}–{} m. The last spurt mechanic (the accel zone) starts at {} m.",
        m(phase_start(d, 1)),
        m(phase_start(d, 1)),
        m(phase_start(d, 2)),
        m(phase_start(d, 2)),
        m(phase_start(d, 3)),
        m(phase_start(d, 3)),
        m(d),
        m(phase_start(d, 2))
    );
    if c.corners.is_empty() {
        let _ = writeln!(out, "- No corners.");
    } else {
        let nums = corner_numbers(c.corners.len());
        let list: Vec<String> = c
            .corners
            .iter()
            .zip(&nums)
            .map(|(k, n)| format!("{n}: {}–{}", m(k.start), m(k.start + k.length)))
            .collect();
        let _ = writeln!(
            out,
            "- Corners, numbered as skills count them: {} m.",
            list.join("; ")
        );
    }
    let straights: Vec<String> = c
        .straights
        .iter()
        .map(|s| format!("{}–{}", m(s.start), m(s.end)))
        .collect();
    let fin = c.straights.last().map(|s| s.start).unwrap_or(d);
    let rel = fin - phase_start(d, 2);
    let when = if rel.abs() < 0.5 {
        "exactly where the late race starts".to_string()
    } else if rel < 0.0 {
        format!(
            "{} m before the late race starts, so skills that fire on it carry speed into the spurt",
            m(-rel)
        )
    } else {
        format!("{} m into the late race", m(rel))
    };
    let _ = writeln!(
        out,
        "- Straights: {} m. The final straight starts at {} m, {when}.",
        straights.join("; "),
        m(fin)
    );
    if c.slopes.is_empty() {
        let _ = writeln!(out, "- Flat: no slopes.");
    } else {
        let list: Vec<String> = c
            .slopes
            .iter()
            .map(|s| {
                format!(
                    "{} {}–{}",
                    if s.slope > 0.0 { "uphill" } else { "downhill" },
                    m(s.start),
                    m(s.start + s.length)
                )
            })
            .collect();
        let _ = writeln!(out, "- Slopes: {} m.", list.join("; "));
    }
    let stats: Vec<&str> = c
        .course_set_status
        .iter()
        .filter_map(|s| STATS.get((*s - 1).clamp(0, 4) as usize).copied())
        .collect();
    let _ = writeln!(
        out,
        "- Speed bonus stats: {}.",
        if stats.is_empty() {
            "none".to_string()
        } else {
            format!(
                "{} (+5% Speed per 300 of the stat, up to 901, averaged over the stats)",
                stats.join(" and ")
            )
        }
    );
    let mut events: Vec<&crate::knowledge::EventPreset> = app
        .kb
        .umalator_presets
        .iter()
        .filter(|e| e.course == id)
        .collect();
    events.sort_by(|a, b| a.date.cmp(&b.date));
    if !events.is_empty() {
        let list: Vec<String> = events
            .iter()
            .map(|e| {
                let kind = if e.is_cm() {
                    format!("CM {}", e.id)
                } else {
                    format!("{} {}", e.event, e.id)
                };
                format!(
                    "{kind} {} ({}, {}{})",
                    e.name,
                    e.date,
                    e.conditions,
                    if e.confirmed { "" } else { ", not confirmed" }
                )
            })
            .collect();
        let _ = writeln!(
            out,
            "- Events here (Umalator's presets): {}.",
            list.join("; ")
        );
    }
    let _ = writeln!(
        out,
        "- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):\n"
    );
    let _ = writeln!(out, "  | Speed | {} |", STYLES.join(" | "));
    let _ = writeln!(out, "  |---|---|---|---|---|");
    for speed in [1200.0, 1600.0] {
        let cells: Vec<String> = (0..4)
            .map(|style| {
                let e = crate::sim::hpcalc::estimate(c, 600.0, 600.0, 1200.0, speed, style, 0.0);
                format!("{} / {}", e.min_stamina, e.adjusted_min_stamina)
            })
            .collect();
        let _ = writeln!(out, "  | {} | {} |", m(speed), cells.join(" | "));
    }
    let _ = writeln!(out);
}

/// The block: one section per course, Champions Meeting courses in course-id order.
pub(crate) fn course_notes(app: &App) -> String {
    let data = app.sim_data.as_ref().expect("sim data");
    let mut ids: Vec<u32> = app
        .kb
        .umalator_presets
        .iter()
        .map(|e| e.course)
        .filter(|id| data.courses.contains_key(id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let mut out = String::new();
    let _ = writeln!(out, "{BEGIN}\n");
    let _ = writeln!(
        out,
        "Generated {} from Moomoolator's course data (`knowledge/data/sim/course_data.json`, data) and event presets (`knowledge/data/umalator.json`); stamina rows are model output. {} courses.\n",
        crate::ui_util::today(),
        ids.len()
    );
    for id in ids {
        course_section(app, id, &data.courses[&id], &mut out);
    }
    let _ = write!(out, "{END}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_count_back_from_four() {
        assert_eq!(corner_numbers(2), [3, 4]);
        assert_eq!(corner_numbers(4), [1, 2, 3, 4]);
        // Sapporo turf 2600 (GameTora's example): six corners, 3 4 1 2 3 4.
        assert_eq!(corner_numbers(6), [3, 4, 1, 2, 3, 4]);
    }

    /// Rewrite the generated block of knowledge/racecourses.md.
    #[test]
    #[ignore]
    fn course_notes_regenerate() {
        let a = App::for_tests();
        let block = course_notes(&a);
        let path = a
            .root
            .as_ref()
            .unwrap()
            .join("knowledge")
            .join("racecourses.md");
        let text = std::fs::read_to_string(&path).unwrap();
        let start = text.find(BEGIN).expect("the start marker");
        let end = text.find(END).expect("the end marker") + END.len();
        let new = format!("{}{block}{}", &text[..start], &text[end..]);
        std::fs::write(&path, new).unwrap();
        println!("{block}");
    }

    /// The block names every CM course and gives each a stamina table.
    #[test]
    fn course_notes_cover_every_cm_course() {
        let a = App::for_tests();
        let block = course_notes(&a);
        assert!(block.starts_with(BEGIN) && block.ends_with(END));
        assert!(
            block.contains("Chukyo Turf 1200m") && block.contains("CM 21 Capricorn Cup"),
            "{block}"
        );
        let sections = block.matches("### ").count();
        assert_eq!(
            block
                .matches("| Speed | Front | Pace | Late | End |")
                .count(),
            sections
        );
    }
}
