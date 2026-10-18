# Changelog

What changed in each release. Releases are built from the tagged source by GitHub Actions; download them from [Releases](https://github.com/brickc4/umaLord/releases).

## [0.1.1] - 2026-10-18

### Added

- Every push is built and tested by GitHub Actions, and releases are built from the tagged source, so the download always matches the code.
- A README with screenshots, this changelog, and forms for bug reports and ideas.

### Changed

- A new install starts with an empty account: the app writes the blank account files on first run, and the release zip never contains anyone's data.

### Fixed

- Race sim: the "Quick stamina estimate" button showed a box instead of its icon.
- Card picks: the background ranking could look busy for a moment after it had ranked every slot.

## [0.1.0] - 2026-10-18

First public release.

- **Collection:** your umas and support cards, with effects at your limit break and level, the skills they teach, and their events.
- **Career → Plan:** goals, trainees, decks and skills to buy. Picks start from the community PvP meta sheet for the next Champions Meeting, then simulated careers rank the rest; Auto-pick buys skills within your simulated SP budget.
- **Career → Race sim:** a Rust port of Moomoolator's race engine. Compare two builds, chart skills, check stamina.
- **Career → Results**, **Notes**, and **Ask** (questions about your roster in plain English).
- **Setup:** load a profile file, or click what you own. ⚙ → Save my data, Load a profile, Start over.
- **Themes:** Seiun Sky, Sakura, Sunset and Starry night, in dark or light.

[0.1.1]: https://github.com/brickc4/umaLord/releases/tag/v0.1.1
[0.1.0]: https://github.com/brickc4/umaLord/releases/tag/v0.1.0
