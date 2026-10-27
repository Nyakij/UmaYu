//! Settings: one window with its sections listed on the left (Profile, Appearance, Data and
//! updates, AI assistant, About). Every control saves straight away to settings.json or the
//! account files; there is no Save button.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Margin, Pos2, Rect, RichText, Sense,
    Stroke, StrokeKind, vec2,
};

use crate::theme::{self, Look, pal, size};
use crate::ui_util::UiExt;
use crate::ui_util::{ToastKind, badge, card, chibi, semibold, web_link};
use crate::{App, Tab, lang, settings};

/// The sections, in the order the list shows them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum SettingsPage {
    #[default]
    Profile,
    Appearance,
    Data,
    GameAudio,
    Ai,
    About,
}

impl SettingsPage {
    pub const ALL: [SettingsPage; 6] = [
        SettingsPage::Profile,
        SettingsPage::Appearance,
        SettingsPage::Data,
        SettingsPage::GameAudio,
        SettingsPage::Ai,
        SettingsPage::About,
    ];

    /// Icon, name, and one line on what the section holds (English; shown through `t!`).
    pub fn info(self) -> (&'static str, &'static str, &'static str) {
        match self {
            SettingsPage::Profile => (
                "👤",
                "Profile",
                "Whose account this is, and saving, loading or emptying it.",
            ),
            SettingsPage::Appearance => (
                "🎨",
                "Appearance",
                "Language, dark or light, the colour theme, Seiun Sky, and the size of everything.",
            ),
            SettingsPage::Data => (
                "🗂",
                "Data and updates",
                "Where the game data comes from, how old it is, and how to refresh it.",
            ),
            SettingsPage::GameAudio => (
                "🎵",
                "Game audio",
                "Connect to local game files to play live music, jukebox tracks and cuesheets.",
            ),
            SettingsPage::Ai => (
                "💬",
                "AI assistant",
                "Who answers on the Ask tab: a free model through OpenCode, or Claude with your key.",
            ),
            SettingsPage::About => (
                "ℹ",
                "About",
                "Version, credits, license and keyboard shortcuts.",
            ),
        }
    }

    /// The name `UMALORD_SETTINGS_PAGE` takes.
    pub fn key(self) -> &'static str {
        match self {
            SettingsPage::Profile => "profile",
            SettingsPage::Appearance => "appearance",
            SettingsPage::Data => "data",
            SettingsPage::GameAudio => "gameaudio",
            SettingsPage::Ai => "ai",
            SettingsPage::About => "about",
        }
    }

    pub fn from_key(key: &str) -> Option<SettingsPage> {
        SettingsPage::ALL.into_iter().find(|p| p.key() == key)
    }
}

/// User-facing application name and version label displayed across Settings UI surfaces.
pub const APP_DISPLAY_VERSION: &str = "UmaYu 0.7.1_beta";

#[derive(Default)]
pub(crate) struct SettingsState {
    pub open: bool,
    pub page: SettingsPage,
    /// The trainer name being edited (saved when the box loses focus or on Enter).
    name: String,
    name_loaded: bool,
    /// Persistent input buffer for Global game directory path.
    pub(crate) global_path_buf: String,
    /// Persistent input buffer for JP game directory path.
    pub(crate) jp_path_buf: String,
    /// Persistent input buffer for Export directory path.
    pub(crate) export_dir_buf: String,
    /// Whether the game audio path buffers have been initialized from saved config.
    paths_loaded: bool,
}

/// A rounded rectangle as a polygon, for painting gradients with rounded corners.
fn rounded_poly(rect: Rect, r: f32) -> Vec<Pos2> {
    let r = r.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let corners = [
        (rect.right_bottom() - vec2(r, r), 0.0),
        (rect.left_bottom() + vec2(r, -r), 0.5),
        (rect.left_top() + vec2(r, r), 1.0),
        (rect.right_top() + vec2(-r, r), 1.5),
    ];
    let mut pts = Vec::new();
    for (c, start) in corners {
        for i in 0..=6 {
            let a = std::f32::consts::PI * (start + 0.5 * i as f32 / 6.0);
            pts.push(c + vec2(a.cos() * r, a.sin() * r));
        }
    }
    pts
}

/// A painted thumbnail of the app in a theme: the page with its sky, the top bar with a selected
/// tab, a card with two lines of text, and a main button.
fn look_preview(painter: &egui::Painter, rect: Rect, look: Look, dark: bool) {
    let painter = &painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let p = theme::palette_for(look, dark);
    // The page, shading toward the theme's sky at the bottom right (Ink stays plain).
    let poly = rounded_poly(rect, 8.0);
    let mut mesh = egui::Mesh::default();
    let center = rect.center();
    let shade = |q: Pos2| {
        if look == Look::Ink {
            return p.bg;
        }
        let t = (((q.x - rect.left()) / rect.width() + (q.y - rect.top()) / rect.height()) / 2.0)
            .clamp(0.0, 1.0);
        p.bg.lerp_to_gamma(p.sky, t * t)
    };
    mesh.colored_vertex(center, shade(center));
    for q in &poly {
        mesh.colored_vertex(*q, shade(*q));
    }
    let n = poly.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(egui::Shape::mesh(mesh));
    // Top bar with three tabs, the first selected.
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), 18.0));
    painter.rect_filled(
        bar,
        CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        },
        p.surface,
    );
    painter.circle_filled(bar.left_center() + vec2(10.0, 0.0), 4.0, p.accent);
    for i in 0..3 {
        let tab = Rect::from_min_size(
            bar.left_top() + vec2(22.0 + i as f32 * 30.0, 5.0),
            vec2(24.0, 8.0),
        );
        painter.rect_filled(
            tab,
            CornerRadius::same(3),
            if i == 0 {
                p.accent.gamma_multiply(if dark { 0.45 } else { 0.3 })
            } else {
                p.weak.gamma_multiply(0.35)
            },
        );
    }
    // A card with a title line, a text line and a main button.
    let card = Rect::from_min_size(
        rect.left_top() + vec2(10.0, 28.0),
        vec2(rect.width() * 0.62, rect.height() - 40.0),
    );
    painter.rect_filled(card, CornerRadius::same(6), p.surface);
    painter.rect_stroke(
        card,
        CornerRadius::same(6),
        Stroke::new(1.0, p.border),
        StrokeKind::Inside,
    );
    painter.rect_filled(
        Rect::from_min_size(
            card.left_top() + vec2(8.0, 8.0),
            vec2(card.width() * 0.6, 6.0),
        ),
        CornerRadius::same(3),
        p.text.gamma_multiply(0.85),
    );
    painter.rect_filled(
        Rect::from_min_size(
            card.left_top() + vec2(8.0, 20.0),
            vec2(card.width() * 0.75, 4.0),
        ),
        CornerRadius::same(2),
        p.weak.gamma_multiply(0.7),
    );
    painter.rect_filled(
        Rect::from_min_size(
            card.left_top() + vec2(8.0, 28.0),
            vec2(card.width() * 0.5, 4.0),
        ),
        CornerRadius::same(2),
        p.weak.gamma_multiply(0.7),
    );
    let button = Rect::from_min_size(card.left_bottom() + vec2(8.0, -18.0), vec2(42.0, 12.0));
    painter.rect_filled(button, CornerRadius::same(4), p.accent);
    painter.rect_filled(
        button.shrink2(vec2(10.0, 4.5)),
        CornerRadius::same(1),
        p.on_accent,
    );
    // A little of each theme's decoration.
    let corner = rect.right_bottom() - vec2(24.0, 22.0);
    match look {
        Look::Sky | Look::Sakura => {
            let cloud = Color32::from_white_alpha(if dark { 30 } else { 170 });
            for (dx, dy, r) in [(-8.0, 2.0, 8.0), (2.0, -2.0, 10.0), (12.0, 3.0, 7.0)] {
                painter.circle_filled(corner + vec2(dx, dy), r, cloud);
            }
            if look == Look::Sakura {
                for (dx, dy) in [(-40.0, -40.0), (-18.0, -56.0), (-60.0, -16.0)] {
                    painter.add(egui::Shape::ellipse_filled(
                        corner + vec2(dx, dy),
                        vec2(4.0, 2.5),
                        p.pink.gamma_multiply(0.6),
                    ));
                }
            }
        }
        Look::Sunset => {
            painter.circle_filled(
                corner + vec2(-4.0, -6.0),
                13.0,
                Color32::from_rgb(251, 146, 60).gamma_multiply(if dark { 0.45 } else { 0.7 }),
            );
        }
        Look::Night => {
            painter.circle_filled(
                corner + vec2(-4.0, -12.0),
                8.0,
                Color32::from_rgb(240, 236, 210).gamma_multiply(if dark { 0.7 } else { 0.95 }),
            );
            for (dx, dy) in [
                (-40.0, -46.0),
                (-20.0, -30.0),
                (-56.0, -24.0),
                (-10.0, -56.0),
            ] {
                painter.circle_filled(corner + vec2(dx, dy), 1.4, p.accent.gamma_multiply(0.8));
            }
        }
        Look::Track => {
            let rail = if dark {
                Color32::from_white_alpha(70)
            } else {
                Color32::from_rgb(146, 100, 0).gamma_multiply(0.5)
            };
            let c = rect.right_bottom() + vec2(0.0, 40.0);
            let arc: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = std::f32::consts::PI * (1.0 + 0.5 * i as f32 / 16.0);
                    c + vec2(a.cos() * 70.0, a.sin() * 50.0)
                })
                .collect();
            painter.add(egui::Shape::line(arc, Stroke::new(1.5, rail)));
        }
        Look::Ink => {}
    }
    painter.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, p.border),
        StrokeKind::Inside,
    );
}

/// A row with a label in a fixed column on the left, and the control after it. The label and
/// its hover are translated here.
fn setting_row(ui: &mut egui::Ui, label: &str, help: &str, add: impl FnOnce(&mut egui::Ui)) {
    let p = pal(ui);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(150.0, 28.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(150.0);
                let r = ui.add(egui::Label::new(semibold(t!(label), size::BODY)).wrap());
                if !help.is_empty() {
                    r.on_hover_text(t!(help));
                }
            },
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            add(ui);
            let _ = p;
        });
    });
    ui.add_space(theme::space(theme::gap::MD));
}

/// A section heading inside a page, translated here.
fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(theme::space(theme::gap::SM));
    ui.label(semibold(t!(text), size::MD));
    ui.add_space(theme::space(theme::gap::XS));
}

/// Big buttons in one row where exactly one is on, for choices like the mode. Labels and hovers
/// are translated here.
fn big_choice<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    value: T,
    options: &[(T, &str, &str)],
) -> Option<T> {
    let p = pal(ui);
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for (v, label, hover) in options {
            let on = *v == value;
            let text = semibold(t!(label), size::BODY).color(if on { p.text } else { p.weak });
            let b = egui::Button::new(text)
                .fill(if on {
                    p.accent
                        .gamma_multiply(if ui.visuals().dark_mode { 0.28 } else { 0.16 })
                } else {
                    p.raised
                })
                .stroke(Stroke::new(
                    if on { 1.5 } else { 1.0 },
                    if on { p.accent_strong } else { p.border },
                ))
                .corner_radius(CornerRadius::same(10))
                .min_size(vec2(132.0, 40.0));
            let mut r = ui.add(b);
            if !hover.is_empty() {
                r = r.on_hover_text(t!(hover));
            }
            if r.clicked() && !on {
                picked = Some(*v);
            }
        }
    });
    picked
}

/// A command with a Copy button, for things to run in a terminal.
fn command_line(ui: &mut egui::Ui, cmd: &str) {
    let p = pal(ui);
    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(p.input)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.label(RichText::new(cmd).monospace().size(size::SM));
            });
        if ui
            .small_btn(t!("📋 Copy"))
            .on_hover_text(t!(
                "Copy the command, to paste in PowerShell in the UmaYu folder"
            ))
            .clicked()
        {
            ui.ctx().copy_text(cmd.to_string());
        }
    });
}

impl App {
    /// Open Settings at a section.
    pub(crate) fn open_settings(&mut self, page: SettingsPage) {
        self.settings.open = true;
        self.settings.page = page;
        self.settings.name_loaded = false;
        self.settings.paths_loaded = false;
    }

    /// The Settings window, while it's open.
    pub(crate) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings.open {
            return;
        }
        if !self.settings.name_loaded {
            self.settings.name = self.roster.profile.clone().unwrap_or_default();
            self.settings.name_loaded = true;
        }
        if !self.settings.paths_loaded {
            self.settings.global_path_buf = self.gameaudio.global.game_path.clone();
            self.settings.jp_path_buf = self.gameaudio.jp.game_path.clone();
            self.settings.export_dir_buf = self.gameaudio.export_dir.clone();
            self.settings.paths_loaded = true;
        }
        let screen = ctx.content_rect();
        let size = vec2(
            (screen.width() - 48.0).min(900.0),
            (screen.height() - 56.0).min(640.0),
        );
        let p = theme::palette(self.dark);
        let frame = egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(CornerRadius::same(theme::RADIUS + 2))
            .inner_margin(Margin::ZERO);
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::unique("settings_window"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_min_size(size);
                ui.set_max_size(size);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    // The section list.
                    egui::Frame::new()
                        .fill(p.raised)
                        .corner_radius(CornerRadius {
                            nw: theme::RADIUS + 2,
                            sw: theme::RADIUS + 2,
                            ne: 0,
                            se: 0,
                        })
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                ui.set_width(176.0);
                                ui.set_height(size.y - 24.0);
                                ui.label(semibold(t!("⚙ Settings"), size::LG));
                                ui.add_space(10.0);
                                for page in SettingsPage::ALL {
                                    let (icon, label, what) = page.info();
                                    let on = self.settings.page == page;
                                    let (rect, resp) =
                                        ui.allocate_exact_size(vec2(176.0, 34.0), Sense::click());
                                    let painter = ui.painter();
                                    if on {
                                        painter.rect_filled(
                                            rect,
                                            CornerRadius::same(8),
                                            p.accent.gamma_multiply(if self.dark {
                                                0.26
                                            } else {
                                                0.15
                                            }),
                                        );
                                        painter.rect_filled(
                                            Rect::from_min_size(
                                                rect.left_top() + vec2(0.0, 7.0),
                                                vec2(3.0, 20.0),
                                            ),
                                            CornerRadius::same(2),
                                            p.accent,
                                        );
                                    } else if resp.hovered() {
                                        painter.rect_filled(rect, CornerRadius::same(8), p.hover);
                                    }
                                    let font = if on {
                                        FontId::new(
                                            size::BODY,
                                            FontFamily::Name(theme::SEMIBOLD.into()),
                                        )
                                    } else {
                                        FontId::proportional(size::BODY)
                                    };
                                    painter.text(
                                        rect.left_center() + vec2(12.0, 0.0),
                                        Align2::LEFT_CENTER,
                                        format!("{icon}  {}", t!(label)),
                                        font,
                                        if on { p.text } else { p.weak },
                                    );
                                    if resp
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .on_hover_text(t!(what))
                                        .clicked()
                                    {
                                        self.settings.page = page;
                                    }
                                }
                                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                                    ui.label(
                                        RichText::new(APP_DISPLAY_VERSION)
                                            .size(size::XS)
                                            .color(p.weak),
                                    );
                                    ui.label(
                                        RichText::new(t!("Changes save as you make them."))
                                            .size(size::XS)
                                            .color(p.weak),
                                    );
                                });
                            });
                        });
                    // The open section.
                    egui::Frame::new()
                        .inner_margin(Margin {
                            left: 22,
                            right: 18,
                            top: 16,
                            bottom: 12,
                        })
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                ui.set_width(size.x - 200.0 - 40.0);
                                ui.set_height(size.y - 28.0);
                                let (icon, label, what) = self.settings.page.info();
                                ui.horizontal(|ui| {
                                    ui.label(semibold(format!("{icon} {}", t!(label)), size::XL));
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new("✕").size(size::LG),
                                                    )
                                                    .frame(false),
                                                )
                                                .on_hover_text(t!("Close (Esc)"))
                                                .clicked()
                                            {
                                                close = true;
                                            }
                                        },
                                    );
                                });
                                ui.label(RichText::new(t!(what)).size(13.0).color(p.weak));
                                ui.add_space(8.0);
                                ui.separator();
                                let mut scroll =
                                    egui::ScrollArea::vertical().auto_shrink([false, false]);
                                // UMALORD_SCROLL holds it scrolled down, for screenshots of the lower part.
                                if let Some(y) = self.debug_scroll {
                                    scroll = scroll.vertical_scroll_offset(y);
                                }
                                scroll.show(ui, |ui| {
                                    ui.set_max_width(ui.available_width() - 14.0);
                                    ui.add_space(6.0);
                                    match self.settings.page {
                                        SettingsPage::Profile => {
                                            self.settings_profile(ui, &mut close)
                                        }
                                        SettingsPage::Appearance => self.settings_appearance(ui),
                                        SettingsPage::Data => self.settings_data(ui, &mut close),
                                        SettingsPage::GameAudio => self.settings_gameaudio(ui),
                                        SettingsPage::Ai => self.settings_ai(ui),
                                        SettingsPage::About => self.settings_about(ui),
                                    }
                                    ui.add_space(12.0);
                                });
                            });
                        });
                });
            });
        if close || modal.should_close() {
            self.save_trainer_name();
            self.commit_gameaudio_paths_if_changed();
            self.settings.open = false;
            self.settings.paths_loaded = false;
        }
    }

    /// Persist any edited game audio paths if they changed from current config.
    fn commit_gameaudio_paths_if_changed(&mut self) {
        if !self.settings.paths_loaded {
            return;
        }
        let global_trimmed = self.settings.global_path_buf.trim().to_string();
        if global_trimmed != self.gameaudio.global.game_path {
            self.settings.global_path_buf = global_trimmed.clone();
            self.gameaudio
                .set_region_path(crate::gameaudio::GameRegion::Global, global_trimmed);
        }
        let jp_trimmed = self.settings.jp_path_buf.trim().to_string();
        if jp_trimmed != self.gameaudio.jp.game_path {
            self.settings.jp_path_buf = jp_trimmed.clone();
            self.gameaudio
                .set_region_path(crate::gameaudio::GameRegion::Jp, jp_trimmed);
        }
        let export_trimmed = self.settings.export_dir_buf.trim().to_string();
        if export_trimmed != self.gameaudio.export_dir {
            self.settings.export_dir_buf = export_trimmed.clone();
            self.gameaudio.set_export_dir(export_trimmed);
        }
    }

    /// Write the edited trainer name to account/umas.md, if it changed.
    fn save_trainer_name(&mut self) {
        let name = self.settings.name.trim().to_string();
        if name.is_empty() || Some(&name) == self.roster.profile.as_ref() {
            return;
        }
        let Some(root) = self.root.clone() else {
            return;
        };
        let path = root.join(crate::data::UMAS_FILE);
        let result = std::fs::read_to_string(&path)
            .and_then(|text| std::fs::write(&path, crate::data::set_meta(&text, "Profile", &name)));
        match result {
            Ok(()) => {
                self.reload_roster();
                self.write_digest();
                self.toast(tf!("Renamed the account to {}.", name), ToastKind::Success);
            }
            Err(e) => self.toast(tf!("Couldn't save the name: {}", e), ToastKind::Error),
        }
    }

    fn settings_profile(&mut self, ui: &mut egui::Ui, close: &mut bool) {
        let p = pal(ui);
        setting_row(
            ui,
            "Trainer name",
            "Whose account this is. It's saved in account/umas.md and in profiles you save.",
            |ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.settings.name)
                        .hint_text(t!("e.g. Sam"))
                        .desired_width(260.0)
                        .font(FontId::proportional(size::MD)),
                );
                if r.lost_focus() {
                    self.save_trainer_name();
                }
                ui.label(
                    RichText::new(t!(
                        "Saved when you press Enter or click elsewhere. Home greets you by it."
                    ))
                    .size(size::SM)
                    .color(p.weak),
                );
            },
        );
        setting_row(
            ui,
            "Server",
            "Uma Musume runs separate servers with different content. The game data in this app is for Global.",
            |ui| {
                ui.label(
                    self.roster
                        .server
                        .clone()
                        .unwrap_or_else(|| "Global (English)".into())
                        .replace("**", ""),
                );
                ui.label(
                    RichText::new(t!(
                        "The game data, meta sheet and simulator are all for Global."
                    ))
                    .size(size::SM)
                    .color(p.weak),
                );
            },
        );
        setting_row(ui, "Your collection", "", |ui| {
            ui.label(tf!(
                "{} umas · {} support cards{}",
                self.roster.umas.len(),
                self.roster.cards.len(),
                self.roster
                    .cards_updated
                    .as_ref()
                    .map(|d| tf!(" · last updated {}", d))
                    .unwrap_or_default()
            ));
        });
        setting_row(
            ui,
            "Data folder",
            "Everything the app saves lives here: your account, plans, results and profiles.",
            |ui| match &self.root {
                Some(root) => {
                    ui.label(
                        RichText::new(root.display().to_string())
                            .monospace()
                            .size(size::SM),
                    );
                    if ui.button(t!("📂 Open folder")).clicked() {
                        let _ = std::process::Command::new("explorer").arg(root).spawn();
                    }
                }
                None => {
                    _ = ui.label(RichText::new(t!("The UmaYu folder wasn't found.")).color(p.warn))
                }
            },
        );

        heading(ui, "Your data");
        let action = |ui: &mut egui::Ui, text: &str, about: &str| {
            ui.horizontal(|ui| {
                let b = ui.add(
                    egui::Button::new(RichText::new(t!(text)).size(size::BODY))
                        .min_size(vec2(210.0, 32.0)),
                );
                ui.add(
                    egui::Label::new(RichText::new(t!(about)).size(size::SM).color(p.weak)).wrap(),
                );
                b.clicked()
            })
            .inner
        };
        if action(
            ui,
            "📤 Save my data",
            "Your umas, cards, parents, plans and results in one .umalord.json file, to keep or share.",
        ) {
            match self.export_profile() {
                Ok(path) => self.toast_action(
                    tf!("Saved {}.", path.display()),
                    ToastKind::Success,
                    t!("Show"),
                    crate::ui_util::ToastAction::Open(path),
                ),
                Err(e) => self.toast(e, ToastKind::Error),
            }
        }
        if action(
            ui,
            "📥 Load a profile",
            "Switch to a saved account. What's here is backed up first.",
        ) {
            self.open_setup(crate::onboarding::Step::Import);
            *close = true;
        }
        if action(
            ui,
            "📝 Set up my collection",
            "Click the umas and cards you own, and set stars and limit breaks.",
        ) {
            self.open_setup(crate::onboarding::Step::Umas);
            *close = true;
        }

        heading(ui, "Danger zone");
        egui::Frame::new().stroke(Stroke::new(1.0, p.bad.gamma_multiply(0.7))).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let b = egui::Button::new(RichText::new(t!("⚠ Start over…")).color(Color32::WHITE)).fill(p.bad).min_size(vec2(0.0, 30.0));
                if ui.add(b).clicked() {
                    self.onboarding.confirm_start_over = true;
                    *close = true;
                }
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.label(semibold(t!("Start over"), size::BODY));
                    ui.add(
                        egui::Label::new(RichText::new(t!("Back this account up to profiles/backups, then empty the app for a new one. You'll be asked to confirm.")).size(size::SM).color(p.weak))
                            .wrap(),
                    );
                });
            });
        });
    }

    /// Tiếng Việt or English, as big buttons; applies at once and is saved as `lang`.
    pub(crate) fn language_picker(&mut self, ui: &mut egui::Ui) {
        let options = lang::Lang::ALL.map(|l| (l, l.label(), ""));
        if let Some(l) = big_choice(ui, lang::current(), &options) {
            lang::set(l);
            settings::set("lang", serde_json::json!(l.key()));
            ui.ctx().request_repaint();
        }
    }

    /// System, Light or Dark, as big buttons. Shared with setup's "Make it yours".
    pub(crate) fn mode_picker(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let options = [
            (
                theme::Mode::System,
                theme::Mode::System.label(),
                "Follow Windows: dark when Windows is dark",
            ),
            (theme::Mode::Light, theme::Mode::Light.label(), ""),
            (theme::Mode::Dark, theme::Mode::Dark.label(), ""),
        ];
        if let Some(m) = big_choice(ui, self.mode, &options) {
            self.set_mode(&ctx, m);
        }
    }

    /// The colour themes as cards with a painted preview each; a click applies one.
    pub(crate) fn theme_picker(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let ctx = ui.ctx().clone();
        let w = ui.available_width();
        let cols = if w >= 600.0 { 3 } else { 2 };
        let gap = 10.0;
        let tile = vec2((w - gap * (cols - 1) as f32) / cols as f32, 156.0);
        for row in Look::ALL.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for look in row {
                    let on = self.look == *look;
                    let (rect, resp) = ui.allocate_exact_size(tile, Sense::click());
                    let painter = ui.painter();
                    if resp.hovered() && !on {
                        painter.rect_filled(rect.expand(2.0), CornerRadius::same(10), p.hover);
                    }
                    let thumb = Rect::from_min_size(rect.min, vec2(rect.width(), 100.0));
                    look_preview(painter, thumb, *look, self.dark);
                    if on {
                        painter.rect_stroke(
                            thumb.expand(1.5),
                            CornerRadius::same(9),
                            Stroke::new(2.5, p.accent_strong),
                            StrokeKind::Outside,
                        );
                        let tick = thumb.right_top() + vec2(-14.0, 14.0);
                        painter.circle_filled(tick, 9.0, p.accent);
                        painter.text(
                            tick,
                            Align2::CENTER_CENTER,
                            "✔",
                            FontId::proportional(size::SM),
                            p.on_accent,
                        );
                    }
                    painter.text(
                        thumb.left_bottom() + vec2(2.0, 8.0),
                        Align2::LEFT_TOP,
                        t!(look.label()),
                        FontId::new(size::BODY, FontFamily::Name(theme::SEMIBOLD.into())),
                        p.text,
                    );
                    let about = painter.layout(
                        t!(look.about()).to_string(),
                        FontId::proportional(size::XS),
                        p.weak,
                        rect.width() - 4.0,
                    );
                    painter.galley(thumb.left_bottom() + vec2(2.0, 27.0), about, p.weak);
                    if resp
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                        && !on
                    {
                        self.look = *look;
                        theme::set_look(&ctx, *look);
                        settings::set("look", serde_json::json!(look.key()));
                    }
                }
            });
            ui.add_space(gap);
        }
    }

    /// The sky-and-decorations switch.
    pub(crate) fn background_toggle(&mut self, ui: &mut egui::Ui) {
        if ui
            .tick(&mut self.mascot, t!("Sky and decorations behind the pages"))
            .changed()
        {
            settings::set("mascot", serde_json::json!(self.mascot));
        }
    }

    /// Where chibi Seiun Sky goes.
    pub(crate) fn chibi_picker(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            chibi(ui, 0, 64.0);
            ui.vertical(|ui| {
                let options = [
                    (
                        theme::Chibi::Everywhere,
                        "Everywhere",
                        "On Home, on empty pages, and small beside every page title",
                    ),
                    (
                        theme::Chibi::Home,
                        "Home and empty pages",
                        "Beside Home's welcome, and on pages with nothing to show yet",
                    ),
                    (theme::Chibi::Off, "Off", "Nowhere"),
                ];
                if let Some(c) = big_choice(ui, theme::chibi(), &options) {
                    theme::use_chibi(c);
                    settings::set("chibi", serde_json::json!(c.key()));
                }
            });
        });
    }

    fn settings_appearance(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let ctx = ui.ctx().clone();
        setting_row(
            ui,
            "Language",
            "The language of the app's labels and messages.",
            |ui| {
                self.language_picker(ui);
            },
        );
        setting_row(ui, "Mode", "", |ui| self.mode_picker(ui));
        setting_row(
            ui,
            "Theme",
            "The accent colour of buttons and highlights, and the background. Click one to try it.",
            |ui| self.theme_picker(ui),
        );
        setting_row(ui, "Background", "", |ui| {
            self.background_toggle(ui);
            ui.label(
                RichText::new(t!(
                    "The theme's gradient, clouds, petals, stars or rails. Ink has none."
                ))
                .size(size::SM)
                .color(p.weak),
            );
        });
        setting_row(
            ui,
            "Seiun Sky",
            "Chibi Seiun Sky, the app's mascot. She's only drawn where nothing else is, so she never covers your data.",
            |ui| self.chibi_picker(ui),
        );
        setting_row(
            ui,
            "Size",
            "Makes everything bigger or smaller, text and all. Ctrl+plus and Ctrl+minus do the same for this session.",
            |ui| {
                ui.horizontal(|ui| {
                    let mut scale = self.ui_scale;
                    ui.spacing_mut().slider_width = 240.0;
                    let slider = egui::Slider::new(&mut scale, theme::UI_SCALE)
                        .step_by(0.05)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0));
                    let r = ui.add(slider);
                    // Apply when the drag ends, so the slider doesn't move under the mouse.
                    if r.changed() {
                        self.ui_scale = scale;
                    }
                    if (r.drag_stopped() || (r.changed() && !r.dragged()))
                        && (ctx.zoom_factor() - self.ui_scale).abs() > 0.001
                    {
                        ctx.set_zoom_factor(self.ui_scale);
                        settings::set("ui_scale", serde_json::json!(self.ui_scale));
                    }
                    if (self.ui_scale - 1.0).abs() > 0.001
                        && ui.button(t!("Reset to 100%")).clicked()
                    {
                        self.ui_scale = 1.0;
                        ctx.set_zoom_factor(1.0);
                        settings::set("ui_scale", serde_json::json!(1.0));
                    }
                });
            },
        );
        setting_row(
            ui,
            "Density",
            "How much space there is between things.",
            |ui| {
                let options = [
                    (false, "Comfortable", "Roomier, easier to read"),
                    (true, "Compact", "Tighter spacing, more on screen"),
                ];
                if let Some(c) = big_choice(ui, self.compact, &options) {
                    self.compact = c;
                    theme::set_compact(&ctx, c);
                    settings::set(
                        "density",
                        serde_json::json!(if c { "compact" } else { "comfortable" }),
                    );
                }
            },
        );
        setting_row(
            ui,
            "Navigation",
            "Choose which tabs appear in the top bar. Home is always shown.",
            |ui| {
                let mut show_collection = !self.hidden_tabs.contains("collection");
                let mut show_career = !self.hidden_tabs.contains("career");
                let mut show_ask = !self.hidden_tabs.contains("ask");
                let mut show_music = !self.hidden_tabs.contains("music");

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(16.0, 6.0);
                    if ui
                        .tick(&mut show_collection, t!("📚 Collection"))
                        .on_hover_text(t!("Show or hide the Collection tab"))
                        .changed()
                    {
                        self.set_tab_hidden("collection", !show_collection);
                    }
                    if ui
                        .tick(&mut show_career, t!("🏇 Career"))
                        .on_hover_text(t!("Show or hide the Career tab"))
                        .changed()
                    {
                        self.set_tab_hidden("career", !show_career);
                    }
                    if ui
                        .tick(&mut show_ask, t!("💬 Ask"))
                        .on_hover_text(t!("Show or hide the Ask tab"))
                        .changed()
                    {
                        self.set_tab_hidden("ask", !show_ask);
                    }
                    if ui
                        .tick(&mut show_music, t!("🎵 Music"))
                        .on_hover_text(t!(
                            "Show or hide the Music tab (when Game audio is enabled)"
                        ))
                        .changed()
                    {
                        self.set_tab_hidden("music", !show_music);
                    }
                });
                ui.label(RichText::new(t!("Home is always shown as a safe default. Hidden tabs can still be accessed via shortcuts.")).size(size::SM).color(p.weak));
                ui.add_space(theme::space(theme::gap::XS));
                let mut open_pinned = self.open_pinned_tab_on_start;
                if ui
                    .tick(&mut open_pinned, t!("Open pinned tab on startup"))
                    .on_hover_text(t!(
                        "Start UmaYu on your pinned favorite tab instead of Home"
                    ))
                    .changed()
                {
                    self.set_open_pinned_tab_on_start(open_pinned);
                }
                ui.horizontal(|ui| {
                match self.pinned_tab {
                    Some(pinned) => {
                        let (icon, label, _) = pinned.info();
                        let is_hidden = self.is_tab_hidden(pinned) || (pinned == Tab::Music && !self.gameaudio.enabled);
                        let pin_desc = if is_hidden {
                            tf!("Pinned tab: {0} {1} (Currently hidden — Home will open on startup)", icon, t!(label))
                        } else {
                            tf!("Pinned tab: {0} {1}", icon, t!(label))
                        };
                        ui.label(RichText::new(pin_desc).size(size::SM));
                        if ui.button(t!("Unpin")).clicked() {
                            self.set_pinned_tab(None);
                        }
                    }
                    None => {
                        ui.label(RichText::new(t!("No pinned tab. Right-click any tab in the top bar to pin it.")).size(size::SM).color(p.weak));
                    }
                }
            });
            },
        );
    }

    fn settings_data(&mut self, ui: &mut egui::Ui, close: &mut bool) {
        let p = pal(ui);
        let day = |s: &Option<String>| {
            s.as_deref()
                .map(|b| b[..10.min(b.len())].to_string())
                .unwrap_or_else(|| t!("unknown").into())
        };
        heading(ui, "Champions Meeting schedule");
        ui.label(
            RichText::new(t!(
                "Official dates, race conditions and the skills that won't activate come from the game's news on umamusume.com. Umalator (umalator.app) gives the exact course and estimates the events not announced yet."
            ))
            .size(size::SM)
            .color(p.weak),
        );
        ui.add_space(4.0);
        let mut on = crate::live::enabled();
        if ui
            .tick(
                &mut on,
                t!("Fetch the latest schedule when the app starts (at most every 6 hours)"),
            )
            .changed()
        {
            crate::live::set_enabled(on);
        }
        let now = crate::live::now();
        let source_line = |ui: &mut egui::Ui, name: &str, s: Option<&crate::live::SourceStatus>| {
            ui.horizontal_wrapped(|ui| {
                ui.label(semibold(name, size::BODY));
                match s {
                    Some(s) if s.ok => {
                        ui.label(RichText::new(format!("✔ {}", s.message)).color(p.good));
                    }
                    Some(s) => {
                        ui.label(RichText::new(format!("✖ {}", s.message)).color(p.warn));
                    }
                    None => {
                        ui.label(RichText::new(t!("not fetched yet")).color(p.weak));
                    }
                }
                if let Some(t) = s.and_then(|s| s.fetched_at) {
                    ui.label(
                        RichText::new(tf!("· fetched {}", crate::live::fmt_ago(now - t)))
                            .color(p.weak),
                    );
                }
            });
        };
        let data = self.live.data.clone();
        source_line(ui, "umamusume.com", data.as_ref().map(|d| &d.news));
        source_line(ui, "umalator.app", data.as_ref().map(|d| &d.umalator));
        ui.horizontal(|ui| {
            if self.live.rx.is_some() {
                ui.spinner();
                ui.label(RichText::new(t!("Checking…")).color(p.weak));
            } else if ui
                .add(egui::Button::new(t!("🔄 Refresh now")).min_size(vec2(0.0, 30.0)))
                .on_hover_text(t!("Fetch the news and Umalator's presets again"))
                .clicked()
            {
                self.refresh_live(true);
            }
            if let Some(d) = &data {
                ui.label(
                    RichText::new(tf!(
                        "Last checked {}.",
                        crate::live::fmt_ago(now - d.checked_at)
                    ))
                    .size(size::SM)
                    .color(p.weak),
                );
            }
        });
        ui.add_space(6.0);
        heading(ui, "Game data");
        setting_row(
            ui,
            "uma.guide",
            "Every uma, support card, skill, race and event on the Global server.",
            |ui| {
                ui.label(tf!(
                    "Built {} · {} umas · {} support cards · {} skills · {} races",
                    day(&self.kb.built),
                    self.kb.umas.len(),
                    self.kb.cards.len(),
                    self.kb.skills.len(),
                    self.kb.races.len()
                ));
                if let Some(e) = &self.kb.error {
                    ui.label(RichText::new(format!("⚠ {e}")).color(p.warn));
                }
            },
        );
        setting_row(
            ui,
            "Umalator",
            "The race simulator's skills and courses, and its Champions Meeting presets (Moomoolator, umalator.app).",
            |ui| {
                ui.label(tf!(
                    "Built {} · {} courses · {} event presets",
                    day(&self.kb.umalator_built),
                    self.kb.umalator_courses.len(),
                    self.kb.umalator_presets.len()
                ));
            },
        );
        setting_row(ui, "Meta sheet", "", |ui| match &self.kb.pvp_meta {
            Some(m) => {
                ui.label(tf!("Read {} · community opinion, not game data", m.fetched));
                ui.label(RichText::new(&m.authors).size(size::SM).color(p.weak));
            }
            None => {
                _ = ui.label(
                    RichText::new(t!(
                        "Not downloaded (knowledge/data/pvp_meta.json is missing)."
                    ))
                    .color(p.warn),
                )
            }
        });
        setting_row(
            ui,
            "Skill tier list",
            "The community skills spreadsheet's Team Trials and Champions Meeting tiers, shown on skills and used by the Rating auto-pick.",
            |ui| match &self.kb.skill_tiers {
                Some(t) => {
                    ui.label(tf!(
                        "Sheet updated {} · {} skills · community opinion, not game data",
                        t.sheet_updated,
                        t.len()
                    ))
                    .on_hover_text(format!("{}\n\n{}", t.caveat, t.source));
                    ui.label(RichText::new(&t.authors).size(size::SM).color(p.weak));
                }
                None => {
                    _ = ui.label(
                        RichText::new(t!(
                            "Not downloaded (knowledge/data/skill_tiers.json is missing)."
                        ))
                        .color(p.warn),
                    )
                }
            },
        );
        heading(ui, "Refreshing it");
        ui.label(
            RichText::new(t!("Install Python 3, then run one of these in PowerShell from the UmaYu folder, and press 🔄 Reload (F5). The meta sheet changes most often."))
                .size(size::SM)
                .color(p.weak),
        );
        ui.add_space(4.0);
        command_line(ui, "python -I -X utf8 tools\\update_knowledge.py");
        command_line(
            ui,
            "python -I -X utf8 tools\\update_knowledge.py --only meta",
        );
        heading(ui, "Advanced");
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(t!("📝 Browse notes and data files")).min_size(vec2(0.0, 30.0))).on_hover_text(t!("Read the markdown files in this folder: insights, plans, results, your roster files and the knowledge notes")).clicked() {
                self.open_notes();
                *close = true;
            }
            if let Some(root) = &self.root
                && ui.add(egui::Button::new(t!("📂 Open the game data folder")).min_size(vec2(0.0, 30.0))).clicked()
            {
                let _ = std::process::Command::new("explorer").arg(root.join("knowledge").join("data")).spawn();
            }
        });
    }

    pub(crate) fn gameaudio_music_prefs(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        setting_row(
            ui,
            "Output device",
            "Choose default audio playback output device for Music.",
            |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let current_label = if self.gameaudio.output_device.is_empty() {
                            t!("System default (auto)").to_string()
                        } else {
                            self.gameaudio.output_device.clone()
                        };

                        let mut selected = self.gameaudio.output_device.clone();
                        egui::ComboBox::from_id_salt("gameaudio_output_device_combo")
                            .selected_text(RichText::new(&current_label).size(size::SM))
                            .width(280.0)
                            .truncate()
                            .show_ui(ui, |ui| {
                                let is_def = selected.is_empty();
                                if ui.selectable_label(is_def, t!("System default (auto)")).clicked() {
                                    selected = String::new();
                                }
                                for dev in &self.gameaudio.output_devices {
                                    let is_sel = selected == *dev;
                                    if ui.selectable_label(is_sel, dev).clicked() {
                                        selected = dev.clone();
                                    }
                                }
                            })
                            .response
                            .on_hover_text(t!(
                                "Music plays through the selected device instead of the Windows default. Headphones must be connected and active to appear in the list."
                            ));

                        if selected != self.gameaudio.output_device {
                            self.gameaudio.set_output_device(selected);
                        }

                        if ui
                            .button(t!("🔄 Rescan"))
                            .on_hover_text(t!(
                                "Rescan active audio output devices (e.g. newly plugged headphones)"
                            ))
                            .clicked()
                        {
                            self.gameaudio.refresh_output_devices();
                        }
                    });

                    let active_desc = self
                        .gameaudio
                        .active_device()
                        .unwrap_or_else(|| t!("System default"));
                    ui.label(
                        RichText::new(tf!("Active: {}", active_desc))
                            .size(size::SM)
                            .color(p.weak),
                    );
                });
            },
        );

        setting_row(
            ui,
            "Default volume",
            "Default music playback volume applied on startup.",
            |ui| {
                let p = pal(ui);
                let mut vol = self.gameaudio.volume * 100.0;
                if ui
                    .add(egui::Slider::new(&mut vol, 0.0..=300.0).suffix("%"))
                    .changed()
                {
                    self.request_set_volume(vol / 100.0);
                }
                ui.label(
                    RichText::new(t!(
                        "⚠ Volumes above 200% may cause excessive loudness and risk hearing damage."
                    ))
                    .size(size::SM)
                    .color(p.warn),
                );
            },
        );

        setting_row(
            ui,
            "Default vocal volume",
            "Default vocal volume for Music Live performers applied on startup.",
            |ui| {
                let mut vol = (self.gameaudio.live_vocal_default_volume * 100.0).round();
                if ui
                    .add(egui::Slider::new(&mut vol, 0.0..=100.0).suffix("%"))
                    .changed()
                {
                    let clamped = (vol / 100.0).clamp(0.0, 1.0);
                    self.set_live_vocal_default_volume(clamped);
                }
            },
        );
    }

    fn settings_gameaudio(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        heading(ui, "Game audio");
        ui.label(
            RichText::new(t!(
                "Connect to local Uma Musume game installation files to enable Audio Player, Jukebox, and Live Music."
            ))
            .size(size::SM)
            .color(p.weak),
        );
        ui.add_space(theme::space(theme::gap::SM));

        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(semibold(t!("🎵 Music (Beta)"), size::BODY));
                ui.add_space(4.0);
                badge(ui, t!("Beta"), p.accent);
            });
            ui.add_space(2.0);
            ui.label(
                RichText::new(t!(
                    "The Music and Game audio player is currently in Beta. It reads local game files offline to provide audio preview, Jukebox songs, and multi-track Live performances. Compatibility and available tracks depend on your locally downloaded game assets."
                ))
                .size(size::SM)
                .color(p.weak),
            );
        });
        ui.add_space(theme::space(theme::gap::SM));

        setting_row(
            ui,
            "Enable",
            "Enable Game Audio features using local game assets.",
            |ui| {
                let mut enabled = self.gameaudio.enabled;
                if ui
                    .checkbox(&mut enabled, t!("Enable Game Audio plug-in"))
                    .changed()
                {
                    self.gameaudio.set_enabled(enabled);
                }
            },
        );

        if !self.gameaudio.enabled {
            ui.add_space(theme::space(theme::gap::MD));
            ui.label(
                RichText::new(t!(
                    "Game audio is currently disabled. Enable it above to configure and load game tracks."
                ))
                .size(size::SM)
                .color(p.weak),
            );
            return;
        }

        self.gameaudio_music_prefs(ui);

        setting_row(
            ui,
            "Active region",
            "Select active game data region (Global or JP) used in Music.",
            |ui| {
                let current_reg = self.gameaudio.active_region;
                let reg_options = [
                    (
                        crate::gameaudio::GameRegion::Global,
                        "Global",
                        "Use Global game data",
                    ),
                    (crate::gameaudio::GameRegion::Jp, "JP", "Use JP game data"),
                ];
                if let Some(picked) = big_choice(ui, current_reg, &reg_options) {
                    self.gameaudio.set_active_region(picked);
                }
            },
        );

        heading(ui, "Global version");
        setting_row(
            ui,
            "Global directory",
            "Path to the Persistent directory containing dat, meta, and master folders for Global.",
            |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.settings.global_path_buf)
                                .desired_width(280.0)
                                .hint_text(t!("e.g. C:\\...\\LocalLow\\Cygames\\umamusume")),
                        );
                        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if (resp.lost_focus() || (resp.has_focus() && enter_pressed))
                            && self.settings.global_path_buf.trim()
                                != self.gameaudio.global.game_path
                        {
                            let trimmed = self.settings.global_path_buf.trim().to_string();
                            self.settings.global_path_buf = trimmed.clone();
                            self.gameaudio
                                .set_region_path(crate::gameaudio::GameRegion::Global, trimmed);
                        }
                        if ui
                            .button(t!("📁 Browse…"))
                            .on_hover_text(t!("Select Global game directory via folder picker"))
                            .clicked()
                        {
                            let mut dialog = rfd::FileDialog::new()
                                .set_title(t!("Select Global Game Directory"));
                            let cur = self.settings.global_path_buf.trim();
                            if !cur.is_empty() {
                                let p = std::path::Path::new(cur);
                                if p.is_dir() {
                                    dialog = dialog.set_directory(p);
                                } else if let Some(parent) = p.parent() {
                                    if parent.is_dir() {
                                        dialog = dialog.set_directory(parent);
                                    }
                                }
                            }
                            if let Some(folder) = dialog.pick_folder() {
                                let p_str = folder.to_string_lossy().into_owned();
                                self.settings.global_path_buf = p_str.clone();
                                self.gameaudio
                                    .set_region_path(crate::gameaudio::GameRegion::Global, p_str);
                            }
                        }
                        if ui
                            .button(t!("🔄 Reload Global"))
                            .on_hover_text(t!(
                                "Reload Global manifest and master databases from disk"
                            ))
                            .clicked()
                        {
                            self.gameaudio
                                .start_load_region(crate::gameaudio::GameRegion::Global);
                        }
                    });

                    match &self.gameaudio.global.status {
                        crate::gameaudio::GameAudioStatus::Disabled
                        | crate::gameaudio::GameAudioStatus::Idle => {
                            if self.gameaudio.global.game_path.is_empty() {
                                ui.label(
                                    RichText::new(t!("No Global directory configured."))
                                        .color(p.warn)
                                        .size(size::SM),
                                );
                            } else {
                                ui.label(
                                    RichText::new(t!("Idle. Ready to load."))
                                        .color(p.weak)
                                        .size(size::SM),
                                );
                            }
                        }
                        crate::gameaudio::GameAudioStatus::Scanning => {
                            ui.label(
                                RichText::new(format!("⏳ {}", t!("Scanning…")))
                                    .color(p.accent)
                                    .size(size::SM),
                            );
                        }
                        crate::gameaudio::GameAudioStatus::Loading => {
                            ui.label(
                                RichText::new(format!("⏳ {}", t!("Loading Global database…")))
                                    .color(p.accent)
                                    .size(size::SM),
                            );
                        }
                        crate::gameaudio::GameAudioStatus::Ready => {
                            ui.label(
                                RichText::new(tf!(
                                    "✓ Global: {} audio entries · {} live songs, {} jukebox tracks",
                                    self.gameaudio.global.manifest_entries.len(),
                                    self.gameaudio.global.master.live_songs.len(),
                                    self.gameaudio.global.master.jukebox_songs.len()
                                ))
                                .size(size::SM)
                                .color(p.good),
                            );
                        }
                        crate::gameaudio::GameAudioStatus::Error(err) => {
                            ui.label(
                                RichText::new(format!("⚠ {err}"))
                                    .color(p.warn)
                                    .size(size::SM),
                            );
                        }
                    }
                });
            },
        );

        heading(ui, "JP version");
        setting_row(
            ui,
            "JP directory",
            "Path to the Persistent directory containing dat, meta, and master folders for JP (Steam or DMM).",
            |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.settings.jp_path_buf)
                                .desired_width(280.0)
                                .hint_text(t!(
                                    "e.g. C:\\...\\UmamusumePrettyDerby_Jpn or DMM Persistent"
                                )),
                        );
                        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if (resp.lost_focus() || (resp.has_focus() && enter_pressed))
                            && self.settings.jp_path_buf.trim() != self.gameaudio.jp.game_path
                        {
                            let trimmed = self.settings.jp_path_buf.trim().to_string();
                            self.settings.jp_path_buf = trimmed.clone();
                            self.gameaudio
                                .set_region_path(crate::gameaudio::GameRegion::Jp, trimmed);
                        }
                        if ui
                            .button(t!("📁 Browse…"))
                            .on_hover_text(t!("Select JP game directory via folder picker"))
                            .clicked()
                        {
                            let mut dialog =
                                rfd::FileDialog::new().set_title(t!("Select JP Game Directory"));
                            let cur = self.settings.jp_path_buf.trim();
                            if !cur.is_empty() {
                                let p = std::path::Path::new(cur);
                                if p.is_dir() {
                                    dialog = dialog.set_directory(p);
                                } else if let Some(parent) = p.parent() {
                                    if parent.is_dir() {
                                        dialog = dialog.set_directory(parent);
                                    }
                                }
                            }
                            if let Some(folder) = dialog.pick_folder() {
                                let p_str = folder.to_string_lossy().into_owned();
                                self.settings.jp_path_buf = p_str.clone();
                                self.gameaudio
                                    .set_region_path(crate::gameaudio::GameRegion::Jp, p_str);
                            }
                        }
                        if ui
                            .button(t!("🔄 Reload JP"))
                            .on_hover_text(t!("Reload JP manifest and master databases from disk"))
                            .clicked()
                        {
                            self.gameaudio
                                .start_load_region(crate::gameaudio::GameRegion::Jp);
                        }
                    });

                    match &self.gameaudio.jp.status {
                        crate::gameaudio::GameAudioStatus::Disabled
                        | crate::gameaudio::GameAudioStatus::Idle => {
                            if self.gameaudio.jp.game_path.is_empty() {
                                ui.label(
                                    RichText::new(t!("No JP directory configured."))
                                        .color(p.warn)
                                        .size(size::SM),
                                );
                            } else {
                                ui.label(
                                    RichText::new(t!("Idle. Ready to load."))
                                        .color(p.weak)
                                        .size(size::SM),
                                );
                            }
                        }
                        crate::gameaudio::GameAudioStatus::Scanning => {
                            ui.label(
                                RichText::new(format!("⏳ {}", t!("Scanning…")))
                                    .color(p.accent)
                                    .size(size::SM),
                            );
                        }
                        crate::gameaudio::GameAudioStatus::Loading => {
                            ui.label(
                                RichText::new(format!("⏳ {}", t!("Loading JP database…")))
                                    .color(p.accent)
                                    .size(size::SM),
                            );
                        }
                        crate::gameaudio::GameAudioStatus::Ready => {
                            ui.label(
                                RichText::new(tf!(
                                    "✓ JP: {} audio entries · {} live songs, {} jukebox tracks",
                                    self.gameaudio.jp.manifest_entries.len(),
                                    self.gameaudio.jp.master.live_songs.len(),
                                    self.gameaudio.jp.master.jukebox_songs.len()
                                ))
                                .size(size::SM)
                                .color(p.good),
                            );
                        }
                        crate::gameaudio::GameAudioStatus::Error(err) => {
                            ui.label(
                                RichText::new(format!("⚠ {err}"))
                                    .color(p.warn)
                                    .size(size::SM),
                            );
                        }
                    }
                });
            },
        );

        heading(ui, "Game discovery");
        setting_row(
            ui,
            "Detected games",
            "Scan for installed Global and JP game clients.",
            |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .button(t!("🔍 Auto-detect"))
                            .on_hover_text(t!(
                                "Scan for Global, Steam JP, and DMM JP installations"
                            ))
                            .clicked()
                        {
                            self.gameaudio.start_detection();
                        }
                        if self.gameaudio.status == crate::gameaudio::GameAudioStatus::Scanning {
                            ui.spinner();
                            ui.label(
                                RichText::new(t!("Scanning for installations…"))
                                    .color(p.weak)
                                    .size(size::SM),
                            );
                        }
                    });

                    let mut assign_path: Option<(crate::gameaudio::GameRegion, String)> = None;
                    for inst in &self.gameaudio.detected_installs {
                        let is_active = (inst.region == crate::gameaudio::GameRegion::Global
                            && inst.path.to_string_lossy() == self.gameaudio.global.game_path)
                            || (inst.region == crate::gameaudio::GameRegion::Jp
                                && inst.path.to_string_lossy() == self.gameaudio.jp.game_path);
                        let valid_badge = if inst.is_valid() {
                            format!("✓ {}", t!("Valid"))
                        } else {
                            format!("⚠ {}", t!("Incomplete"))
                        };
                        let reg_badge = inst.region.label();
                        ui.horizontal(|ui| {
                            let btn_text = format!("[{reg_badge}] {} ({valid_badge})", inst.label);
                            let b = ui.button(RichText::new(btn_text).color(if is_active {
                                p.accent
                            } else {
                                p.text
                            }));
                            if b.on_hover_text(format!(
                                "{}\n{}",
                                inst.path.display(),
                                t!("Click to assign as game directory")
                            ))
                            .clicked()
                            {
                                assign_path =
                                    Some((inst.region, inst.path.to_string_lossy().into_owned()));
                            }
                        });
                    }
                    if let Some((reg, p_str)) = assign_path {
                        match reg {
                            crate::gameaudio::GameRegion::Global => {
                                self.settings.global_path_buf = p_str.clone()
                            }
                            crate::gameaudio::GameRegion::Jp => {
                                self.settings.jp_path_buf = p_str.clone()
                            }
                        }
                        self.gameaudio.set_region_path(reg, p_str);
                    }
                });
            },
        );

        setting_row(
            ui,
            "Export directory",
            "Custom base folder where exported WAV files are saved (files, jukebox, live).",
            |ui| {
                ui.horizontal(|ui| {
                    let hint = self.gameaudio.default_export_dir_string();
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.settings.export_dir_buf)
                            .desired_width(280.0)
                            .hint_text(&hint),
                    );
                    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (resp.lost_focus() || (resp.has_focus() && enter_pressed))
                        && self.settings.export_dir_buf.trim() != self.gameaudio.export_dir
                    {
                        let trimmed = self.settings.export_dir_buf.trim().to_string();
                        self.settings.export_dir_buf = trimmed.clone();
                        self.gameaudio.set_export_dir(trimmed);
                    }
                    if ui
                        .button(t!("🔄 Reset"))
                        .on_hover_text(t!("Reset export directory to default (exports/audio)"))
                        .clicked()
                    {
                        self.settings.export_dir_buf.clear();
                        self.gameaudio.set_export_dir(String::new());
                    }
                    if ui
                        .button(t!("📂 Open folder"))
                        .on_hover_text(t!("Open export folder in Windows Explorer"))
                        .clicked()
                    {
                        if let Err(e) = self.gameaudio.open_export_dir() {
                            self.toast(e, crate::ui_util::ToastKind::Warn);
                        }
                    }
                });
            },
        );

        setting_row(
            ui,
            "Advanced: Key",
            "Custom hex encryption key for meta database (optional).",
            |ui| {
                let mut key_str = self.gameaudio.custom_key.clone();
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut key_str)
                        .desired_width(320.0)
                        .hint_text(t!("Leave blank to use standard game keys")),
                );
                if resp.lost_focus() && key_str != self.gameaudio.custom_key {
                    self.gameaudio.set_custom_key(key_str);
                }
            },
        );
    }

    fn settings_ai(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        let busy = self.ask_busy();
        setting_row(
            ui,
            "Answered by",
            "Who answers your questions on the Ask tab.",
            |ui| {
                let options = [
                    (
                        crate::ask::Backend::OpenCode,
                        "🆓 OpenCode · free models",
                        "Free models through OpenCode. No account or key; weaker than Claude.",
                    ),
                    (
                        crate::ask::Backend::ClaudeApi,
                        "🔑 Claude API · your key",
                        "Claude with your own Anthropic API key; use is billed to that key.",
                    ),
                ];
                ui.add_enabled_ui(!busy, |ui| {
                    if let Some(b) = big_choice(ui, self.ask.backend, &options) {
                        self.ask.backend = b;
                        settings::set(
                            "ask_backend",
                            serde_json::json!(if b == crate::ask::Backend::OpenCode {
                                "opencode"
                            } else {
                                "claude_api"
                            }),
                        );
                    }
                });
                if busy {
                    ui.label(
                        RichText::new(t!("A question is being answered; switch when it's done."))
                            .size(size::SM)
                            .color(p.weak),
                    );
                }
            },
        );
        ui.separator();
        ui.add_space(6.0);
        ui.set_max_width(ui.available_width().min(560.0));
        if self.ask.backend == crate::ask::Backend::OpenCode && self.ask.oc.exe.is_none() {
            self.opencode_install_ui(ui);
        } else if self.ask.backend == crate::ask::Backend::OpenCode {
            App::opencode_settings(ui, &mut self.ask);
        } else {
            App::claude_api_settings(ui, &mut self.ask);
        }
        ui.add_space(8.0);
        App::agent_permissions(ui, &mut self.ask);
    }

    fn settings_about(&mut self, ui: &mut egui::Ui) {
        let p = pal(ui);
        ui.horizontal(|ui| {
            chibi(ui, 3, 84.0);
            ui.vertical(|ui| {
                ui.add_space(10.0);
                ui.label(semibold(APP_DISPLAY_VERSION, size::XL));
                ui.label(RichText::new(t!("Your Uma Musume: Pretty Derby (Global) companion. Plan careers on the community meta, race them in a real simulator, and keep your whole collection at hand.")).color(p.weak));
            });
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            web_link(ui, "GitHub", "https://github.com/Nyakij/UmaYu");
            ui.label("·");
            web_link(
                ui,
                t!("Releases and downloads"),
                "https://github.com/Nyakij/UmaYu/releases",
            );
            ui.label("·");
            web_link(
                ui,
                t!("Report a bug or suggest an idea"),
                "https://github.com/Nyakij/UmaYu/issues/new/choose",
            );
        });
        heading(ui, "Credits");
        for (what, who, link) in [
            (
                "Original project",
                "UmaYu is a fork of brickc4's umaLord, the companion app this project builds on",
                "https://github.com/brickc4/umaLord",
            ),
            (
                "Game data",
                "uma.guide: umas, cards, skills, races and events",
                "https://uma.guide",
            ),
            (
                "Race simulator",
                "a port of Moomoolator (TheCing/uma-tools), which builds on alpha123's Umalator and kachi-dev's VFalator",
                "https://umalator.app",
            ),
            (
                "Game audio",
                "third-party crates cridecoder (CRIWARE ACB/AWB/HCA decoding), cpal (WASAPI audio output), rusqlite (SQLite manifest reading), lz4_flex (UnityFS LZ4 decompression)",
                "",
            ),
            (
                "Game audio reference",
                "UmamusumeExplorer by MarshmallowAndroid: inspiration and reference for the game audio concept",
                "https://github.com/MarshmallowAndroid/UmamusumeExplorer",
            ),
            (
                "Meta picks",
                "the community \"PvP Deck & Uma Recommendation\" sheet by Lightning, Daptrius and Henry",
                "",
            ),
            (
                "Skill tiers",
                "the community \"Uma Musume Skills Spreadsheet\" tier list by Dylan, formatted by sayaduck",
                "",
            ),
            (
                "Glossary and tips",
                "GameTora, uma.guide and other community guides, each cited in knowledge/mechanics.md",
                "",
            ),
            ("Chibi art", "GameTora", "https://gametora.com"),
        ] {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(semibold(format!("{}:", t!(what)), size::BODY));
                ui.label(RichText::new(t!(who)).color(p.weak));
                if !link.is_empty() {
                    web_link(ui, link.trim_start_matches("https://"), link);
                }
            });
        }
        heading(ui, "License");
        ui.label(RichText::new(t!("GPL-3.0-or-later: the race simulator ports Moomoolator, which is GPL-3.0-or-later. The source is on GitHub and in the app folder.")).color(p.weak));
        ui.label(RichText::new(t!("Uma Musume: Pretty Derby is © Cygames, Inc. UmaYu is an unofficial fan project, not affiliated with or endorsed by Cygames.")).size(size::SM).color(p.weak));
        heading(ui, "Keyboard shortcuts");
        egui::Grid::new("about_shortcuts")
            .num_columns(2)
            .spacing([24.0, 6.0])
            .show(ui, |ui| {
                for (keys, what) in crate::SHORTCUTS {
                    ui.label(RichText::new(*keys).monospace());
                    ui.label(t!(*what));
                    ui.end_row();
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_keys_round_trip() {
        for p in SettingsPage::ALL {
            assert_eq!(SettingsPage::from_key(p.key()), Some(p));
        }
    }

    /// Every section draws, in both modes, at the smallest window size.
    #[test]
    fn every_section_draws() {
        let mut a = App::for_tests();
        for mode in [theme::Mode::Dark, theme::Mode::Light] {
            let ctx = egui::Context::default();
            theme::install(&ctx, mode);
            for page in SettingsPage::ALL {
                a.open_settings(page);
                for _ in 0..2 {
                    let input = egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(960.0, 600.0))),
                        ..Default::default()
                    };
                    let _ = crate::ui_util::test_frame(&ctx, input, |ui| a.frame(ui));
                }
                assert!(a.settings.open, "{page:?} closed itself");
            }
        }
    }
}
