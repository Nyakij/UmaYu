# Skills

How skills work, which ones the community rates, what the race simulator can't value, and which race wins give a hidden hint. Each section names its source and the date it was read (all 2026-10-18 unless noted). Labels: **data**, **engine** (the app's simulator), **community** (opinion or community-collected data). For one skill's numbers, use `knowledge/data/skills.json` or the app's Skills page.

## How skills work

Source: uma.guide, Skills Explained (catbomb, peter), https://uma.guide/guides/skills; uma.guide, Racecourse Analysis, https://uma.guide/guides/racecourse-analysis. Engine: `sim/solver.rs`, `sim/builder.rs`.

- **Activation:** every skill except greens rolls a Wit check, max(20, 100 − 9000 / Wit) %. Greens (passives) always apply, except Lucky Seven and Super Lucky Seven. Table by Wit in `mechanics.md`.
- **Duration:** a skill's base duration is for a 1000 m race and scales with the distance (×2.4 at 2400 m). The app's Skills page shows it at each race distance.
- **Cooldown:** most skills fire once per race (500 s). These fire again after 30 s × distance / 1000 (75 s at 2500 m): Professor of Curvature and Corner Adept ○, Beeline Burst and Straightaway Adept, It's On and Ramp Up, See Ya Later! and Playtime's Over!, Corner Connoisseur and Corner Acceleration ○, Rushing Gale! and Straightaway Acceleration, No Stopping Me! and Nimble Navigator, In the Right Place! and Slipstream.
- **Types:** recovery (heals a share of max HP), velocity (raises target speed), acceleration (early: Groundwork, Taking the Lead; late: Let's Pump Some Iron!, Angling and Scheming, On Your Left!), passive / green (adds stats, can pass 1200), vision, debuff, lane movement, and start-delay skills (Focus, Concentration). Some are hybrids (Resplendent Red Ace: velocity and acceleration; Keen Eye: debuff and recovery).
- **Getting them from cards:** an event chain is the most reliable; a one-off event isn't. Hints depend on the card's hint frequency and how many skills share its pool (Tachyon Wit SR: 4; Gold Ship's SSRs: 14); buying a hinted skill shrinks the pool, so the rest come more often.
- **Hint discounts:** 10/20/30/35/40% at levels 1–5; Fast Learner takes another 10% of the base price (`mechanics.md`, Skill costs).
- **Timing beats strength:** where a skill fires decides its value. Acceleration in the accel zone is worth lengths; velocity there is mostly wasted (`racecourses.md`, Reading a course).

## The community tier list

Source: Uma Musume Skills Spreadsheet, Tierlist tab (info by Dylan, @dylank0; formatted by sayaduck; sheet updated 2025-12-12), https://docs.google.com/spreadsheets/d/1oB3eTvKqREtJDWJL0q80O_VjBcpOmRl5xE0z5fZKgFY. Community opinion. `python -I -X utf8 tools/update_knowledge.py --only tiers` turns it into `knowledge/data/skill_tiers.json` (431 skills, by id).

- Each skill has two tiers, one for **Team Trials** and one for **Champions Meeting**: ⍟ essential for its distance or style; ◎ among the best; ◯ good, but short of ◎; ▲ situational (anywhere from ⍟ to ✕ depending on the race); △ only with spare SP; ✕ useless or very inconsistent. The sheet also gives Team Trials points per SP and a one-line reason.
- The sheet's own warning: tiers depend on the track, ▲ most of all. It predates Global's 2026-07-01 skill balance changes (uma.guide has a before/after: https://uma.guide/guides/july1st-skills), so check a skill the change touched.
- Two names on the sheet aren't Global skills: Super Stan and "The View from the Lead is Mine!".
- **Where the app shows it:** Step 5's "Not simulated" list (debuffs and skills that depend on other umas, which the simulator can't value), the Skills page's details, and the assistant's skill lookups. It is shown next to the simulator's numbers, not mixed into them.
- Examples of its Champions Meeting ⍟ picks: the style and distance corner and straight skills (cheap and consistent), Early Lead and Taking the Lead for Front Runners, and course-dependent accels where the course suits them (Straight Descent before a downhill spurt start, Straightaway Spurt and Encroaching Shadow where the spurt starts on a straight). Examples of ✕ in CM: Mile Maven (it fires in the early race, before she reaches top speed, and may not fire at all if it picks a point in the first 5 s).

## Debuffs

Source: uma.guide, Debuff Skills (DT, catbomb, peter), https://uma.guide/guides/skills-debuff. The simulator races one uma against a virtual field, so it can't value a debuff (`skill_pick.rs`, `skill_unsimulated`): judge these by the tier list and the field you expect.

- **HP debuffs:** Subdued [style] fires in the early race and Flustered [style] in the mid race, taking a flat amount of HP from every rival of that style, wherever they are; one of each kind stacks. Trick (Front) and Trick (Rear) need a rushed rival: Front hits those behind while the debuffer is 1st–4th, Rear those ahead while she is 6th–9th. Stronger golds with stricter conditions: Murmur / Mystifying Murmur (Medium, mid race, rivals ahead, after being blocked 1 s), Stamina Eater / Stamina Siphon (Long, mid race, ahead, while 6th–9th), Sharp Gaze / All-Seeing Eyes (Late Surgers, late race, rivals in view), Tantalizing Trick (Halloween Rice Shower).
- **Speed debuffs** cut current speed at once: Hesitant [style] (late race, −0.1 m/s for 3 s base); Intimidate / Adored by All (Sprint, early race after 5 s, rivals behind, while 1st–4th); Speed Eater (Mile, mid race, the 5 nearest behind, while 1st–3rd; no gold on Global); Tether / Dominator (Medium, late race, rivals ahead, while 6th–9th); Intense Gaze (End Closers, late race, rivals in view; no gold on Global).
- **Frenzied [style]** lengthens rivals' rushed status. **Vision:** Smoke Screen / Illusionist (Long, late race, ahead), Disorient / Dazzling Disorientation (Pace Chasers, late race, behind, while 1st–4th).
- Debuffs from several umas add up (rentry FAQ). Champions Meetings can turn skills off: the app reads each CM's list from the official news and never auto-picks those.
- uma.guide's view: as players get more Stamina and recovery, HP debuffs matter less.

## Skills the simulator overrates

Sources: uma.guide, Umalator 101 (https://uma.guide/guides/umalator-101) and Racecourse Analysis; the tier list. Community.

- The simulator treats conditions that depend on other umas as always met (`skill_pick.rs`, `POSITION_CONDITIONS`), so its skill chart is a best case. Unyielding Spirit (needs a change of order) ranks high but rarely fires usefully.
- Random-place skills (Corner Acceleration, Straightaway Acceleration, Highlander, Straight Descent, Rushing Gale!) look fine in a chart averaged over many races but often fire in the wrong place: worth it only when the course has one such feature, where it counts.
- Early-race velocity (Mile Maven) adds little because she hasn't reached her target speed yet.
- The app's auto-pick races sets in realistic races (Wit checks and rushing on) and leaves position-dependent skills to you, for these reasons.

## Team Trials picks

Source: uma.guide, Team Trials (updated 2026-10-05), https://uma.guide/guides/team-trials. Community.

- Score rewards skills that fire, so pick **consistent, generic** skills over strong conditional ones: Professor of Curvature and Lane Legerdemain are good examples; Blinding Flash and Super Lucky Seven are not.
- A consistent unique matters most: Special Week's Shooting Star (pass someone in the top five late in the race) fires almost every race; King Halo's Prideful King (not rushed, 4th–8th with exactly 200 m left) doesn't.
- Recovery skills both save Stamina and score points; Restraint scores in every race and cuts rushing.
- Summer Gold Ship's 564 Escapades fires a random gold in the second half (it can't repeat a gold that already fired, unless its cooldown is up). Don't buy golds that can't fire for the uma's distance and style just because 564 might.
- The app's Step 5 aim "Team Trials (consistency)" weights each skill's gain by how often it fired in simulation.

## Hidden post-race hints

Source: Hidden Guaranteed Post-Race Skill Hints (Causew and others), "Guaranteed Skills" tab, https://docs.google.com/spreadsheets/d/1RnkYfATh3-I9Y_HK_GK7-LEbk1wlvuGDtwDhBaCNZfY. Community-collected (it says these may not be on GameTora); names fixed to Global's spelling. Winning the race in that year gives the trainee a hint for the skill.

| Uma | Race (year) | Skill |
|---|---|---|
| Agnes Tachyon | Satsuki Sho (Classic) | Murmur |
| Agnes Digital | Mile Championship (Classic) | Watchful Eye |
| Air Groove | Sapporo Kinen (Senior) | Sapporo Racecourse |
| Biwa Hayahide | Takarazuka Kinen (Senior) | Prepared to Pass |
| Curren Chan | Takamatsunomiya Kinen (Senior) | Preferred Position |
| Daiwa Scarlet | Tenno Sho (Autumn) (Senior) | Steadfast |
| El Condor Pasa | Japan Cup (Senior) | Prepared to Pass |
| Gold City | Hanshin Juvenile Fillies (Junior) | Updrafters |
| Gold Ship | Tenno Sho (Spring) (Senior) | Pressure |
| Grass Wonder | Takarazuka Kinen (Senior) | Frenzied Pace Chasers |
| Haru Urara | JBC Sprint (Senior) | Meticulous Measures |
| Hishi Amazon | Japan Cup (Senior) | Strategist |
| Kawakami Princess | Japanese Oaks (Classic) | Highlander |
| King Halo | Takamatsunomiya Kinen (Senior) | Homestretch Haste |
| Manhattan Cafe | Arima Kinen (Classic) | Outer Swell |
| Maruzensky | Asahi Hai Futurity Stakes (Junior) | Productive Plan |
| Matikanefukukitaru | Kikuka Sho (Classic) | Inside Scoop |
| Mayano Top Gun | Tenno Sho (Spring) (Senior) | Position Pilfer |
| Mejiro Dober | Japanese Oaks (Classic) | Late Surger Savvy |
| Mejiro McQueen | Kikuka Sho (Classic) | Hydrate |
| Mejiro Ryan | Takarazuka Kinen (Senior) | Competitive Spirit |
| Mihono Bourbon | Tokyo Derby (Classic) | Final Push |
| Narita Brian (Hard) | Tenno Sho (Autumn), Japan Cup, Arima Kinen, Tenno Sho (Spring) (Senior) | Nimble Navigator, Outer Swell, Straightaway Adept, Hydrate (in that order) |
| Narita Taishin | Satsuki Sho (Classic) | Medium Straightaways |
| Nice Nature | Arima Kinen (Senior) | Non-Standard Distance |
| Oguri Cap | Tenno Sho (Autumn) (Senior) | Thunderbolt Step |
| Rice Shower | Takarazuka Kinen (Senior) | Kyoto Racecourse |
| Sakura Bakushin O | Sprinters Stakes (Senior) | Moxie |
| Seiun Sky | Kikuka Sho (Classic) | Leader's Pride |
| Silence Suzuka | Mainichi Okan (Senior) | Early Lead |
| Smart Falcon | JBC Classic (Senior) | Fast-Paced |
| Special Week | Japan Cup (Senior) | Standard Distance |
| Super Creek | Kikuka Sho (Classic) | Extra Tank |
| Symboli Rudolf | Tenno Sho (Spring) (Senior) | Go with the Flow |
| Symboli Rudolf | Satsuki Sho, Tokyo Derby and Kikuka Sho (Classic) | Be Still |
| Taiki Shuttle | Yasuda Kinen (Senior) | Rainy Days |
| Tokai Teio | Tokyo Derby (Classic) | Nimble Navigator |
| Tosen Jordan | Tenno Sho (Autumn) (Senior) | Late Surger Straightaways |
| Vodka | Yasuda Kinen (Classic) | Nimble Navigator |
| Winning Ticket | Tokyo Yushun (Japanese Derby) (Classic) | Tokyo Racecourse |

The sheet's "Random Skills" tab (hints that may or may not come) was still a work in progress.
