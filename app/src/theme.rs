//! Look and feel: palette, fonts, spacing; dark and light modes, and colour themes.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, TextStyle, Theme, Visuals};

/// Name of the semibold font family used for headings and titles.
pub const SEMIBOLD: &str = "semibold";

/// A colour theme: the accent (highlights, selections, main buttons), link colour, and the
/// background's sky and decorations. Each works in dark and light mode. Green stays for good news
/// in every theme, so none of them is green.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    Sky,
    Sakura,
    Sunset,
    Night,
}

impl Look {
    pub const ALL: [Look; 4] = [Look::Sky, Look::Sakura, Look::Sunset, Look::Night];

    pub fn label(self) -> &'static str {
        match self {
            Look::Sky => "Seiun Sky",
            Look::Sakura => "Sakura",
            Look::Sunset => "Sunset",
            Look::Night => "Starry night",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Look::Sky => "Sky blue, with soft clouds",
            Look::Sakura => "Cherry-blossom pink, with falling petals",
            Look::Sunset => "Warm orange, with an evening sun",
            Look::Night => "Violet, with stars and a moon",
        }
    }

    /// The name saved in settings.json.
    pub fn key(self) -> &'static str {
        match self {
            Look::Sky => "sky",
            Look::Sakura => "sakura",
            Look::Sunset => "sunset",
            Look::Night => "night",
        }
    }

    pub fn from_key(key: &str) -> Look {
        Look::ALL.into_iter().find(|l| l.key() == key).unwrap_or(Look::Sky)
    }
}

static LOOK: AtomicU8 = AtomicU8::new(0);

/// The current colour theme.
pub fn look() -> Look {
    Look::ALL[usize::from(LOOK.load(Ordering::Relaxed)).min(Look::ALL.len() - 1)]
}

/// Use a colour theme from now on; `set_look` also restyles a running app.
pub fn use_look(look: Look) {
    LOOK.store(Look::ALL.iter().position(|l| *l == look).unwrap_or(0) as u8, Ordering::Relaxed);
}

/// Switch the running app to a colour theme.
pub fn set_look(ctx: &egui::Context, look: Look) {
    use_look(look);
    for theme in [Theme::Dark, Theme::Light] {
        ctx.set_visuals_of(theme, visuals(theme == Theme::Dark));
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub hover: Color32,
    pub input: Color32,
    pub border: Color32,
    pub text: Color32,
    pub weak: Color32,
    /// Highlights, selections and main buttons: the colour of the theme's background.
    pub accent: Color32,
    /// Text on an accent-filled button.
    pub on_accent: Color32,
    /// The accent, darker: outlines of selected things in light mode.
    pub accent_strong: Color32,
    pub link: Color32,
    pub gold: Color32,
    pub pink: Color32,
    /// The background's colour toward the bottom right, behind the mascot.
    pub sky: Color32,
}

/// Accent, text on it, its strong version, link and sky colour: per theme, dark then light.
fn tint(look: Look, dark: bool) -> [Color32; 5] {
    let c = Color32::from_rgb;
    match (look, dark) {
        (Look::Sky, true) => [c(96, 165, 250), c(10, 20, 36), c(96, 165, 250), c(125, 211, 252), c(30, 52, 84)],
        (Look::Sky, false) => [c(14, 110, 190), Color32::WHITE, c(10, 85, 150), c(3, 105, 161), c(204, 228, 250)],
        (Look::Sakura, true) => [c(244, 143, 177), c(40, 10, 22), c(244, 143, 177), c(249, 168, 212), c(70, 34, 54)],
        (Look::Sakura, false) => [c(204, 52, 108), Color32::WHITE, c(160, 32, 82), c(176, 30, 94), c(252, 221, 233)],
        (Look::Sunset, true) => [c(251, 157, 75), c(40, 20, 6), c(251, 157, 75), c(253, 196, 128), c(86, 44, 34)],
        (Look::Sunset, false) => [c(196, 90, 14), Color32::WHITE, c(150, 66, 8), c(170, 78, 10), c(255, 222, 190)],
        (Look::Night, true) => [c(167, 139, 250), c(22, 14, 46), c(167, 139, 250), c(196, 181, 253), c(40, 32, 84)],
        (Look::Night, false) => [c(109, 58, 214), Color32::WHITE, c(84, 40, 170), c(92, 46, 190), c(226, 218, 252)],
    }
}

/// The palette for the current theme.
pub fn palette(dark: bool) -> Palette {
    palette_for(look(), dark)
}

pub fn palette_for(look: Look, dark: bool) -> Palette {
    let [accent, on_accent, accent_strong, link, sky] = tint(look, dark);
    if dark {
        Palette {
            bg: Color32::from_rgb(17, 19, 24),
            surface: Color32::from_rgb(25, 28, 34),
            raised: Color32::from_rgb(34, 38, 46),
            hover: Color32::from_rgb(43, 48, 58),
            input: Color32::from_rgb(12, 14, 18),
            border: Color32::from_rgb(44, 49, 59),
            text: Color32::from_rgb(231, 233, 238),
            weak: Color32::from_rgb(143, 151, 163),
            accent,
            on_accent,
            accent_strong,
            link,
            gold: Color32::from_rgb(250, 204, 21),
            pink: Color32::from_rgb(244, 114, 182),
            sky,
        }
    } else {
        Palette {
            bg: Color32::from_rgb(243, 244, 246),
            surface: Color32::WHITE,
            raised: Color32::from_rgb(236, 238, 242),
            hover: Color32::from_rgb(226, 229, 234),
            input: Color32::from_rgb(244, 245, 248),
            border: Color32::from_rgb(214, 218, 225),
            text: Color32::from_rgb(29, 33, 41),
            weak: Color32::from_rgb(93, 101, 114),
            accent,
            on_accent,
            accent_strong,
            link,
            gold: Color32::from_rgb(176, 124, 0),
            pink: Color32::from_rgb(219, 39, 119),
            sky,
        }
    }
}

/// The palette for the theme `ui` is drawn in.
pub fn pal(ui: &egui::Ui) -> Palette {
    palette(ui.visuals().dark_mode)
}

fn visuals(dark: bool) -> Visuals {
    let p = palette(dark);
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    // Pages are see-through: main.rs paints the background (and the sky) behind every panel.
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.input;
    // Table stripes tint what's behind them rather than covering it.
    v.faint_bg_color = if dark { Color32::from_white_alpha(7) } else { Color32::from_black_alpha(9) };
    v.code_bg_color = p.raised;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v.hyperlink_color = p.link;
    v.selection.bg_fill = p.accent.gamma_multiply(if dark { 0.28 } else { 0.22 });
    v.selection.stroke = Stroke::new(1.0, p.accent_strong);

    let r = CornerRadius::same(8);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = p.raised;
    w.inactive.weak_bg_fill = p.raised;
    // Light mode needs outlines, or inputs and buttons vanish on white cards.
    w.inactive.bg_stroke = if dark { Stroke::NONE } else { Stroke::new(1.0, p.border) };
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = p.hover;
    w.hovered.weak_bg_fill = p.hover;
    w.hovered.bg_stroke = Stroke::new(1.0, p.accent.gamma_multiply(0.6));
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.hovered.corner_radius = r;
    w.active.bg_fill = p.accent.gamma_multiply(0.35);
    w.active.weak_bg_fill = p.accent.gamma_multiply(0.35);
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(2.0, p.text);
    w.active.corner_radius = r;
    w.open = w.hovered;
    v
}

/// Segoe UI (Windows' UI font) for text, Segoe UI Semibold for headings, then egui's own fonts,
/// Segoe UI Symbol for ☆ ♪ ≠ ∞ ♡, and a Japanese font for event text. Falls back to egui's
/// defaults where a font is missing.
fn install_fonts(ctx: &egui::Context) {
    let dir = std::env::var_os("WINDIR").map_or_else(|| std::path::PathBuf::from(r"C:\Windows"), std::path::PathBuf::from).join("Fonts");
    let mut defs = FontDefinitions::default();
    let mut load = |name: &str, files: &[&str]| -> Option<String> {
        let bytes = files.iter().find_map(|f| std::fs::read(dir.join(f)).ok())?;
        defs.font_data.insert(name.to_string(), Arc::new(FontData::from_owned(bytes)));
        Some(name.to_string())
    };
    let regular = load("Segoe UI", &["segoeui.ttf"]);
    let semibold = load("Segoe UI Semibold", &["seguisb.ttf"]);
    let fallbacks: Vec<String> =
        [load("Segoe UI Symbol", &["seguisym.ttf"]), load("Japanese", &["YuGothR.ttc", "meiryo.ttc", "msgothic.ttc"])].into_iter().flatten().collect();

    let base_prop = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let base_mono = defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    let chain = |first: Vec<String>, base: &[String]| -> Vec<String> {
        let mut v = first;
        v.extend(base.iter().cloned());
        v.extend(fallbacks.iter().cloned());
        v
    };
    defs.families.insert(FontFamily::Proportional, chain(regular.clone().into_iter().collect(), &base_prop));
    defs.families.insert(FontFamily::Monospace, chain(Vec::new(), &base_mono));
    defs.families.insert(FontFamily::Name(SEMIBOLD.into()), chain(semibold.or(regular).into_iter().collect(), &base_prop));
    ctx.set_fonts(defs);
}

pub fn install(ctx: &egui::Context, dark: bool) {
    install_fonts(ctx);
    for theme in [Theme::Dark, Theme::Light] {
        ctx.set_visuals_of(theme, visuals(theme == Theme::Dark));
        ctx.style_mut_of(theme, |s| {
            s.text_styles = [
                (TextStyle::Small, FontId::proportional(11.5)),
                (TextStyle::Body, FontId::proportional(14.0)),
                (TextStyle::Button, FontId::proportional(14.0)),
                (TextStyle::Monospace, FontId::monospace(12.5)),
                (TextStyle::Heading, FontId::new(22.0, FontFamily::Name(SEMIBOLD.into()))),
            ]
            .into();
            s.spacing.item_spacing = egui::vec2(8.0, 6.0);
            s.spacing.button_padding = egui::vec2(10.0, 5.0);
            s.spacing.interact_size.y = 24.0;
            s.spacing.window_margin = Margin::same(12);
            s.spacing.menu_margin = Margin::same(8);
        });
    }
    set_dark(ctx, dark);
}

pub fn set_dark(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark { Theme::Dark } else { Theme::Light });
}
