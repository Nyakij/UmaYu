# umaLord v0.2 overhaul: a step-by-step guide for the AI doing the work

This guide is for an AI coding agent (Claude Code or similar) working in this repository. It covers what the owner asked for, what is wrong today (with evidence), and the order to build it in. Each step has a definition of done.

Written 2026-10-18 from a full read of `app/src/`, live screenshots of every page (dark and light), a live run of the race sim and skill auto-pick, and research into data sources. Nothing here has been built yet.

---

## Start here

### Read these first, completely

1. **`CLAUDE.md`.** It is the contract for this repo: where data lives, how tabs are built, the screenshot env vars, the test commands, the public-repo rules and the release flow. Everything below assumes you have read it.
2. **`AGENTS.md`.** These are the OpenCode answering rules, which must stay in sync with CLAUDE.md's "Rules for answering".
3. **`app/src/main.rs`, `theme.rs` and `ui_util.rs`.** These hold the shell, the palette and the shared widgets that every page uses.

### Ground rules

- **egui is pinned at 0.37 (eframe 0.37, egui_extras 0.37).** Use that version's API (`egui::Panel::left`, `egui::Modal`, `ui.close()`, `Button::selectable`, and so on). Before adding a crate, check that it has a release built against egui 0.37. If none exists, paint the widget yourself, as the app already does for its charts.
- **Toolchain.** This machine has only `stable-x86_64-pc-windows-gnu`, and `app/.cargo/config.toml` explains the workarounds. Don't add crates that need a C compiler or native TLS. Network access goes through Windows' `curl.exe`; see `fetch_card_art` in `tab_cards.rs` and `ask::hidden()`.
- **Never break file formats.**
  - `account/*.md` are read by column header.
  - Plan files end in an `<!-- umalord-plan {json} -->` block. Every new `Plan` field needs `#[serde(default)]` so old plans still load.
  - `*.umalord.json` profiles.
  - `settings.json` keys.
- **The repo is public.** Never commit `account/`, `plans/`, `results/`, `profiles/` or `insights.md`, and never commit personal names or paths. Check `git diff --cached` before every commit.
- **Words for newcomers.** Write labels for someone new to the game: "Maxed (MLB)", not "MLB". Each term gets a hover explanation.
- **Colours come from `theme::pal(ui)`.** Never hard-code colours. Green (`GOOD`) is only for good news.
- **Label every claim** as data, simulation, community meta or opinion. This applies to UI copy and to the Ask agent.
- **Work in phases, on a branch, with one commit per step.** After each step, finish all of the following:
  1. `cargo build --release` and `cargo test --release` from `app/`. Also run `cargo test --release frame_times -- --ignored --nocapture` when you touch a page; frames should stay at about 1 ms.
  2. Screenshots of every page you touched, in dark and light, at 1400×900 and at the minimum size of 960×600. Use the `UMALORD_*` env vars and `Start-Process -Wait`. Look at them, find what's wrong, and fix it before moving on.
  3. Update CLAUDE.md, `docs/guide.txt` and `CHANGELOG.md` (under an "Unreleased" heading) when behaviour changes.
- **Ask before committing to `main`, pushing, tagging or releasing.** Those steps are the owner's call.

### Make screenshots one command (do this first)

Add `tools/screenshots.ps1`. It runs `app/target/release/umalord.exe` once per page × theme × size and saves PNGs into a folder you pass in, which must never be inside the repo. The matrix:

- **Pages:** setup (each step), home, umas (grid and table), uma detail, cards (grid and table), card detail, skills, skill detail, planner (empty and demo), planner scrolled to skills, sim (empty and after a run), results, ask, settings, help.
- **Themes:** dark and light, plus one other look.
- **Sizes:** 1400×900 and 960×600.

For pages that need a roster, set `UMALORD_PROFILE` to the friend's profile (see CLAUDE.md), so the owner's account is never touched. Add new env vars as you add pages; for example, `UMALORD_SETTINGS_PAGE=appearance` once Settings becomes a page.

---

## Problems found in the audit

Fix these as part of the phases below. Each one was seen in a live screenshot or a live run.

### Global

- **G1. The mascot covers content.** On the Umas table, the Cards and Umas grids, the Skills table and the Planner, the chibi Seiun Sky is drawn over data at alpha 120–150 (`App::paint_background`, bottom right). Grades and names under it are unreadable.
- **G2. Status colours fail contrast in light mode.** `GOOD` (34,197,94) and `WARN` (245,158,11) are about 2.3:1 and 2.1:1 on white (WCAG AA needs 4.5:1).
  - Make them palette fields that change per mode. In light mode use about (21,128,61) for good and (180,83,9) for warn.
  - Also check `p.weak` on `p.surface`, gold on white, and the type colours as text in light mode.
- **G3. Controls don't match.** Filter sidebars mix toggle buttons (stars), selectable labels (potential), combo boxes (aptitudes) and radio buttons (sort). Pick one control per kind of choice (Phase 0.2).
- **G4. No type scale.** Font sizes are scattered literals (11, 11.5, 12, 12.5, 13, 13.5, 14, 14.5, 15, 15.5, 16, 16.5, 17, 19, 20, 21, 22, 24, 26, 28). Use named sizes (Phase 0.1).
- **G5. Status messages are inline green or orange lines** that push the layout around (`planner.status`, `cards.add.status`, `sim.status`, `onboarding.status`). Replace them with toasts (Phase 0.2).
- **G6. No keyboard shortcuts** anywhere. Add them as listed in Phase 0.4.
- **G7. The 960×600 minimum size isn't usable on Collection.** A 240 px filter panel and a 440 px detail panel leave too little room. Panels must collapse.

### Setup (onboarding)

- **S1. The stars, potential and LB editor sits at the top of the page.** You click a tile five rows down, the editor appears above the grid, off screen. Each item takes two clicks plus scrolling.
- **S2. Outfits of the same uma look identical.** Two "Agnes Digital" tiles side by side can't be told apart because the outfit title isn't shown.
- **S3. The name box under "Set up from scratch" is disconnected**, and you can skip it without noticing. The welcome card also leaves half the window empty.
- **S4. No theme or dark/light choice in setup**, and no live preview.
- **S5. No bulk actions:** no default ("brush") value, no multi-select, no keyboard, no paste-a-list.
- **S6. The Next and Back buttons scroll away**, and the step header isn't sticky.

### Home

- **H1. "Your strongest cards" is a wall of text chips.** The owner dislikes it.
- **H2. Five big shortcut tiles take a whole row** and say little.
- **H3. The CM date is wrong.** It shows Capricorn Cup on **2026-10-25 · "Not announced"**, but the official site already announced it: league selection Oct 27, Round 1 from Oct 31 22:00 UTC, Chukyo turf 1200 m left, Winter, Sunny, Firm. The bundled presets are Umalator's guesses. Sagittarius Cup was listed for Oct 4; it actually ran Oct 10–16. See Phase 3.
- **H4. Nothing on Home is live or personal** beyond counts: no continue-where-you-left-off, no event timeline, no plan progress.

### Collection

- **C1. Umas table.**
  - Header baselines don't line up ("Outfit" sits higher than "Uma").
  - The sorted header looks like a button.
  - The stars widget is top-aligned while text is centred.
  - Aptitude columns can't be sorted.
  - An empty "Notes" column takes the remaining width.
- **C2. Cards table.**
  - Headers are cut off ("Frie…", "Hin…").
  - Zero shows as "·".
  - LB diamonds are top-aligned.
  - Long titles are clipped with no tooltip.
  - Thumbnails are 19×25 px.
  - Numbers are left-aligned.
- **C3. Sort lives in the sidebar as radio buttons**, separate from the clickable table headers, and has no direction (ascending or descending).
- **C4. Skills.**
  - The "Skills 446" chip is ambiguous (446 available of 954).
  - The "You can get it from" column is long green text with emoji and gets clipped.
  - "Only for" wraps.
  - The effect text is clipped.
- **C5. The Add cards dialog** shows only about six rows. The "owned LB4 Lv50" badge floats mid-row. The LB picker is bare numbers. Some cards show "?" for missing art.

### Career

- **K1. Skill auto-pick isn't optimal.** Measured on the CM 21 demo plan (see Phase 7):
  - It adds up independent skill-chart gains as if they stacked. A set it picked raced only **+3.5 L** in 1000 runs, while the single gains of its picks add up to much more.
  - The meta boost buys Focus (+0.03 L for 133 SP) and two 350 SP golds while leaving better ★ skills unbought.
  - The chart turns wit checks and rushing off, which overstates gains.
  - It ignores the CM's disabled-skill rules.
- **K2. The Race sim results are thin.**
  - The histogram has no axes, no mean or median markers, and colours in one hue.
  - The speed/HP chart has no x-axis ticks, no legend, and unlabelled skill bars.
  - Corners and straights aren't shown.
  - There is no "where were the lengths won" view.
  - The left panel is crowded.
  - CM chips read "? Capricorn Cup 26-10".
- **K3. The Planner doesn't warn about a bad aptitude.** The demo picked a trainee with **Sprint E** for a Sprint CM with no warning.

### Ask

- **A1. The assistant is read-only** and says "You can't run race simulations yourself" (`ask.rs`, `system_prompt`). The owner wants it to build careers and run simulations.

### Notes

- **N1. The owner wants the Notes tab hidden.**

---

## Phase 0: design foundation (do first; everything else uses it)

### 0.1 Tokens in `theme.rs`

1. **Type scale.** Add `pub const` sizes: `XS 11.5, SM 12.5, BODY 14, MD 15.5, LG 17, XL 22, XXL 28`, plus a `display` size for hero numbers. Replace literals as you touch each file. There's no need to sweep untouched code in one go.
2. **Spacing.** Add `pub const` gaps (4, 8, 12, 16, 24) and card radius 12.
3. **Status colours per mode.** Move `GOOD`, `WARN` and a new `BAD` into `Palette` with light and dark variants (G2). Keep the `ui_util::GOOD` and `WARN` constants as deprecated aliases until every call site reads `pal(ui).good` and `pal(ui).warn`.
4. **System theme.** Add a System mode that follows Windows (egui `ThemePreference::System`). Store `theme: "system" | "dark" | "light"` in settings.json; `"dark"` stays the default for existing users.
5. **UI scale.** Store `ui_scale` (0.9–1.25) in settings.json and apply it with `ctx.set_zoom_factor`.
6. **Optional new looks.** Look variants must respect "no green theme", and each needs `tint()` values for both modes plus a background decoration. Ideas: a "Track day" theme in turf-brown or gold, and a high-contrast "Ink" theme.

### 0.2 Shared widgets in `ui_util.rs`

Write each widget once, then use it everywhere.

- **`chip(ui, on, label, count) -> Response`.** A pill toggle with an optional count badge, used for every multi-select filter.
- **`chip_row` and `single_chips`.** A multi-select row, and a single-select row that replaces `selectable_value` rows.
- **`sort_control(ui, &mut field, &mut desc, options)`.** A dropdown for the field plus a ▲/▼ button.
- **`FilterSection`.** A collapsible sidebar section: title, ⓘ hover, an active-count badge and a per-section "Clear". It remembers whether it's open via `ui.data`.
- **`active_filter_bar`.** A row of removable chips for the active filters ("Long ≥ A ✕", "5★ ✕"), shown above results.
- **`DataTable`.** A thin wrapper over `egui_extras::TableBuilder`:
  - one header style: semibold 12.5, weak colour, vertically centred
  - sortable headers with ▲/▼ and a hover hint
  - per-column alignment (numbers right, grades centre)
  - row height 34 with every cell vertically centred (wrap custom widgets in `ui.centered_and_justified` or allocate with `Align::Center`)
  - hover row highlight and selected-row accent bar
  - tooltips for abbreviated headers
  - a "Columns" menu to show or hide columns, saved in settings.json
  - "Copy as CSV"
- **`toast(ctx, text, kind)`.** Toasts stack bottom-centre, last about 4 s, and can carry an optional action button ("Undo", "Open"). They replace the inline status lines (G5) and are drawn in `App::frame` after the pages.
- **`stepper(ui, steps, at)`.** Numbered circles joined by a line, with ✔ for done steps, for Setup.
- **Empty-state and skeleton widgets.** A skeleton is a grey placeholder bar shown while background work runs.
- **Charts module `app/src/charts.rs`.** Painter-based charts shared by Home, Race sim and the Planner. Each chart has:
  - axes with ticks, a legend, and a crosshair tooltip
  - mean and median markers, and shaded bands
  - **light/dark-aware colours that follow the `dataviz` skill's rules** (one hue per series, consistent across views)

### 0.3 The mascot (G1)

Content must never sit under art.

- Draw the chibi only where nothing else is: on Home's hero area, in empty states, and at the right of page headers at a smaller size.
- Elsewhere, draw only the gradient and decorations, which are subtle.
- Give it a three-way Settings option, "Mascot: Everywhere (corner) / Home and empty pages / Off", and make the default "Home and empty pages".
- If you keep "corner", reserve the space: add bottom padding to scroll areas equal to the art's height, so the last row can scroll above it.

### 0.4 Keyboard shortcuts (G6)

- `Ctrl+1…4` switch tabs.
- `Ctrl+F` focuses the page's search.
- `Ctrl+S` saves the plan.
- `F5` reloads.
- `Esc` closes the detail panel or modal.
- `?` opens Help.

List them in Help.

**Done when:** the tokens, widgets and toasts exist with unit tests where logic exists (sorting, the CSV writer), the mascot no longer covers content in any screenshot, and the light-mode status colours pass 4.5:1. Write a tiny contrast test that computes WCAG contrast for `good`, `warn` and `weak` against `surface` in both modes.

---

## Phase 1: Notes tab hidden (small; do early)

1. Remove `Tab::Notes` from `TABS` in `main.rs`, keep the enum variant and `notes_tab`.
2. Give Settings → Data an "Advanced: browse notes and data files" button that opens the notes page as a modal or full page with a Back button. Help no longer lists Notes.
3. Keep `UMALORD_TAB=notes` working for screenshots.
4. Update `Tab::info`, the `paint_background` match, `docs/guide.txt`, the README table and CLAUDE.md ("Tabs" line).
5. `ctrl+number` shortcuts and `frame_times` must not reference Notes.

**Done when:** the top bar shows Home, Collection, Career and Ask, all tests pass, and the notes are still reachable from Settings.

---

## Phase 2: a real Settings page (replaces the ⚙ dropdown)

The owner wants themes, light and dark "in the settings". These exist today in a small popup (`App::settings_menu`), which is easy to miss.

### 2.1 Layout

Build a large `egui::Modal`, about 860×620, or a full page with a left nav. Its sections:

1. **Profile.** Trainer name (editable; writes `- Profile:` in `account/umas.md` with `data::set_meta`), server (read-only "Global (English)"), and where the data folder is (Open folder). Then Save my data, Load a profile, Set up my collection, and Start over, the last in a red danger zone with its confirmation modal.
2. **Appearance.**
   - Mode: System, Light or Dark, as large segmented buttons.
   - Theme: cards with a **live mini-preview**, a painted thumbnail of a page (top bar, a card, an accent button, the sky gradient) for each `Look` in the current mode. Click to apply.
   - Background art on or off, and the mascot option (Phase 0.3).
   - UI scale slider.
   - Density: Comfortable or Compact (spacing tokens × 0.8).
3. **Data and updates.**
   - Game data date (from `kb.built`).
   - "Fetch the latest Champions Meeting schedule on launch" (default on), the last fetch time and status, and Refresh now (Phase 3).
   - Meta sheet date, with Refresh meta (Phase 3).
   - Advanced: browse notes (Phase 1).
4. **AI assistant.**
   - Backend, model, API key (moved here from the Ask tab's side panel; keep a short summary in Ask).
   - Agent permissions (Phase 9): "Let the assistant change plans", "…run simulations", "…edit my roster (asks every time)".
5. **About.** Version (`env!("CARGO_PKG_VERSION")`), credits from the README, license, links to Releases and Issues, and the keyboard shortcuts.

### 2.2 Wiring

- The ⚙ button opens Settings, and a separate "?" button opens Help.
- Every control saves straight away through `settings::set`. There is no Save button.
- `UMALORD_SETTINGS=1` opens it and `UMALORD_SETTINGS_PAGE=<section>` picks a section.

**Done when:** every option from the old menu exists here, screenshots of each section look right in both modes, and the old `settings_menu` is deleted.

---

## Phase 3: live Champions Meeting data on launch (fixes H3)

### 3.1 Sources, in priority order

1. **The official Global news API.** Verified 2026-10-18 with plain `curl.exe`:

   ```
   POST https://umamusume.com/api/ajax/pr_info_index?format=json
   Content-Type: application/json
   {"announce_label":0,"limit":50,"offset":0}
   ```

   - It returns `{"response_code":1,"information_list":[{announce_id,title,message,post_at,update_at,announce_label,image,…}]}`.
   - `message` is HTML. Page backwards with `offset` if the newest 50 items have no CM.
   - CM announcements come in three shapes. All three are in the live feed now.
     - **"The race event Champions Meeting: Capricorn Cup is coming in late October!"** It contains:
       - `League Selection: From 10:00 p.m., Oct 27, 2026 (UTC)`
       - `Round 1: From 10:00 p.m., Oct 31, 2026 (UTC)`
       - a **Race Conditions** line: `Chukyo / Turf / 1,200m (Sprint) / Left-Handed / Winter / Sunny / Firm`. Sagittarius had `… / Right-Handed / Inner / Winter / Cloudy / Good`.
       - **"The following skills will not activate in this … Cup:"** followed by bullets such as `Night Races ◎`, `Sharp Turns ○` and `Collaborative Graded Races ×`.
     - **"The league selection period for the Champions Meeting: X Cup has begun!"** It has the `Event Availability Period` and `League Selection Period` ranges.
     - **"The race event Champions Meeting: X Cup is here!"** It has the same ranges.
   - **Legend Race** announcements use the same "Availability Period" and "Race Conditions" format; parse them too, for Home's timeline. No League of Heroes announcement was in the feed on 2026-10-18; expect a similar shape, but check one before relying on it.
2. **Moomoolator presets.** Port `presets()` and the `PRESET` regex from `tools/update_knowledge.py` to Rust: fetch `https://umalator.app/`, find `/assets/index-*.js`, and run the regex. Presets give the exact Umalator `course` id (inner or outer layout) and a `confirmed` flag, and they cover CMs that aren't announced yet.
3. **The bundled `knowledge/data/umalator.json` and `cm_schedule.json`,** as the offline fallback.

### 3.2 Implementation

- **New module `app/src/live.rs`.**
  - `spawn_refresh(root, ctx) -> Receiver<LiveUpdate>` runs in a background thread with `curl.exe -sSfL --max-time 15` and a User-Agent of `umaLord/<version>`.
  - It is started in `App::new` when the setting is on and the cache is older than 6 h. The UI never waits for it.
  - `App::frame` polls it the way `planner_poll` does.
- **Cache** at `%APPDATA%\umaLord\cache\events.json`, next to settings.json. It is per user and never committed. It holds `fetched_at`, the raw source tags and the parsed events.
- **Parsing (`live::parse_announcement`).**
  - Regexes, or simple string scans, for the cup name, the "From …" times, the ranges with an en dash (`10:00 p.m., Oct 10–9:59 p.m., Oct 16, 2026 (UTC)`), the conditions line, and the disabled-skill bullets.
  - Strip HTML tags and decode `&nbsp;`, ` ` and entities.
  - Store times as UTC timestamps.
  - The same cup can appear in several announcements (coming, selection, here); merge them by cup name and year.
- **The event model.** Extend `EventPreset` in `knowledge.rs` (all fields `Option` or `#[serde(default)]`):
  - `league_selection`, `round1`, `ends` (UTC)
  - `disabled_skills: Vec<u32>`: resolve names plus `◎ ○ ×` against `kb.skills` names; log anything unmatched
  - `source: Official | Umalator | Bundled`
  - `announced: bool`, which replaces "confirmed" for display
- **Merging.** For dates, conditions and disabled skills: official > Umalator > bundled. For the exact course id: Umalator. If there is no Umalator preset, map track + surface + distance (+ Inner/Outer) to `knowledge/data/umalator.json` → `courses`. Keep `preset.date` as the **Round 1 start date**, so that `upcoming_cms` sorting still works.
- **Matching old plans.** `Plan.cm_date` holds the old guessed dates (for example 2026-10-25). `plan_cm()` must match by **cup name + course id within ±21 days**, not exact date. Add a test: a plan saved with `cm_date: "2026-10-25"` and course 10xxx still links to the live Capricorn Cup dated 2026-10-31.
- **Where it shows.**
  - Home's CM card: countdowns to league selection, Round 1 and the end, in the user's local time with UTC on hover, plus an "announced" or "estimated" badge with its source.
  - Planner CM chips, and Race sim chips: "Capricorn Cup · Oct 31", not "26-10".
  - Disabled skills: shown on the CM card, struck through in skill lists with "won't activate in this CM", never auto-picked (Phase 7), and dropped with a note when loading a CM into the sim.
  - Status bar: "CM schedule: updated 3 min ago (umamusume.com)", or "offline — using data from 2026-10-17".
- **Meta sheet refresh (optional).** Port `write_pvp_meta`'s CSV reading to Rust behind a "Refresh meta" button, with the same gviz CSV URLs. If this is too large, show the meta file's date and explain that `tools/update_knowledge.py --only meta` refreshes it.

### 3.3 Tests

- Save sample responses as fixtures: `app/tests/fixtures/news_sample.json`, copied from the live API with the three CM shapes plus one Legend Race. Test the parse of each shape, the date ranges, the conditions and the disabled-skill resolution (all nine names → ids).
- Test the merge: official overrides an Umalator guess, and an Umalator course id fills a missing layout.
- An `#[ignore]` test hits the live API.

**Done when:** with the network on, Home shows Capricorn Cup with Oct 27 league selection and Round 1 on Oct 31, marked "Announced". With the network off, it shows the cached or bundled data with an "offline" note. Disabled skills appear.

---

## Phase 4: onboarding rebuilt (fast, and nice to look at)

### 4.1 The flow

Show it with the new `stepper`; the header and the Back/Next footer stay sticky (S6).

1. **Welcome.** A full-width hero: the theme's sky gradient, the mascot, "Welcome to umaLord" and one sentence on what it does. Two large choice cards: **Load a profile** and **Set up from scratch**.
2. **Make it yours.** Comes right after Welcome on both paths.
   - **"What should we call you?"** A large text field. The name is required; Skip gives "Trainer". It writes `- Profile:`. (On the import path, prefill it from the profile and let the user change it.)
   - **Mode** (System, Light, Dark) and **theme**: the same live-preview cards as Settings, applied as you click (S4).
   - Background and mascot toggles.
3. **Your umas.** Below.
4. **Your support cards.** Below.
5. **All set.**
   - A summary with count-up numbers: umas by stars, cards by rarity and LB, MLB SSRs per type.
   - One friendly line computed from the roster ("Your strongest type is Speed: 9 maxed SSRs").
   - The next CM, with a "Plan for it" button.
   - Big tiles: Plan a career, Explore your collection, Open Home.

### 4.2 Making the uma and card steps fast (S1, S2, S5)

- **Edit on the tile.** No editor at the top.
  - **Cards:** hovering an owned card shows four clickable diamonds on the tile. Clicking diamond *k* sets LB *k*; clicking the lit first diamond again sets LB0. An unowned card shows "＋".
  - **Umas:** hovering shows a five-star strip (click star *n* for *n*★) and P1–P5 pips.
  - Right-click opens a menu: Remove, Set level…, and Open details.
  - The current top editor can remain as an "Edit selected" panel, docked to the right as a side panel, so it never scrolls away.
- **A "New picks start as" brush** in the toolbar.
  - Umas: `[3★ ▾] [P1 ▾]`. Cards: `[LB0 ◇◇◇◇ … MLB ◆◆◆◆]`.
  - Clicking an unowned tile adds it with the brush values.
  - The intended flow: set the brush to MLB, click every maxed card, set it to LB0, click the rest. One click per item.
- **Multi-select.** Shift-click selects a range and Ctrl-click toggles. A bulk bar ("12 selected — Set LB ▾ · Set stars ▾ · Remove") appears above the grid.
- **Keyboard.**
  - Typing a letter focuses search; Enter adds the first match with the brush values.
  - Arrow keys move a focus ring.
  - Space toggles owned.
  - `0–4` sets LB on cards. On umas, `1–5` sets stars and `Shift+1–5` sets potential.
  - `Del` removes. `Ctrl+Z` undoes the last change; keep a small undo stack of the row before each upsert.
- **Grouping and labels.**
  - Umas are grouped by character, with outfits side by side and the outfit title under the name in small text (S2).
  - Cards are grouped by rarity, then type, with section headers showing "12 of 40 owned" and a per-section "Add all with brush".
  - A "Newest first" toggle: new players own recent banners.
  - A sticky progress line: "57 umas · 100 cards added".
- **Paste a list.** A "Paste a list" button opens a modal with a text area: one item per line, for example `Kitasan Black MLB`, `Super Creek SSR LB3`, `Oguri Cap Starlight Beat 5* P5`.
  - A fuzzy matcher (lower-case word overlap plus rarity and type hints) shows a preview table with a confidence column and a dropdown for fixing each match. Confirm adds them all.
  - Unit-test the parser on about 20 lines, including typos and ambiguous names.
- **Screenshot import (stretch; build it last).** Drop screenshots of the in-game Support Card list or Umamusume list. Then:
  1. Find the grid cells.
  2. Match each cell against `app/assets/cards/*.webp` or `umas/*.webp` by downscaled normalised cross-correlation. The `image` crate is enough.
  3. Read the LB diamonds by sampling the cyan (`LB_ON`) pixels, and the stars by sampling gold.
  4. Always end on the same review table as Paste a list.

  `tools/sheet.ps1` and `crop.ps1` show how the art lines up. Ship it only if it is at least 95% accurate on a test set of real screenshots that you keep out of the repo.
- **Speed of writes.** Each click upserts `account/*.md` (`data::upsert_uma` and `upsert_card`). Keep that, but debounce `write_digest()` and the `rank_cache` invalidation to when the user leaves the step.

### 4.3 Making it look nice

- Constrain the page to a 1100 px centred column. Tiles get a soft shadow on hover and a 150 ms fade on add, using `ctx.animate_bool`.
- Owned tiles: full colour, a ✔ badge and the LB or stars. Unowned tiles: desaturated, 35% opacity and "＋" on hover.
- The choice cards on Welcome get a small illustration (card art collage or uma icons) rather than a single emoji.

### 4.4 Tests

Step navigation on both paths, brush adds, keyboard digits, undo, the paste parser, and the fact that the setup name writes `- Profile:`. Use `App::for_tests` with a temp root, never the real `account/`.

**Done when:** a new user can add 50 cards with correct LBs in under 3 minutes using the brush. Time it yourself with simulated clicks, or estimate it from click counts. Every screen also passes the screenshot check in dark and light at both sizes.

---

## Phase 5: Home as the hub

The owner wants Home to be "the hub for everything" and to "feel nice to be in". Remove **Your strongest cards** (H1) and replace the five tiles (H2).

### 5.1 Layout

Two columns at 1100 px or wider and one column below that, in widget cards that line up (`equal_columns`).

1. **Hero.**
   - "Good evening, <name>" with the time of day, today's date, and a small mascot whose pose changes by time of day. The mascot lives here (Phase 0.3).
   - Below it, a row of **quick actions** as compact icon buttons with labels: Plan a career, Race sim, Add pulls, Look up a skill, Ask.
2. **Champions Meeting command centre.** This is the largest card.
   - Cup name and an announced or estimated badge.
   - A **live countdown** to the next milestone (league selection, Round 1, end).
   - A **course strip** using the Race sim's course map (Phase 8): corners, straights, slopes and phases.
   - Condition badges, the scenario, and **disabled skills**.
   - **The meta at a glance:** the style ranking and recommended stats from `pvp_meta.json`, labelled "community meta".
   - **Your best 3 trainees** for it (`trainee_candidates` with that CM applied).
   - **Your plan progress:** whether a plan exists for this CM, and ✔ trainee / deck / skills / simulated. Buttons: "Continue plan" or "Plan for it", and "Simulate".
3. **Coming up.** A horizontal timeline for the next 3 weeks, built from the live news feed (Phase 3): CMs, League of Heroes, Legend Races (with their race conditions), story events, and scout banners. Each item shows its date range and opens the official article in the browser.
4. **Continue where you left off.** The last opened or saved plans (from `plans/` mtimes), the last Race sim result with its one-line verdict, and the latest result log entries.
5. **Your stable.** Compact and visual:
   - **Collection completion rings:** owned / released umas, and owned / released SSR and SR cards, from `kb` `released()`.
   - **Card strength by type:** keep the bar chart from `cards_by_type`, but shorter and with clearer legend swatches.
   - **Aptitude map:** a heatmap of how many owned umas are A or better for each distance × style. It shows gaps at a glance, for example "No Sprint Front Runners".
   - **Potential to raise:** the top 5 as small uma icons, plus "+N more".
6. **The fun stuff.** The owner liked the idea of something playful. Each widget is small, rotates daily (seeded by date), and can be refreshed with a click.
   - **Uma of the day:** a random owned uma with her art, unique skill, best aptitude, and a "Plan with her" button.
   - **Trainer card:** name, a "trainer since" date (first roster date), and badges earned from the roster, for example "50 umas", "Full house: an MLB SSR of every type", "First CM plan", "First simulation". Badges are computed, so nothing is stored except an optional seen flag.
   - **Showcase:** your **meta-backed cards for the next CM** as art tiles. This replaces the old chips with something useful: the cards `plan_card_meta` rates ★★★ for the next CM's most-recommended style.
   - **Missing pieces:** cards in the next CM's meta decks that you don't own ("on your wishlist"), labelled community meta.
   - **Did you know?:** one tip from `knowledge/mechanics.md` or the glossary (Phase 10), with its source.

### 5.2 Feel

- Numbers count up when Home opens (once per session).
- Cards lift on hover.
- Theme gradient in the hero.
- Nothing jumps while data loads: use skeletons for live data.

**Done when:** Home shows correct live CM data with countdowns, at least four useful widgets and two fun ones, no chip walls, nothing under the mascot, and frame time stays at about 1 ms. Cache derived widget data per roster or plan key; don't recompute it each frame.

---

## Phase 6: Collection redesign

### 6.1 Sidebar and filters (all three sub-tabs)

- Build the sidebar from `FilterSection`s using chips, so it is consistent (G3). The search box at the top has a ✕ and Ctrl+F.
- **Move sort out of the sidebar** into the results header: `Showing 12 of 57 · Sort [Stars ▾][▼] · Pictures | Table`. It shares state with the table headers (C3).
- Show the `active_filter_bar` above results, with "Reset all (n)".
- **Quick views:** one-click presets per sub-tab, saved in settings.json, plus "Save current filters as a view…".
  - Umas: "Maxed potential", "Needs potential", "Long A+", "Dirt A+".
  - Cards: "Maxed SSRs", "Not maxed", "Teaches a gold".
- The sidebar collapses to a thin rail with icons (G7), remembered per sub-tab.
- **Umas filters:**
  - Stars chips. Potential chips P1–P5 (multi).
  - An **aptitude matrix**: three rows (Surface: Turf, Dirt · Distance: Spr, Mile, Med, Long · Style: Front, Pace, Late, End). Clicking a cell cycles its minimum grade: Any → A → S. Active cells are tinted with the grade colour.
  - Toggles: "Has an evolved skill", "Released in the last 60 days".
- **Cards filters:**
  - Type chips with icons and counts. Rarity chips. **LB chips 0, 1, 2, 3 and MLB** (multi), replacing Any / Maxed / Not maxed.
  - "Below level cap".
  - **"Teaches…"** as a skill picker with autocomplete chips (multiple skills, OR), plus a "gold hints only" toggle.
  - **Effect thresholds:** pick an effect from `effect::KEY` and a minimum, at the owned level, for example "Friendship ≥ 25".
  - "Linked in scenario ▾", from `scenarios.json` `link_golds`.

### 6.2 Grid view

- Density S, M or L for tile size. Sticky group headers.
- The hover card shows the key numbers. Quick actions: "Plan with her", "Use in current plan" (cards) and "Compare" (pick two cards for a side-by-side effects table).
- Reserve bottom padding so the last row clears the mascot, if the corner mascot is on.

### 6.3 Table view (C1, C2)

Both tables use `DataTable`.

- **Umas.**
  - Columns: art (28 px), Uma, Outfit, Stars, Potential.
  - Aptitudes as **coloured grade tiles**, the same as the detail panel, under grouped super-headers "Surface · Distance · Style". Every column sorts.
  - Optional hidden-by-default columns: growth % per stat, unique skill, release date.
  - Notes appears only if any note exists.
- **Cards.**
  - Columns: art (28×37), Uma, Card title (with a tooltip of the full title), Rarity badge, Type (icon + colour), LB diamonds (vertically centred), Level as "45 / 50" with a thin progress bar.
  - Effect columns use full names when they fit, otherwise short names with tooltips, never clipped ("Friendship", "Training", "Specialty", "Race bonus", "Hint rate").
  - Values are right-aligned with units (%). Use a **subtle heat tint** relative to the column's maximum. Show "—" for zero.
  - Optional columns: unique effect, hint count, specialty priority, initial bond, mood.
- Both tables offer Columns, Copy as CSV, and sticky headers.

### 6.4 Skills sub-tab (C4)

- **Header.** A segmented control **"Ones I can get (446) | All skills (954)"** replaces the checkbox and the ambiguous chip. The Collection sub-tab label reads "Skills".
- **Filters.**
  - Tier chips in tier colours; category chips with icons; "Works for" style, distance and surface chips.
  - **New "Course" filter:** pick a CM or course, and only skills that can activate there remain. Reuse the activation check in `can_activate_on_plan`, generalised to take a course and conditions.
  - With a CM selected, also **mark disabled skills** (Phase 3).
- **Table.**
  - Skill (tier-coloured pill), SP, Effect (wraps to two lines, so row height 44 here), the trigger summary in one line with the full text on hover.
  - "Only for" as small badges.
  - **"From" as up to four tiny avatars** (uma icons and card thumbs, 20 px) plus "+N", with names on hover, replacing the green text.
  - If a course is selected and the planner's skill chart exists for it, add a **"Gain on course"** column.
- **Detail panel.**
  - Effect, with **duration scaled to a race distance**. Durations scale with distance; check the formula against `sim/builder.rs` before showing a number.
  - Cooldown, the plain-English trigger, and the raw conditions (collapsed).
  - **Upgrade path:** white → gold, and ○ → ◎ (`lower_version`).
  - "Where you can get it", with art.
  - A button **"Test on a course"** that opens Race sim → Which skills help most with this skill added.

### 6.5 Add cards dialog (C5)

- Make it taller (80% of the window) with art tiles instead of text rows. Use the setup tile with the diamond quick-set.
- The owned badge goes inline.
- Missing art is downloaded for every card shown (queue `fetch_card_art`, max 4 at a time).

**Done when:** every C1–C5 item is fixed, the screenshots at 960×600 are usable, and table sorting is unit-tested.

---

## Phase 7: skill auto-pick that actually optimises (K1)

### 7.1 What's wrong: evidence from a live run

- **The setup:** the demo plan (`UMALORD_TAB=planner UMALORD_SELECT=100601 UMALORD_RUN=1`): Oguri Cap at 1200/1000/1000/600/1000, Pace, CM 21 Capricorn Cup, Chukyo 1200, Grand Concert, with a 2547 SP budget.
- **What it picked (after simulating the skills):** 13 skills. The ten visible in the screenshot have single-skill chart gains that add up to **6.95 L** on their own.
- **What a picked set does in a race:** the Race sim demo (`UMALORD_TAB=sim UMALORD_SELECT=100601 UMALORD_RUN=1`) races the same trainee, with and without the estimate-based auto-pick (14 skills including her unique), 1000 runs: **+3.52 L mean**. Seven of those 14 skills alone have chart gains summing to 4.7 L.
  - The two sets differ, so this is a strong hint rather than a measurement. It suggests gains are **not additive**, for these likely reasons:
    - acceleration and speed windows overlap in the same phase
    - several skills fire together
    - wit checks fail
  - **Measure it first:** add an `#[ignore]` test that, for the demo plan, prints the sum of single-skill gains next to the actual Compare gain of the auto-picked set (same seed). It becomes the baseline that the new algorithm must beat.
- **The meta boost dominates.** `META_SKILL_LENGTHS = 2.0` L per unit of meta weight means:
  - Auto-pick bought ★ **Focus** (+0.03 L for 133 SP), ★ **Professor of Curvature** (350 SP for +0.49 L, 0.14 L/100 SP) and ★ **Unstoppable** (346 SP, +0.41 L).
  - It left ★ **Corner Adept ○** (+0.23 L, 170 SP) and ★ **On the Attack** (+0.20 L) unbought.
  - The sheet's advice is a placeholder past CM 21 (CLAUDE.md), yet it outranks the simulation.
- **The chart is optimistic.** It runs with wit check, rushing and lane movement **off**, "so each skill's own effect shows". Real races have them on, so gains are overstated, more so for low-Wit builds.
- **No CM rules.** The official CM 20 and CM 21 rules disable Night Races, Sharp Turns and Collaborative Graded Races (◎ ○ ×). Nothing stops the picker from buying them.
- **No aptitude check.** The trainee had Sprint E on a Sprint course with no warning (K3).

### 7.2 The new algorithm

Put it in `recommend.rs`, with the job plumbing in `plan_sim.rs`.

1. **Candidates.**
   - The same pool as `plan_skill_candidates`.
   - **Plus** skills from saved parents' white sparks, when the plan's parents match `account/parents.md` rows. Inherited sparks give hints, not free skills; check uma.guide's parenting guide and record what you find in `knowledge/mechanics.md`.
   - **Minus:** skills disabled by the plan's CM, and skills that `can_activate_on_plan == Some(false)`.
   - **Minus banned, plus pinned:** new plan fields `pinned_skills` and `banned_skills`, both `#[serde(default)]`.
2. **Cost model.**
   - Base SP × (1 − hint discount at the expected hint level).
   - × 0.9 when **Fast Learner** is on. That's a new plan toggle; confirm the 10% figure on uma.guide before shipping.
   - A gold or ◎ costs its own price **plus** its white or ○ unless that is already chosen or held. Keep the "one per group" rule (`id / 10`).
3. **Screen.** Run the existing skill chart (fast), but with **wit check and rushed ON** (realistic), and keep the top ~25 by gain and by gain/SP, plus every pinned and ★ skill.
4. **Greedy marginal selection with re-simulation.**
   - Start from the baseline build: unique plus innate skills, plus pinned skills.
   - Each step: for every affordable remaining candidate *c*, race **B+c vs B** with **common random numbers** (sync RNG, same seed list), 150 runs, and get the marginal gain *m_c*.
   - Add the candidate with the best *m_c / cost_c*.
   - When the remaining budget is under ~250 SP, switch to the best absolute *m_c* that fits.
   - Stop when no *m_c* > 0.02 L or the budget runs out.
5. **Local search.** Try 1-for-1 swaps (drop *s*, add *c*, still within budget) between the chosen and the best unchosen; take the best improvement. Repeat until there's no gain or 15 s have passed.
6. **Verify.** Run a 1000-run Compare of baseline vs final, and report the mean gain with a 95% interval (1.96·sd/√n), plus **"sum of single gains vs actual"** so the user sees the interaction.
7. **Performance.** Evaluate candidates in parallel with `std::thread::scope`; `SimData` is already `Arc`. The budget is ≤ 30 s in release on 8 threads, with a progress bar and cancel (reuse `SkillJob` and `Progress`). Cache by `plan_key` + candidate set + cost settings.
8. **Meta influence** becomes a plan setting with three levels:
   - **Off:** simulation only.
   - **Tie-break** (the default): +0.05 L × weight, enough to order near-equals.
   - **Strong:** today's behaviour.

   When the sheet recommends a skill that wasn't picked, say why in its row ("+0.03 L for 133 SP on this course in simulation").
9. **Things the single-uma sim can't value.** These are debuffs and position or order skills (the sim treats order conditions as met).
   - List them in a separate "Not simulated" group, with the sheet's advice and a pin button.
   - The CM objective ignores them unless they are pinned.
10. **Objectives.** A dropdown:
    - **"Win the CM race (lengths)"**: the default; the algorithm above.
    - **"Team Trials (consistency)":** weight by activation rate × gain, and prefer cheap reliable skills.
    - **"Rating / parent"**: needs skill evaluation points from a source (Rating/Score Calculator sheet, Phase 10). Build it only once that data is in `knowledge/`; until then show it disabled with "needs data".

### 7.3 Skill shop mode (end of a real career)

Add a toggle in Step 5: **"I'm at the skill shop now"**.

- The user enters the **actual final stats** (these replace the targets for the simulation) and the **actual SP**.
- They tick the **skills on offer with their hint levels**, from the candidate list, with a hint-level stepper 0–5 per row. They can also paste the skill list (fuzzy match, like setup's paste).
- The optimiser then runs on exactly those skills, those costs and those stats.

This is the most practical use of auto-pick, and it removes the uncertainty in the career model's SP estimate.

### 7.4 UI for Step 5

- **Columns:** Pick ✓, 📌/🚫, Skill, **Gain alone**, **Gain in this set** (marginal, after the optimiser ran), Cost, an L/100 SP efficiency bar, Activation % (from the verify run), and Why.
- **Above the table:** a summary line, for example "13 skills · 2497/2547 SP · **+3.5 L in 1000 races** (95%: 3.4–3.6) · singles summed: 7.1 L", and an **Explain** expander with the decision log (step by step: what was added, its marginal gain and cost).
- **Aptitude warning (K3):** if the trainee's grade for the plan's distance, surface or style is below B, show a warning card in Step 2: "Sprint E: she runs about X% slower here". Take X from the simulator's aptitude coefficients in `sim/params.rs`. Suggest pink sparks (`pink_sparks_wanted`) or a better-fitting trainee.

### 7.5 Tests

- The budget and group rules hold.
- Disabled skills are never chosen. Pinned skills are always chosen; banned never.
- Meta "Off" equals simulation-only order.
- **A regression test on the CM 21 demo:** the new pick's verified gain is ≥ the old algorithm's verified gain on the same seed. Keep the old `recommend_skills` as `recommend_skills_v1` under `#[cfg(test)]` for the comparison.
- The run finishes in under 60 s in a `--release` test marked `#[ignore]`.
- Update `auto_pick_picks_with_or_without_a_simulation` and `meta_sheet_picks_come_first`. The latter's assertion must change, because ★ skills are no longer forced unless meta is set to Strong.

**Done when:** on the demo plan, auto-pick's verified gain beats the old pick, the UI shows marginal gains and the explain log, and CM-disabled skills are excluded.

---

## Phase 8: Race sim panel with real graphs (K2)

### 8.1 Layout

- **The left panel becomes "Race setup":**
  - CM chips with clear labels ("Capricorn Cup · Oct 31", "announced" or "estimated")
  - the course picker, with a mini course strip under it
  - conditions chips
  - "What to test" as a segmented control
  - **"Advanced options"** collapsed: runs, seed, sync RNG, wit check, rushed, lanes, spot struggle, dueling
- **Main area:** the two build cards side by side (Swap ⇄, Copy →). Add **"Use meta stats"** (fills the stats from the sheet's recommended stats for this CM) and stat templates.
- **A results dashboard with tabs: Overview · Race · Skills · Stamina · Runs.**

### 8.2 Charts

Put these in `charts.rs`. Use a shared x-axis in metres, aligned across charts.

1. **Course strip.**
   - Phases from `phase_start`: Opening 0–⅙, Middle ⅙–⅔, Final ⅔–⅚, Last spurt ⅚–end.
   - **Corners** numbered and shaded (`course.corners`), straights with the **final straight** highlighted (`course.straights`), and **slopes** as up or down ramps with grade % (`course.slopes`).
   - Each uma's mean **spurt start** marker.
   - Hover tooltips for every feature.
2. **Gap over distance**, the key new view: Uma 2's position minus Uma 1's at the same time, plotted against Uma 1's distance.
   - The median run as a line, plus a **P10–P90 band** across all runs.
   - It shows **where the lengths are won**.
   - It needs `run_comparison` to record the gap at, say, 100 fixed distance checkpoints for every run. That is cheap: an array per run, then percentiles.
3. **Speed.** Both umas, with:
   - axis ticks: every 200 m on x, and m/s on y
   - legend, crosshair tooltip
   - skill activations as labelled markers (names on hover; the top 5 labelled)
   - the spurt start, and the point where HP runs out
4. **HP.** A separate small chart under Speed, not a faint overlay:
   - a 0 line
   - **rushed** segments shaded in `BAD`
   - recovery skill activations marked
5. **Distribution.**
   - A histogram with a labelled x-axis (L) and y-axis (runs)
   - mean and median lines
   - an area split at 0, with "Uma 2 wins 83%"
   - P10 and P90 ticks
   - a CDF toggle
6. **Skill timeline.** One row per skill (Gantt): a **density strip of where it activated across runs**, its rate, and its mean start.
   - `SkillStat` needs a histogram of start positions (e.g. 50 bins over the course) in `sim/run.rs`.
   - Grey rows for skills that never activated, with the likely reason: wit check, condition not met, or disabled in this CM.
7. **Skill contribution** (a button, because it costs runs): leave-one-out for each of Uma 2's skills, 200 runs each, as a sorted horizontal bar chart. "Without X, Uma 2 loses 0.6 L."
8. **Stat sensitivity:** +50 to each stat in turn (300 runs each) as a bar chart: "where your next training points matter most". It reuses the +50 Speed run that the planner already does.
9. **Stamina card:**
   - the HP-at-finish distribution
   - max HP vs needed, as a bar
   - minimum Stamina for a full spurt, and recovery needed (from `hpcalc`)
   - shown in Compare too, not only in the Stamina mode
10. **The verdict card** has larger type: "Uma 2 wins 100% of 1000 races, by 3.5 L (≈ 0.40 s) on average." Add the 95% interval, and a plain note when the gap is within noise.

### 8.3 Extras

- **Run history.** Keep the last 10 runs (setup summary and verdict) in memory. Pin one to compare against the current run; its gap distribution is drawn as a ghost.
- **Replay (fun).** Animate the median run along the course strip: two dots moving, skill pop-ups, play/pause/scrub. It's cheap because the traces exist.
- **Export.** "Copy summary" as markdown, and "Save chart as PNG" (crop `ctx.request_screenshot`, like `debug_screenshot`).

### 8.4 Done when

- Every chart has axes, a legend and a tooltip, and reads well in both themes.
- Gap-over-distance and the skill timeline work.
- The leave-one-out and sensitivity buttons run in the background with progress.
- The left panel fits at 960×600 without scrolling for the basic options.
- `matches_moomoolator` still passes; don't change engine numbers.

---

## Phase 9: Ask becomes an agent that drives the app (A1)

### 9.1 Goal

The user says "build me a Capricorn Cup career with my best Pace trainee and simulate it". The assistant then:

1. creates the plan
2. picks the trainee, deck and skills
3. runs the career and race simulations
4. shows the results in the chat with "Open in Planner" and "Open in Race sim" buttons

The user can see and undo every change.

### 9.2 Architecture

1. **Tool registry** in `app/src/agent/` (new): `tools.rs` and `exec.rs`. Each tool has:
   - a name, a JSON schema and a description
   - `kind`: `Read` (no side effects), `Draft` (changes the in-app working plan or the sim setup; undoable), `Long` (background job), or `Write` (writes files: asks first unless allowed in Settings)
   - a handler

   Move the existing tools from `ask.rs` into it.
2. **New tools:**
   - **Read:**
     - `get_account_summary`
     - `list_upcoming_events` (live CM data with disabled skills)
     - `get_cm_meta(cm, style)`
     - `list_plans`, `get_plan(name | "current")`
     - `get_slot_ranking(slot, top_n)`, `explain_card(card)`
   - **Draft:**
     - `new_plan(goal, cm?, scenario?, title?)`
     - `set_plan(fields…)`: trainee, style, distance, surface, course, conditions, target stats, parents, notes, SP budget, meta influence, pins and bans
     - `recommend_trainees(top_n)`
     - `recommend_deck(replace_all)`, `set_deck(slots)`
     - `set_skills(ids)`
     - `load_plan_into_sim()`, `set_sim_build(which, build)`
   - **Long:**
     - `auto_pick_skills(objective)`: Phase 7's optimiser
     - `simulate_skills()`
     - `run_compare(a, b | "plan", runs ≤ 2000, course?, conditions?)`
     - `run_skill_chart(build, pool, top_n)`
     - `run_stamina(build)`, `stat_sensitivity(build)`
     - `career_sim()`: SP budget and final stats
   - **UI:** `open_page(tab, sub)`, `select(kind, id)`
   - **Write:**
     - `save_plan(title?)`
     - `log_result(kind, fields)`
     - `update_owned(card | uma, lb | stars | potential)`: always asks
3. **Async execution.**
   - Today `ask_poll` runs every tool synchronously on the UI thread.
   - Change this to a queue: Read and Draft tools run immediately on the UI thread.
   - Long tools start a background job (reuse `SkillJob` and the sim `Job`, with progress) and **hold the turn** until it finishes, then send all `tool_result`s for that assistant turn together. The API requires one result per `tool_use`, and parallel calls are allowed.
   - Raise `MAX_TOOL_ROUNDS` to about 30 for agent flows, with a wall-clock guard.
4. **Approval UX.** A Write tool shows an inline chat card ("Save plan 'Capricorn – Oguri Pace'? [Approve] [Deny]"). Deny returns `is_error: true` with "The user declined". Settings → AI holds the auto-approve toggles.
5. **Chat UI.**
   - Render tool activity as compact cards: an icon, what ran, and a mini result (deck art row, verdict line, a tiny gap histogram), with "Open in…" buttons.
   - Stream text over SSE (`curl.exe -N` with `"stream": true`) so long answers appear as they come.
   - Keep `egui_commonmark` for the markdown.
6. **System prompt.**
   - Remove "You can't run race simulations".
   - Add the workflow: understand the goal → `list_upcoming_events` and the meta → draft the plan → recommend → simulate → verify → present, with claims labelled (data, simulation, community meta, opinion), the sim settings used, and an offer to save.
   - Add limits: at most 4 simulations per question unless the user asks for more.
7. **OpenCode backend: the same tools over MCP.**
   - Add `umalord.exe --mcp`, a stdio JSON-RPC MCP server (`initialize`, `tools/list`, `tools/call`) running a headless `App` on the same folder.
   - Point OpenCode at it through its config (an `mcp` entry with a local command). **Check the current OpenCode config schema in its docs before writing it.** Pass it through `OPENCODE_CONFIG_CONTENT` or the project's `opencode.json`, and keep the file and bash permissions denied.
   - The MCP process writes plans to `plans/`. The GUI notices new or changed plan files (an mtime poll every 2 s while Ask is busy) and shows a toast: "The assistant saved 'X' — Open".
   - UI tools are no-ops over MCP in v1. Optionally add a `ui_requests.json` drop file that the GUI polls.
   - As a bonus, Claude Code and Claude Desktop can use the same MCP server. Document it in the README.
8. **Claude API details.**
   - **Load the `claude-api` skill before editing `ask.rs`.**
   - Keep the current model ids (`claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-5-5`), effort, and the `fallbacks` handling.
   - Put `cache_control` on the tools and system prompt.

### 9.3 Tests

- Every tool runs headless against `App::for_tests`: extend `ask_tools_answer_from_real_data`.
- A Draft tool followed by an undo restores the plan.
- Long tools return results after the job completes, driven by polling in the test like `auto_pick_picks_with_or_without_a_simulation`.
- The MCP handshake and a `tools/call` round trip over in-memory stdio.
- The approval card blocks Write tools until approved.

Update `AGENTS.md` (OpenCode can now act through the MCP tools) and CLAUDE.md.

**Done when:** "Plan the next CM for my best Pace uma and simulate it" produces a saved plan (after approval), a deck, skills and a Compare result, all visible in the app, on both backends.

---

## Phase 10: build the knowledge base from the Steam guide's resources

`knowledge/mechanics.md`, `skills.md` and `racecourses.md` are **3–5 lines each** today. The owner pointed to the community resource guide as the place to build them from.

### 10.1 The source

- Steam guide id 3540114219. **Steam has since removed it** ("visible only to the owner").
- The same list is mirrored at **https://rentry.co/uma-resources**, with a changelog at https://rentry.co/uma-resources-changelog.
- Add both to `sources.md`.

### 10.2 Rules

- Follow CLAUDE.md:
  - Record the source and date for every fact.
  - Label community opinion as opinion.
  - Prefer Global sources.
  - Check JP-only content against uma.guide.
  - Summarise; don't copy large passages.
- **How to read each kind of source:**
  - JS-rendered sites: the built-in browser's `get_page_text`, as CLAUDE.md says for uma.guide.
  - Google Sheets: `/gviz/tq?tqx=out:csv&gid=<gid>`.
  - Google Docs: `/document/d/<id>/export?format=txt`.

### 10.3 What to pull, and where it goes

| Resource (from the guide) | Extract | Into |
|---|---|---|
| uma.guide guides: beginners, glossary, team trials, parenting, CM guides, Grand Concert, Umalator 101 | Core mechanics, the CM format (leagues, rounds), Team Trials scoring, inheritance basics | `mechanics.md`, the Help glossary |
| GameTora beginner's guide, CM page, affinity calculator | The CM league rules (Open vs Graded), affinity | `mechanics.md` |
| umamusu.wiki (New Player Guide and race mechanics pages) | Race mechanics: phases, spurt, HP use, rushed (≈1.6× HP drain), position keeping, Wit checks | `mechanics.md`, cross-checked with `sim/` |
| Skills Spreadsheet and Tier List (Dylan, sayaduck) | Tiers for skills the sim can't value (debuffs, position skills) | `skills.md` (opinion) and the Phase 7 "Not simulated" group |
| Hidden Post-Race Skill Hints sheet | Which races give which hints | `skills.md`; later the planner's race suggestions |
| Inheritance / Parenting guide (Crazyfellow), Spark Proc Chances (IceDynamix) | Spark odds by stars and stat, affinity effects | `mechanics.md` → Parents; planner parent hints |
| Required Stamina Calculator sheet, Stamina threshold doc | Stamina targets per course and style | `racecourses.md`; cross-check `hpcalc` |
| Rating / Score Calculator (dak, Neon, IceDynamix) | Skill evaluation points, rating thresholds | `knowledge/` data for the Phase 7 "Rating" objective |
| Estimated Timeline (SoulEC), Another Timeline (Phoenix) | Expected banners and CMs (estimates) | Home "Coming up", labelled estimated, under the official feed |
| Support Card Encyclopedia (Luh), Pull Guide 2.0, Euophrys tier list | Card value notes | `knowledge/support_cards/*.md` (opinion) |
| VFalator, alpha123 Umalator, umasim-en (mee1080), Skill Activation Visualizer | Cross-checks for the sim and skill activation regions | `insights.md` when they disagree |
| Grand Concert guides (uma.guide, GameTora) | Lesson SP and live counts | Calibrate `knowledge/scenarios.json` assumptions (marked sourced or assumed) |
| Skill Optimizer (daftuyda.moe/optimizer) | Feature ideas only: hint levels 0–5, Fast Learner −10%, "optimise for Rating / Team Trials / Aptitude test", skill-list import from screenshots | Phase 7 design |
| uma.moe, ChronoGenesis friend databases | Link-outs for finding a borrow card | A Planner friend-slot hover link |

### 10.4 Using it in the app

- Grow the Help glossary from `mechanics.md`, and make sure every jargon hover has a source.
- Home's "Did you know?" widget.
- The Ask agent's system prompt points at `knowledge/*.md`.
- `AGENTS.md` lists the new files.

**Done when:** each of the three knowledge files has real, sourced sections; `sources.md` lists every resource used with what it covers; and the glossary and tips read from them.

---

## Phase 11: the final UI/UX pass over every page

Run `tools/screenshots.ps1` over the whole matrix, then go through this checklist for **every** screenshot:

- [ ] Nothing overlaps (mascot, tooltips, popups); nothing is clipped without a tooltip.
- [ ] Text baselines line up in rows and table headers; custom widgets are vertically centred.
- [ ] Only theme tokens are used; light mode passes contrast for status text and weak text.
- [ ] Page headers sit at the same height on every tab (`page_frame` and `side_frame`).
- [ ] Every icon-only button has a hover label; hit targets are at least 24 px.
- [ ] Empty states say what to do next; loading states use skeletons or spinners, never a blank space.
- [ ] Toasts confirm saves; destructive actions confirm first.
- [ ] Keyboard: Tab order is sane, Esc closes things, and the shortcuts work.
- [ ] At 960×600, panels collapse and nothing needs horizontal scrolling.
- [ ] Copy reads for a newcomer, and terms have hovers.

Then:

1. Run the full test suite and `frame_times`.
2. Update the README screenshots (with `UMALORD_ROOT`, `UMALORD_PROFILE` and a scratch copy, per CLAUDE.md), `docs/guide.txt`, CLAUDE.md (new modules: `live.rs`, `charts.rs`, `agent/`; new env vars; new settings keys), and `CHANGELOG.md`.
3. Bump to 0.2.0 only when the owner says to release.

---

## Order of work and dependencies

1. Phase 0 (foundation), then Phase 1 (hide Notes; trivial).
2. Phase 2 (Settings) and Phase 3 (live CM data). These are independent; either order works.
3. Phase 4 (onboarding; uses the Settings appearance cards).
4. Phase 5 (Home; uses live CM data, charts and widgets).
5. Phase 6 (Collection; uses `DataTable`, chips and filters).
6. Phase 7 (auto-pick).
7. Phase 8 (Race sim graphs; shares charts and the course strip with Home).
8. Phase 9 (the Ask agent; needs the stable plan and sim APIs from 7–8).
9. Phase 10 (knowledge base). It is independent and can run alongside anything, in a separate session that only touches `knowledge/` and `sources.md`.
10. Phase 11 (final pass).

Each phase is a separate commit series on a feature branch. Show the owner screenshots at the end of each phase before starting the next one.
