# Changelog

What changed in each release. Releases are built from the tagged source by GitHub Actions; download them from [Releases](https://github.com/brickc4/umaLord/releases).

## [Unreleased]

### Added

- **Settings** is now a proper window (⚙ or Ctrl+,) with five sections: Profile (rename the account, save, load, start over), Appearance (mode, themes with live previews, Seiun Sky, size and spacing), Data and updates (how old the game data and meta sheet are, and how to refresh them), AI assistant (the Ask tab's backend, model and key, moved out of the Ask tab), and About. Help has its own ? button.
- **Two new themes:** Track day (trophy gold with a racetrack's rails) and Ink (high contrast black and white).
- **Size and spacing:** make everything 90–125% as big, and switch to compact spacing.
- **System mode:** the app can follow Windows' dark or light setting.
- **Keyboard shortcuts:** Ctrl+1 to Ctrl+4 switch tabs, Ctrl+F jumps to the search box, Ctrl+S saves the plan, F5 reloads, Esc closes things, and ? opens Help. Help lists them.
- Messages like "Saved" now pop up at the bottom of the window and fade away, instead of pushing the page around. A saved profile's message has a "Show" button that opens its folder.
- `tools/screenshots.ps1` takes screenshots of every page in each theme and window size, for checking the look.

### Changed

- The Notes tab is gone from the top bar; open the notes and data files from ⚙ → Advanced → Browse notes and data files. Help links to it too.
- **Chibi Seiun Sky no longer covers your data.** By default she now stands beside Home's welcome and on empty pages; in ⚙ you can also show her beside every page title, or hide her. The background switch now only turns the sky and decorations on and off.
- Light mode is easier to read: green, orange, gold, pink, grade letters and card-type names are darker on white, and every status colour passes the WCAG AA contrast check (a test keeps it that way).

### Fixed

- The Claude API key badge said a key was set even when none was.
- Save my data no longer opens Explorer by itself; its message has a Show button instead.
- Showing a profile with `UMALORD_PROFILE` (for screenshots) no longer overwrites the Ask tab's roster digest with that profile's roster.

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
