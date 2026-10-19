# Racecourses

How to read a course, what Team Trials courses look like, notes for Champions Meeting courses, and (generated at the end) the layout of every course a Champions Meeting has used, with a stamina estimate per style. Each section names its source and the date it was read. Labels: **data** (game data, or the simulator's copy of it), **model** (the app's calculators or simulator), **community** (a guide's advice).

## Reading a course

Source: uma.guide, Racecourse Analysis (Hjax, catbomb, Lussless), https://uma.guide/guides/racecourse-analysis, read 2026-10-18. Global. Community guide, with mechanics from KuromiAK's race mechanics doc.

- **The accel zone decides most races.** The last spurt starts at the start of the late race (two thirds of the distance) if the uma has the HP for it, and she accelerates to her top speed from there. Acceleration skills that fire while she is still accelerating are what matter; velocity skills that fire then do little, because she hasn't reached her target speed yet.
- **Check what the accel zone starts on.** Nakayama 2500: the spurt starts at 1667 m on a straight, and the first corner is 33 m in, so first-straight and corner accels both work. Tokyo 2400: it starts at 1600 m on a corner with the final corner 25 m later and no straight in the zone, so Straightaway Spurt can't fire and final-corner skills (Red Shift/LP1211-M) are good.
- **"Gambling" accels** (On Your Left!, Furious Feat, Head-On) fire at a random point in the final leg: better on short courses, where more of the leg is the accel zone.
- **Random-feature skills** pick one at random: Corner Acceleration (a corner), Straightaway Acceleration (a straight), Highlander (an uphill), Straight Descent (a downhill), Rushing Gale! (a straight). Worth it only when the course has one such feature, in the right place.
- **Well-timed accels** are tied to a place: the final corner (Red Shift/LP1211-M; Shooting for Victory in the second half), the first late-race corner (Let's Pump Some Iron!, Moving Past, and Beyond for Late and End; Angling and Scheming for Front), the first late-race straight (Straightaway Spurt, Encroaching Shadow for End). No Stopping Me! fires more often where the spurt starts before the final corner (long courses), since the field is still bunched.
- **Corner numbers** count back from the finish: the last corner is 4, the one before it 3, and after 1 it loops to 4. Skills such as "corner 3" use these numbers, and a random-corner condition picks the last instance of that number. Source: GameTora, Race Mechanics Handbook (robflop, Gertas), https://gametora.com/umamusume/race-mechanics, read 2026-10-18.
- **Speed bonus stats:** some courses list one or two stats; each 300 of a listed stat adds 5% to Speed, up to 901 (20%), averaged when two are listed. Data: `sim/course.rs` (`speed_modifier`) does the same.
- **Final straight before the late race.** Where the final straight starts a few metres before the late race, skills that fire the moment it starts carry their speed into the spurt (uma.guide's CM 21 pre-guide; see below). The generated sections say where each final straight starts.

## Team Trials courses

Source: uma.guide, Team Trials (vinje, catbomb and others; updated 2026-10-05), https://uma.guide/guides/team-trials, read 2026-10-18. Community data (Mango and Ruben's collection); the guide assumes courses are picked at random among those of the distance and surface.

| Slot | Right-handed | Left-handed | Standard distance | Non-standard | Has downhill | Has uphill |
|---|---|---|---|---|---|---|
| Sprint | 65% | 29% | 53% | 47% | 59% | 71% |
| Mile | 71% | 29% | 41% | 59% | 76% | 82% |
| Medium | 57% | 43% | 71% | 29% | 76% | 76% |
| Long | 83% | 17% | 17% | 83% | 75% | 92% |
| Dirt | 50% | 50% | 25% | 75% | 45% | 64% |

- Conditions: Spring 40% (two spring seasons), Summer, Fall and Winter 20% each; Sunny 59%, Cloudy 30%, Rainy 10%, Snowy 1%; Firm 77%, Good 12%, Soft 6%, Heavy 5%. Snow never falls in summer, and the ground is never soft or heavy in sunshine.
- The guide's rule of thumb: a course-specific skill is worth buying for Team Trials only if it works on about 70% of that slot's courses.
- Stamina it suggests as the most you'll need: Sprint 400–450; Mile 600–650; Medium 700–800 (or 600–700 with a gold recovery); Long 800–900 with a gold recovery (or 700 with two). Most Long courses are 2600 m or shorter, but they go up to 3600 m.

## Champions Meeting notes

### CM 21 Capricorn Cup: Chukyo Turf 1200m

Source: uma.guide, Capricorn Cup (CM21 Pre-guide) (causew, catbomb; updated 2026-10-18), https://uma.guide/guides/cm21-guide, read 2026-10-18. Community guide.

- Based on the Takamatsunomiya Kinen; Winter, Sunny, Firm; no speed bonus stats.
- Recommended stats: Speed 1600, Power 1200, Guts 600+, Wit 1200, and no Stamina target: with over 800 m of downhill any amount survives, so train only the Stamina the career races need. Guts settles close finishes, since duels on the final straight are common.
- Since CM 9 the final straight starts in the mid race, 12 m before the late race, so skills that fire as it starts carry speed into the spurt: G00 1st. F∞ (Front and Pace) and Pedal to the Metal (Late) are the reliable ones. Keep the carry-over to about +0.3 m/s of target speed; 12 m isn't enough to reach more.
- Parent-layer accels worth looking for: Victoria por plancha ☆, Budding Blossom, Louder! Tracen Cheer!.
- The app's generated section below agrees on the layout: data from Moomoolator's course file.

## Course data

The block below is generated from the simulator's data; don't edit it by hand. Regenerate it with `cargo test --release course_notes -- --ignored` (in `app/`). Stamina is the minimum for a full spurt by the HP calculator's closed form, which skips position keep, rushing and skills: treat it as a floor, and check a build in the Race sim's Stamina mode.

<!-- generated: course notes (cargo test --release course_notes -- --ignored) -->

Generated 2026-10-19 from Moomoolator's course data (`knowledge/data/sim/course_data.json`, data) and event presets (`knowledge/data/umalator.json`); stamina rows are model output. 15 courses.

### Nakayama Turf 1200m (outer) (course 10501)

- Sprint, turf, right-handed (clockwise), 1200 m.
- Phases: early 0–200, mid 200–800, late 800–1000, last spurt 1000–1200 m. The last spurt mechanic (the accel zone) starts at 800 m.
- Corners, numbered as skills count them: 3: 300–650; 4: 650–890 m.
- Straights: 890–1200 m. The final straight starts at 890 m, 90 m into the late race.
- Slopes: downhill 0–200; uphill 1025–1135 m.
- Speed bonus stats: none.
- Events here (Umalator's presets): CM 16 Leo Cup (2026-07-12, Firm · Sunny · Summer · Midday).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 381 / 277 | 418 / 308 | 367 / 273 | 374 / 280 |
  | 1600 | 423 / 319 | 463 / 353 | 407 / 313 | 414 / 320 |

### Nakayama Turf 2000m (inner) (course 10504)

- Medium, turf, right-handed (clockwise), 2000 m.
- Phases: early 0–333, mid 333–1333, late 1333–1667, last spurt 1667–2000 m. The last spurt mechanic (the accel zone) starts at 1333 m.
- Corners, numbered as skills count them: 1: 375–625; 2: 625–875; 3: 1200–1450; 4: 1450–1690 m.
- Straights: 0–375; 875–1200; 1690–2000 m. The final straight starts at 1690 m, 357 m into the late race.
- Slopes: uphill 125–235; uphill 325–525; downhill 625–1025; uphill 1825–1935 m.
- Speed bonus stats: Speed (+5% Speed per 300 of the stat, up to 901, averaged over the stats).
- Events here (Umalator's presets): CM 12 Aries Cup (2026-04-20, Firm · Sunny · Spring · Midday); CM 23 Pisces Cup (2026-12-06, Firm · Sunny · Spring · Midday, not confirmed).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 756 / 538 | 825 / 595 | 725 / 529 | 737 / 541 |
  | 1600 | 827 / 609 | 901 / 671 | 793 / 597 | 805 / 609 |

### Nakayama Turf 2500m (inner) (course 10506)

- Long, turf, right-handed (clockwise), 2500 m.
- Phases: early 0–417, mid 417–1667, late 1667–2083, last spurt 2083–2500 m. The last spurt mechanic (the accel zone) starts at 1667 m.
- Corners, numbered as skills count them: 3: 100–246; 4: 246–496; 1: 875–1125; 2: 1125–1375; 3: 1700–1950; 4: 1950–2190 m.
- Straights: 496–875; 1375–1700; 2190–2500 m. The final straight starts at 2190 m, 523 m into the late race.
- Slopes: uphill 621–731; uphill 825–1025; downhill 1125–1525; uphill 2325–2435 m.
- Speed bonus stats: Stamina and Guts (+5% Speed per 300 of the stat, up to 901, averaged over the stats).
- Events here (Umalator's presets): CM 8 Sagittarius Cup (2026-01-19, Firm · Sunny · Winter · Midday); CM 20 Sagittarius Cup (2026-10-04, Good · Cloudy · Winter · Midday, not confirmed).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 1045 / 821 | 1137 / 902 | 1001 / 799 | 1016 / 814 |
  | 1600 | 1134 / 911 | 1233 / 997 | 1087 / 885 | 1102 / 900 |

### Tokyo Turf 1600m (course 10602)

- Mile, turf, left-handed (counter-clockwise), 1600 m.
- Phases: early 0–267, mid 267–1067, late 1067–1333, last spurt 1333–1600 m. The last spurt mechanic (the accel zone) starts at 1067 m.
- Corners, numbered as skills count them: 3: 550–825; 4: 825–1075 m.
- Straights: 0–550; 1075–1600 m. The final straight starts at 1075 m, 8 m into the late race.
- Slopes: uphill 325–400; downhill 450–700; uphill 1150–1300 m.
- Speed bonus stats: Stamina and Guts (+5% Speed per 300 of the stat, up to 901, averaged over the stats).
- Events here (Umalator's presets): CM 3 Cancer Cup (2025-10-07, Good · Sunny · Summer · Midday); CM 14 Gemini Cup (2026-05-31, Firm · Sunny · Spring · Midday).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 556 / 422 | 608 / 467 | 534 / 414 | 543 / 423 |
  | 1600 | 612 / 478 | 668 / 527 | 588 / 468 | 597 / 477 |

### Tokyo Turf 2000m (course 10604)

- Medium, turf, left-handed (counter-clockwise), 2000 m.
- Phases: early 0–333, mid 333–1333, late 1333–1667, last spurt 1667–2000 m. The last spurt mechanic (the accel zone) starts at 1333 m.
- Corners, numbered as skills count them: 2: 200–400; 3: 950–1225; 4: 1225–1475 m.
- Straights: 400–950; 1475–2000 m. The final straight starts at 1475 m, 142 m into the late race.
- Slopes: uphill 725–800; downhill 850–1100; uphill 1550–1700 m.
- Speed bonus stats: none.
- Events here (Umalator's presets): CM 7 Scorpio Cup (2025-12-28, Soft · Rainy · Autumn · Midday).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 756 / 620 | 825 / 681 | 725 / 603 | 737 / 614 |
  | 1600 | 827 / 691 | 901 / 757 | 793 / 670 | 805 / 682 |

### Tokyo Turf 2400m (course 10606)

- Medium, turf, left-handed (counter-clockwise), 2400 m.
- Phases: early 0–400, mid 400–1600, late 1600–2000, last spurt 2000–2400 m. The last spurt mechanic (the accel zone) starts at 1600 m.
- Corners, numbered as skills count them: 1: 325–575; 2: 575–900; 3: 1350–1625; 4: 1625–1875 m.
- Straights: 0–325; 900–1350; 1875–2400 m. The final straight starts at 1875 m, 275 m into the late race.
- Slopes: uphill 0–40; uphill 1125–1200; downhill 1250–1500; uphill 1950–2100 m.
- Speed bonus stats: none.
- Events here (Umalator's presets): CM 1 Taurus Cup (2025-08-17, Firm · Sunny · Spring · Midday); CM 13 Taurus Cup (2026-05-10, Firm · Sunny · Spring · Midday).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 983 / 845 | 1071 / 925 | 943 / 817 | 956 / 831 |
  | 1600 | 1069 / 930 | 1163 / 1016 | 1024 / 899 | 1039 / 914 |

### Tokyo Dirt 1600m (course 10611)

- Mile, dirt, left-handed (counter-clockwise), 1600 m.
- Phases: early 0–267, mid 267–1067, late 1067–1333, last spurt 1333–1600 m. The last spurt mechanic (the accel zone) starts at 1067 m.
- Corners, numbered as skills count them: 3: 650–875; 4: 875–1099 m.
- Straights: 0–650; 1099–1600 m. The final straight starts at 1099 m, 32 m into the late race.
- Slopes: downhill 575–775; uphill 1100–1350 m.
- Speed bonus stats: Speed and Stamina (+5% Speed per 300 of the stat, up to 901, averaged over the stats).
- Events here (Umalator's presets): CM 10 Aquarius Cup (2026-03-02, Firm · Sunny · Winter · Midday); CM 22 Aquarius Cup (2026-11-15, Soft · Snowy · Winter · Midday, not confirmed).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 556 / 449 | 608 / 495 | 534 / 438 | 543 / 447 |
  | 1600 | 612 / 505 | 668 / 555 | 588 / 492 | 597 / 501 |

### Chukyo Turf 1200m (course 10701)

- Sprint, turf, left-handed (counter-clockwise), 1200 m.
- Phases: early 0–200, mid 200–800, late 800–1000, last spurt 1000–1200 m. The last spurt mechanic (the accel zone) starts at 800 m.
- Corners, numbered as skills count them: 3: 300–550; 4: 550–788 m.
- Straights: 0–300; 788–1200 m. The final straight starts at 788 m, 12 m before the late race starts, so skills that fire on it carry speed into the spurt.
- Slopes: downhill 100–875; uphill 875–975 m.
- Speed bonus stats: none.
- Events here (Umalator's presets): CM 9 Capricorn Cup (2026-02-09, Soft · Snowy · Winter · Midday); CM 21 Capricorn Cup (2026-10-25, Firm · Sunny · Winter · Midday, not confirmed).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 381 / 101 | 418 / 101 | 367 / 101 | 374 / 101 |
  | 1600 | 423 / 101 | 463 / 101 | 407 / 101 | 414 / 101 |

### Kyoto Turf 2200m (outer) (course 10808)

- Medium, turf, right-handed (clockwise), 2200 m.
- Phases: early 0–367, mid 367–1467, late 1467–1833, last spurt 1833–2200 m. The last spurt mechanic (the accel zone) starts at 1467 m.
- Corners, numbered as skills count them: 1: 400–600; 2: 600–800; 3: 1300–1550; 4: 1550–1797 m.
- Straights: 0–400; 800–1300; 1797–2200 m. The final straight starts at 1797 m, 330 m into the late race.
- Slopes: uphill 1050–1150; uphill 1150–1375; downhill 1375–1525 m.
- Speed bonus stats: Speed (+5% Speed per 300 of the stat, up to 901, averaged over the stats).
- Events here (Umalator's presets): CM 19 Scorpio Cup (2026-09-13, Firm · Sunny · Autumn · Midday, not confirmed).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 866 / 753 | 944 / 823 | 831 / 722 | 843 / 734 |
  | 1600 | 944 / 829 | 1028 / 904 | 905 / 795 | 919 / 807 |

### Kyoto Turf 3000m (outer) (course 10810)

- Long, turf, right-handed (clockwise), 3000 m.
- Phases: early 0–500, mid 500–2000, late 2000–2500, last spurt 2500–3000 m. The last spurt mechanic (the accel zone) starts at 2000 m.
- Corners, numbered as skills count them: 3: 261–511; 4: 511–761; 1: 1250–1450; 2: 1450–1650; 3: 2100–2350; 4: 2350–2597 m.
- Straights: 0–261; 761–1250; 1650–2100; 2597–3000 m. The final straight starts at 2597 m, 597 m into the late race.
- Slopes: uphill 11–111; uphill 111–336; downhill 336–486; uphill 1850–1950; uphill 1950–2175; downhill 2175–2325 m.
- Speed bonus stats: Power and Wit (+5% Speed per 300 of the stat, up to 901, averaged over the stats).
- Events here (Umalator's presets): CM 6 Libra Cup (2025-12-08, Firm · Sunny · Autumn · Midday).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 1380 / 1128 | 1499 / 1227 | 1320 / 1076 | 1338 / 1091 |
  | 1600 | 1488 / 1229 | 1615 / 1335 | 1424 / 1173 | 1442 / 1189 |

### Kyoto Turf 3200m (outer) (course 10811)

- Long, turf, right-handed (clockwise), 3200 m.
- Phases: early 0–533, mid 533–2133, late 2133–2667, last spurt 2667–3200 m. The last spurt mechanic (the accel zone) starts at 2133 m.
- Corners, numbered as skills count them: 3: 458–708; 4: 708–958; 1: 1450–1650; 2: 1650–1850; 3: 2300–2550; 4: 2550–2797 m.
- Straights: 0–458; 958–1450; 1850–2300; 2797–3200 m. The final straight starts at 2797 m, 664 m into the late race.
- Slopes: uphill 208–308; uphill 308–533; downhill 533–683; uphill 2050–2150; uphill 2150–2375; downhill 2375–2525 m.
- Speed bonus stats: none.
- Events here (Umalator's presets): CM 2 Gemini Cup (2025-09-07, Firm · Sunny · Spring · Midday); CM 24 Aries Cup (2026-12-27, Firm · Sunny · Spring · Midday, not confirmed).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 1528 / 1273 | 1658 / 1383 | 1461 / 1214 | 1480 / 1231 |
  | 1600 | 1644 / 1382 | 1782 / 1500 | 1572 / 1319 | 1592 / 1336 |

### Hanshin Turf 1600m (outer) (course 10903)

- Mile, turf, right-handed (clockwise), 1600 m.
- Phases: early 0–267, mid 267–1067, late 1067–1333, last spurt 1333–1600 m. The last spurt mechanic (the accel zone) starts at 1067 m.
- Corners, numbered as skills count them: 3: 450–800; 4: 800–1127 m.
- Straights: 0–450; 1127–1600 m. The final straight starts at 1127 m, 60 m into the late race.
- Slopes: downhill 950–1350; uphill 1405–1525 m.
- Speed bonus stats: Power (+5% Speed per 300 of the stat, up to 901, averaged over the stats).
- Events here (Umalator's presets): CM 5 Virgo Cup (2025-11-16, Firm · Sunny · Autumn · Midday); CM 18 Libra Cup (2026-08-23, Firm · Cloudy · Autumn · Midday, not confirmed).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 556 / 197 | 608 / 218 | 534 / 180 | 543 / 184 |
  | 1600 | 612 / 240 | 668 / 265 | 588 / 221 | 597 / 226 |

### Hanshin Turf 2200m (inner) (course 10906)

- Medium, turf, right-handed (clockwise), 2200 m.
- Phases: early 0–367, mid 367–1467, late 1467–1833, last spurt 1833–2200 m. The last spurt mechanic (the accel zone) starts at 1467 m.
- Corners, numbered as skills count them: 1: 520–710; 2: 710–900; 3: 1250–1550; 4: 1550–1844 m.
- Straights: 0–520; 900–1250; 1844–2200 m. The final straight starts at 1844 m, 377 m into the late race.
- Slopes: downhill 0–290; uphill 295–420; downhill 1400–1995; uphill 2000–2125 m.
- Speed bonus stats: Speed (+5% Speed per 300 of the stat, up to 901, averaged over the stats).
- Events here (Umalator's presets): CM 4 Leo Cup (2025-10-30, Firm · Sunny · Summer · Midday); CM 15 Cancer Cup (2026-06-21, Good · Cloudy · Summer · Midday).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 866 / 104 | 944 / 119 | 831 / 101 | 843 / 101 |
  | 1600 | 944 / 158 | 1028 / 177 | 905 / 137 | 919 / 142 |

### Hanshin Turf 3200m (outer→inner) (course 10914)

- Long, turf, right-handed (clockwise), 3200 m.
- Phases: early 0–533, mid 533–2133, late 2133–2667, last spurt 2667–3200 m. The last spurt mechanic (the accel zone) starts at 2133 m.
- Corners, numbered as skills count them: 3: 370–720; 4: 720–1070; 1: 1520–1710; 2: 1710–1900; 3: 2250–2550; 4: 2550–2844 m.
- Straights: 0–370; 1070–1520; 1900–2250; 2844–3200 m. The final straight starts at 2844 m, 711 m into the late race.
- Slopes: downhill 870–1270; uphill 1325–1445; downhill 2400–2995; uphill 3000–3125 m.
- Speed bonus stats: none.
- Events here (Umalator's presets): CM 11 Pisces Cup (2026-03-26, Heavy · Rainy · Spring · Midday).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 1528 / 630 | 1658 / 688 | 1461 / 586 | 1480 / 595 |
  | 1600 | 1644 / 719 | 1782 / 783 | 1572 / 670 | 1592 / 680 |

### Ooi Dirt 2000m (course 11103)

- Medium, dirt, right-handed (clockwise), 2000 m.
- Phases: early 0–333, mid 333–1333, late 1333–1667, last spurt 1667–2000 m. The last spurt mechanic (the accel zone) starts at 1333 m.
- Corners, numbered as skills count them: 1: 500–650; 2: 650–800; 3: 1300–1450; 4: 1450–1614 m.
- Straights: 0–500; 800–1300; 1614–2000 m. The final straight starts at 1614 m, 281 m into the late race.
- Flat: no slopes.
- Speed bonus stats: Stamina (+5% Speed per 300 of the stat, up to 901, averaged over the stats).
- Events here (Umalator's presets): CM 17 Virgo Cup (2026-08-02, Good · Sunny · Autumn · Evening, not confirmed).
- Stamina for a full spurt (model output, the app's HP calculator, which ports Moomoolator's; Guts 600, Wit 1200, mood normal, no recovery skills; the number after the slash counts expected downhill savings):

  | Speed | Front | Pace | Late | End |
  |---|---|---|---|---|
  | 1200 | 756 / 756 | 825 / 825 | 725 / 725 | 737 / 737 |
  | 1600 | 827 / 827 | 901 / 901 | 793 / 793 | 805 / 805 |

<!-- end of generated course notes -->
