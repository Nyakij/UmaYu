//! Course geometry from Moomoolator's `course_data.json` (CourseData.ts).

use serde::Deserialize;

use super::region::Region;

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Corner {
    pub start: f64,
    pub length: f64,
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Straight {
    pub start: f64,
    pub end: f64,
    #[serde(rename = "frontType")]
    pub front_type: i32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Slope {
    pub start: f64,
    pub length: f64,
    pub slope: f64,
}

/// One course as stored in `course_data.json`.
#[derive(Deserialize, Clone, Debug)]
pub struct RawCourse {
    #[serde(rename = "raceTrackId")]
    pub race_track_id: u32,
    pub distance: f64,
    #[serde(rename = "distanceType")]
    pub distance_type: i32,
    pub surface: i32,
    pub turn: i32,
    /// Layout: 1 normal, 2 inner, 3 outer, 4 outer→inner.
    #[serde(default)]
    pub course: i32,
    #[serde(rename = "courseSetStatus", default)]
    pub course_set_status: Vec<i32>,
    #[serde(default)]
    pub corners: Vec<Corner>,
    #[serde(default)]
    pub straights: Vec<Straight>,
    #[serde(default)]
    pub slopes: Vec<Slope>,
    #[serde(rename = "laneMax", default)]
    pub lane_max: f64,
}

/// A course ready for simulation, with the lane constants CourseHelpers.getCourse derives.
#[derive(Clone, Debug)]
pub struct Course {
    pub race_track_id: u32,
    pub distance: f64,
    pub distance_type: i32,
    pub surface: i32,
    pub turn: i32,
    pub layout: i32,
    pub course_set_status: Vec<i32>,
    pub corners: Vec<Corner>,
    pub straights: Vec<Straight>,
    pub slopes: Vec<Slope>,
    pub horse_lane: f64,
    pub lane_change_acceleration_per_frame: f64,
    pub max_lane_distance: f64,
    pub move_lane_point: f64,
}

impl Course {
    pub fn from_raw(raw: &RawCourse) -> Course {
        let mut slopes = raw.slopes.clone();
        // course_data.json sometimes sorts slopes by type first; the solver needs them by start.
        slopes.sort_by(|a, b| a.start.total_cmp(&b.start));
        let course_width = 11.25;
        let lane_change_acceleration = 0.02 * 1.5;
        Course {
            race_track_id: raw.race_track_id,
            distance: raw.distance,
            distance_type: raw.distance_type,
            surface: raw.surface,
            turn: raw.turn,
            layout: raw.course,
            course_set_status: raw.course_set_status.clone(),
            corners: raw.corners.clone(),
            straights: raw.straights.clone(),
            slopes,
            horse_lane: course_width / 18.0,
            lane_change_acceleration_per_frame: lane_change_acceleration / 15.0,
            max_lane_distance: course_width * raw.lane_max / 10000.0,
            move_lane_point: raw.corners.first().map_or(30.0, |c| c.start),
        }
    }

    pub fn last_straight(&self) -> Region {
        self.straights
            .last()
            .map_or(Region::new(self.distance, self.distance), |s| {
                Region::new(s.start, s.end)
            })
    }

    pub fn corner_regions(&self) -> Vec<Region> {
        self.corners
            .iter()
            .map(|c| Region::new(c.start, c.start + c.length))
            .collect()
    }

    /// 1 + the course's stat bonus: +5% per 300 of each listed stat, averaged (capped at 901).
    pub fn speed_modifier(&self, stats: [f64; 5]) -> f64 {
        let value = |stat: i32| match stat {
            1..=5 => stats[(stat - 1) as usize].min(901.0),
            _ => 0.0,
        };
        let sum: f64 = self
            .course_set_status
            .iter()
            .map(|s| (1.0 + (value(*s) / 300.01).floor()) * 0.05)
            .sum();
        1.0 + sum / (self.course_set_status.len().max(1) as f64)
    }
}

pub fn phase_start(distance: f64, phase: i32) -> f64 {
    match phase {
        0 => 0.0,
        1 => distance * 1.0 / 6.0,
        2 => distance * 2.0 / 3.0,
        _ => distance * 5.0 / 6.0,
    }
}

pub fn phase_end(distance: f64, phase: i32) -> f64 {
    match phase {
        0 => distance * 1.0 / 6.0,
        1 => distance * 2.0 / 3.0,
        2 => distance * 5.0 / 6.0,
        _ => distance,
    }
}

pub fn base_speed(distance: f64) -> f64 {
    20.0 - (distance - 2000.0) / 1000.0
}
