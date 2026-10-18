//! Engine checks against the real data in knowledge/data/sim/.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use super::builder::build_skill_data;
use super::data::SimData;
use super::params::{HorseDesc, SASI, base_stats, parse_aptitude};
use super::run::{Progress, SimOptions, UmaBuild, race_params, run_chart, run_comparison, run_hpcalc};
use super::solver::Perspective;

fn data() -> SimData {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    SimData::load(root).expect("knowledge/data/sim present")
}

fn build(stamina: f64, skills: Vec<u32>) -> UmaBuild {
    UmaBuild {
        outfit: Some(100101),
        stars: 5,
        unique_level: 4,
        desc: HorseDesc {
            speed: 1200.0,
            stamina,
            power: 900.0,
            guts: 400.0,
            wisdom: 600.0,
            strategy: SASI,
            distance_apt: parse_aptitude("A"),
            surface_apt: parse_aptitude("A"),
            strategy_apt: parse_aptitude("A"),
            mood: 2,
        },
        skills,
        forced: Default::default(),
    }
}

#[test]
fn every_skill_builds() {
    let d = data();
    let course = &d.courses[&10506];
    let race = race_params(1, 1, 4, 2, None);
    let horse = base_stats(&build(800.0, vec![]).desc);
    let mut failed = Vec::new();
    for id in d.skills.keys() {
        if let Err(e) = build_skill_data(&d, &horse, &race, course, *id, Perspective::Mine, 1) {
            failed.push(format!("{id}: {e}"));
        }
    }
    // A handful of JP-only skills use comparisons the engine doesn't model either.
    assert!(failed.len() < 15, "{} skills failed: {failed:#?}", failed.len());
}

#[test]
fn identical_builds_tie_and_skills_help() {
    let d = data();
    let course = Arc::new(d.courses[&10506].clone());
    let race = race_params(1, 1, 4, 2, None);
    let unique = d.unique_skill(100101, 5).expect("Special Week's unique");
    let a = build(900.0, vec![unique]);
    let opts = SimOptions { samples: 300, ..Default::default() };
    let t = Instant::now();
    let r = run_comparison(&d, &course, &race, &a, &a, &opts, true, true, None);
    eprintln!("300 compare runs in {:?}; gaps mean {:.3} median {:.3}", t.elapsed(), r.mean, r.median);
    assert!(r.median.abs() < 0.05 && r.mean.abs() < 0.5, "identical builds should tie: {r:?}", r = (r.mean, r.median));
    // Raw simulated seconds (about distance / base speed), not the game's displayed time.
    assert!(r.umas[0].mean_time > 115.0 && r.umas[0].mean_time < 135.0, "2500 m in {:.1}s", r.umas[0].mean_time);
    assert!(!r.median_run[0].pos.is_empty());

    // A gold speed skill: Unrestrained / Beeline Burst style late-race target speed boosts help.
    let mut b = a.clone();
    let speed_gold = d
        .skills
        .iter()
        .filter(|(id, s)| s.rarity == 2 && !d.not_in_game.contains(id) && s.alternatives.iter().any(|alt| alt.condition == "phase>=2&order_rate<=50" || alt.condition.starts_with("is_finalcorner==1&corner==0")))
        .map(|(id, _)| *id)
        .next();
    if let Some(id) = speed_gold {
        b.skills.push(id);
        let r = run_comparison(&d, &course, &race, &a, &b, &opts, true, false, None);
        eprintln!("{} gains {:.2} L mean", d.skill_name(id), r.mean);
        assert!(r.mean > 0.0);
    }
}

/// Cross-check against umalator.app (2026-10-17, seed 12345, 1000 runs, Nakayama 2500 inner,
/// Firm/Sunny/Winter/Midday): Special Week 5★ unique Lv4, 1200/900/900/400/600 Late A/A/A, vs the
/// same plus Hot Pursuit and Swinging Maestro. Moomoolator showed mean +1.21 L, median +1.04 L,
/// full spurt 88% / 97%, HP survival 82.9% / 96.1%, rushed 13.9%.
#[test]
fn matches_moomoolator() {
    let d = data();
    let course = Arc::new(d.courses[&10506].clone());
    let race = race_params(1, 1, 4, 2, None);
    let a = build(900.0, vec![100011]);
    let mut b = a.clone();
    b.skills.extend([202471, 200351]);
    let opts = SimOptions { samples: 1000, seed: 12345.0, ..Default::default() };
    let r = run_comparison(&d, &course, &race, &a, &b, &opts, true, false, None);
    let (u1, u2) = (&r.umas[0], &r.umas[1]);
    eprintln!(
        "mean {:.2} median {:.2} min {:.2} max {:.2}; spurt {:.0}/{:.0}; survival {:.1}/{:.1}; rushed {:.1}",
        r.mean, r.median, r.min, r.max, u1.full_spurt_rate, u2.full_spurt_rate, u1.survival_rate, u2.survival_rate, u1.rushed_rate
    );
    // The port gave mean 1.21, median 1.04, spurt 88/97, survival 83.3/96.1, rushed 13.9.
    assert!((r.mean - 1.21).abs() < 0.05 && (r.median - 1.04).abs() < 0.05, "gap");
    assert!((u1.full_spurt_rate - 88.0).abs() < 1.5 && (u2.full_spurt_rate - 97.0).abs() < 1.5, "full spurt");
    assert!((u1.survival_rate - 82.9).abs() < 1.5 && (u2.survival_rate - 96.1).abs() < 1.5, "survival");
    assert!((u1.rushed_rate - 13.9).abs() < 1.0, "rushed");
}

#[test]
fn stamina_decides_the_spurt() {
    let d = data();
    let course = Arc::new(d.courses[&10506].clone());
    let race = race_params(1, 1, 4, 2, None);
    let opts = SimOptions { samples: 200, ..Default::default() };
    let low = run_comparison(&d, &course, &race, &build(500.0, vec![]), &build(1100.0, vec![]), &opts, true, false, None);
    eprintln!("full spurt: 500 sta {:.0}%, 1100 sta {:.0}%; gap {:.2} L", low.umas[0].full_spurt_rate, low.umas[1].full_spurt_rate, low.mean);
    assert!(low.umas[0].full_spurt_rate < 20.0 && low.umas[1].full_spurt_rate > 80.0);
    assert!(low.mean > 1.0, "the stamina build finishes ahead");
    let hp = run_hpcalc(&d, &course, &race, &build(1100.0, vec![]), &opts, false, None);
    assert!(hp.remaining[hp.remaining.len() / 2] > 0.0 && hp.full_spurt_runs > 150);
}

#[test]
fn chart_ranks_skills() {
    let d = data();
    let course = Arc::new(d.courses[&10506].clone());
    let race = race_params(1, 1, 4, 2, Some(SASI));
    let base = build(900.0, vec![]);
    // Every Global skill a Late Surger can use on this course, a sample of 40.
    let horse = base_stats(&base.desc);
    let candidates: Vec<u32> = d
        .skills
        .keys()
        .copied()
        .filter(|id| !d.not_in_game.contains(id) && *id >= 200000 && *id < 300000)
        .filter(|id| build_skill_data(&d, &horse, &race, &course, *id, Perspective::Mine, 1).is_ok_and(|v| v.iter().any(|s| s.regions[0].start < 9999.0)))
        .take(40)
        .collect();
    let p = Progress::default();
    let t = Instant::now();
    let rows = run_chart(&d, &course, &race, &base, &candidates, 7.0, false, &p);
    eprintln!("chart of {} skills in {:?}", candidates.len(), t.elapsed());
    for r in rows.iter().take(8) {
        eprintln!("{:>6.2} L  {}", r.mean, d.skill_name(r.id));
    }
    assert_eq!(rows.len(), candidates.len());
    assert!(rows[0].mean > 0.3, "the best skill gains something");
}
