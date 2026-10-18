# Changelog

What changed in each release. Releases are built from the tagged source by GitHub Actions; download them from [Releases](https://github.com/brickc4/umaLord/releases).

## [Unreleased]

### Added

- **Home is now the hub:** a greeting with quick actions; the next Champions Meeting with its course drawn (phases, corners, slopes), what the community meta sheet recommends, your best umas for it and how far your plan has got ("Continue plan" opens it); a timeline of the next three weeks from the game's news; and widgets: where you left off, your stable, cards by type, who can run where, an uma of the day, a trainer card with badges, your best and missing cards for the next CM, and a game term a day.
- **Collection redesigned:** filters as chips in a sidebar that folds away, quick views (built in, and your own), the filters that are on listed above the results, and sorting beside the results that matches the table headers. Umas filter by an aptitude grid (A or better, or S, at any surface, distance or style). Cards filter by limit break, by a skill they teach, by an effect's minimum, and by scenario links; hover a card to put it in your plan's deck or to compare two side by side. Tables show aptitudes as coloured grades under Surface · Distance · Style, and effects with units and a heat tint; they can hide columns or copy as CSV. Pictures come in three sizes, with group headers that stay on screen while you scroll.
- **Skills:** "Ones I can get | All skills", a course filter that keeps only skills that can trigger at a Champions Meeting or on any course (checked by the race simulator), owners shown as small pictures, durations at each race distance, the white → gold upgrade path, and "Test on a course" in the Race sim.
- **Add cards** shows every card's picture in a taller window; click a diamond on a card to save it at that limit break, and missing pictures download by themselves.
- **A faster, friendlier setup:** a step bar that stays on screen, a "Make it yours" step for your name, mode and theme, and quicker grids: set "New picks start as" (stars and potential, or a limit break) and every click adds an item that way; hover an added uma to set her stars and potential on the picture, or a card to click its limit-break diamonds; Ctrl- and Shift-click to change several at once; "Add all" per card group; keys for everything; Ctrl+Z to undo; and "Paste a list" for typing or pasting what you own. Outfits show their name under the uma, cards are grouped by rarity and type, and setup ends with a summary and the next Champions Meeting.
- **Live Champions Meeting schedule:** on launch the app reads the game's official news (umamusume.com) and Umalator's latest presets in the background. Home shows whether the next CM is announced, with countdowns to league selection, Round 1 and the end in your time zone, and the skills it turns off. The planner and Race sim chips read "Capricorn Cup · Oct 31". The status bar says how fresh the schedule is, and Settings → Data and updates can turn it off or refresh it. Without internet it uses the last fetch, or the bundled data.
- **Skills a CM turns off** are never auto-picked for it, are struck through in the plan's skill list, and are taken out of builds when a CM or a plan for it is loaded into the Race sim.
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

- The next Champions Meeting's date was Umalator's estimate (Capricorn Cup on Oct 25); it's now the official Round 1 (Oct 31), and plans saved with the old date still find it.
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
