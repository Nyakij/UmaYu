//! Native race simulator: a Rust port of Moomoolator (https://umalator.app/), TheCing's fork of
//! alpha123's Umalator and kachi-dev's VFalator, built on uma-skill-tools (alpha123 and pecan).
//! Source: https://github.com/TheCing/uma-tools (engine v2, checked 2026-10-17).
//!
//! The original is licensed GPL-3.0-or-later, and so is this port: see LICENSE in
//! the umaLord folder.
//! Game data (skills, courses, outfits) comes from Moomoolator's Global files, which
//! tools/update_knowledge.py saves in knowledge/data/sim/.
//!
//! Like Moomoolator, it simulates one uma at a time against a virtual pacemaker: position keep
//! is approximated, order conditions are assumed met, and some modelled conditions use
//! probability distributions instead of real opponents.

pub mod builder;
pub mod conditions;
pub mod course;
pub mod data;
pub mod hp;
pub mod hpcalc;
pub mod params;
pub mod region;
pub mod rng;
pub mod run;
pub mod solver;

#[cfg(test)]
mod tests;
