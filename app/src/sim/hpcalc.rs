//! Moomoolator's HP Calculator (hp-calculator/calculations.ts): a quick closed-form estimate of
//! max HP, HP needed for a full spurt and the minimum Stamina, without running the race.

use super::course::{Course, base_speed};

/// HP coefficient and phase speed coefficients per style: Front, Pace, Late, End, Runaway.
const HP_COEF: [f64; 5] = [0.95, 0.89, 1.0, 0.995, 0.86];
const PHASE_COEF: [[f64; 3]; 5] = [[1.0, 0.98, 0.962], [0.978, 0.991, 0.975], [0.938, 0.998, 0.994], [0.931, 1.0, 1.0], [1.0, 0.98, 0.962]];

#[derive(Clone, Debug, Default)]
pub struct HpEstimate {
    pub max_hp: f64,
    pub phase01_hp: f64,
    pub phase2_hp: f64,
    pub total_needed: f64,
    pub surplus: f64,
    pub full_spurt: bool,
    /// Extra recovery needed in % of max HP when short.
    pub heal_needed: f64,
    pub min_stamina: u32,
    pub downhill_distance: f64,
    pub downhill_mode_rate: f64,
    pub downhill_savings: f64,
    pub adjusted_needed: f64,
    pub adjusted_surplus: f64,
    pub adjusted_full_spurt: bool,
    pub adjusted_min_stamina: u32,
}

fn guts_mod(guts: f64) -> f64 {
    1.0 + 200.0 / (600.0 * guts).sqrt()
}

fn hp_per_second(v: f64, bs: f64, guts: f64, late: bool) -> f64 {
    20.0 * (v - bs + 12.0).powi(2) / 144.0 * if late { guts } else { 1.0 }
}

/// `style`: 0 Front, 1 Pace, 2 Late, 3 End, 4 Runaway.
pub fn max_hp(stamina: f64, distance: f64, style: usize) -> f64 {
    0.8 * HP_COEF[style.min(4)] * stamina + distance
}

fn spurt_speed(bs: f64, speed: f64, style: usize) -> f64 {
    let c = PHASE_COEF[style.min(4)];
    let v2 = bs * c[2] + (500.0 * speed).sqrt() * 0.002;
    (v2 + 0.01 * bs) * 1.05 + (500.0 * speed).sqrt() * 0.002
}

fn hp_needed(distance: f64, style: usize, guts: f64, speed: f64) -> (f64, f64) {
    let bs = base_speed(distance);
    let g = guts_mod(guts);
    let c = PHASE_COEF[style.min(4)];
    let v01 = bs * (c[0] + c[1]) / 2.0;
    let early = hp_per_second(v01, bs, g, false) * (distance * 2.0 / 3.0 / v01);
    let vs = spurt_speed(bs, speed, style);
    let late = hp_per_second(vs, bs, g, true) * (distance / 3.0 / vs);
    (early, late)
}

fn min_stamina(distance: f64, style: usize, heal_pct: f64, needed: f64) -> u32 {
    let (mut lo, mut hi) = (100u32, 2000u32);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if max_hp(f64::from(mid), distance, style) * (1.0 + heal_pct / 100.0) >= needed {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// Expected HP saved on downhills: 60% less drain while in downhill mode, which a wit check
/// starts (0.04% per Wit each second) and ends (20% each second).
fn downhill(course: &Course, wisdom: f64, style: usize, guts: f64, speed: f64) -> (f64, f64, f64) {
    let d = course.distance;
    let slopes: Vec<_> = course.slopes.iter().filter(|s| s.slope < 0.0).collect();
    let dist: f64 = slopes.iter().map(|s| s.length).sum();
    let p = wisdom * 0.0004;
    let rate = p / (p + 0.2);
    if dist == 0.0 {
        return (0.0, rate, 0.0);
    }
    let bs = base_speed(d);
    let g = guts_mod(guts);
    let c = PHASE_COEF[style.min(4)];
    let v01 = bs * (c[0] + c[1]) / 2.0;
    let vs = spurt_speed(bs, speed, style);
    let late_start = d * 2.0 / 3.0;
    let mut hp = 0.0;
    for s in slopes {
        let end = s.start + s.length;
        if s.start < late_start {
            hp += hp_per_second(v01, bs, g, false) * ((end.min(late_start) - s.start) / v01);
        }
        if end > late_start {
            hp += hp_per_second(vs, bs, g, true) * ((end - s.start.max(late_start)) / vs);
        }
    }
    (dist, rate, hp * rate * 0.6)
}

/// The HP Calculator's estimate for one course and build. `style`: 0 Front … 4 Runaway.
pub fn estimate(course: &Course, stamina: f64, guts: f64, wisdom: f64, speed: f64, style: usize, heal_pct: f64) -> HpEstimate {
    let d = course.distance;
    let max = max_hp(stamina, d, style);
    let (early, late) = hp_needed(d, style, guts, speed);
    let total = early + late;
    let effective = max * (1.0 + heal_pct / 100.0);
    let surplus = effective - total;
    let (dist, rate, savings) = downhill(course, wisdom, style, guts, speed);
    let adjusted = total - savings;
    HpEstimate {
        max_hp: max,
        phase01_hp: early,
        phase2_hp: late,
        total_needed: total,
        surplus,
        full_spurt: surplus >= 0.0,
        heal_needed: if surplus >= 0.0 { 0.0 } else { ((total - max) / max * 1000.0).ceil() / 10.0 },
        min_stamina: min_stamina(d, style, heal_pct, total),
        downhill_distance: dist,
        downhill_mode_rate: rate,
        downhill_savings: savings,
        adjusted_needed: adjusted,
        adjusted_surplus: effective - adjusted,
        adjusted_full_spurt: effective >= adjusted,
        adjusted_min_stamina: min_stamina(d, style, heal_pct, adjusted),
    }
}
