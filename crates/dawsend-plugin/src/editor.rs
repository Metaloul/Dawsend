//! Interface du plugin (egui), façon façade d'appareil : bandeau avec le logo, nom de la
//! source, voyant de liaison avec l'appli, commutateur Envoi et vumètre à segments.
//! Maquette : page « V2 — Interface » du fichier Figma, cadres « Plugin — … ».
//!
//! Tourne sur le thread de l'interface du DAW : jamais sur le thread audio.

use std::sync::Arc;

use nih_plug::prelude::*;
use nih_plug_egui::egui::text::{LayoutJob, TextFormat};
use nih_plug_egui::egui::{
    self, Align2, Color32, ColorImage, CornerRadius, CursorIcon, FontData, FontDefinitions,
    FontFamily, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
    WidgetInfo, WidgetType, pos2, vec2,
};
use nih_plug_egui::{EguiState, create_egui_editor};

use crate::i18n::{self, Texts};
use crate::{DawsendParams, LinkView, Shared};
use dawsend_core::LinkStatus;

const WIDTH: u32 = 380;
const HEIGHT: u32 = 236;

/// Symbole et logotype exportés de la maquette Figma en 3× (314 × 60), fond transparent.
const HEADER_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/plugin-header.png"
));

/// Marque GitHub (Octicons, licence MIT) en blanc, 48 × 48, teintée au dessin.
const GITHUB_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/github-mark.png"
));

/// Polices de l'interface (licence OFL, voir `assets/fonts/`).
const FONT_SANS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/FamiljenGrotesk-Regular.ttf"
));
const FONT_SANS_SEMIBOLD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/FamiljenGrotesk-SemiBold.ttf"
));
const FONT_MONO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/GeistMono-Regular.ttf"
));

/// Palette V2 (voir `assets/palette.md`).
mod theme {
    use super::Color32;
    /// Fond de la fenêtre.
    pub const INK: Color32 = Color32::from_rgb(0x12, 0x14, 0x18);
    /// Façade (bandeau).
    pub const FACE: Color32 = Color32::from_rgb(0x18, 0x1B, 0x20);
    /// Creux (champ de saisie, fond du commutateur).
    pub const WELL: Color32 = Color32::from_rgb(0x0C, 0x0E, 0x11);
    /// Commandes.
    pub const CTRL: Color32 = Color32::from_rgb(0x2A, 0x2E, 0x36);
    /// Filets.
    pub const LINE: Color32 = Color32::from_rgb(0x26, 0x2A, 0x31);
    /// Segment éteint.
    pub const UNLIT: Color32 = Color32::from_rgb(0x1E, 0x22, 0x28);
    pub const PAPER: Color32 = Color32::from_rgb(0xF3, 0xEF, 0xE8);
    pub const MUTED: Color32 = Color32::from_rgb(0x9A, 0x9F, 0xAA);
    pub const DIM: Color32 = Color32::from_rgb(0x6B, 0x72, 0x80);
    pub const AMBER: Color32 = Color32::from_rgb(0xFF, 0x7A, 0x2F);
    pub const OK: Color32 = Color32::from_rgb(0x3D, 0xD6, 0x8C);
    pub const WARN: Color32 = Color32::from_rgb(0xF2, 0xB8, 0x3B);
    pub const BAD: Color32 = Color32::from_rgb(0xF2, 0x55, 0x55);
}

/// Famille de la graisse demi-grasse (titres, nom de la source, étiquettes).
fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(WIDTH, HEIGHT)
}

struct UiState {
    params: Arc<DawsendParams>,
    shared: Arc<Shared>,
    texts: &'static Texts,
    header: Option<TextureHandle>,
    github: Option<TextureHandle>,
    name_buf: String,
    editing: bool,
    meter: [f32; 2],
}

pub fn create(params: Arc<DawsendParams>, shared: Arc<Shared>) -> Option<Box<dyn Editor>> {
    let state = UiState {
        params: params.clone(),
        shared,
        texts: i18n::system(),
        header: None,
        github: None,
        name_buf: String::new(),
        editing: false,
        meter: [0.0; 2],
    };
    create_egui_editor(
        params.editor_state.clone(),
        state,
        |ctx, st| {
            ctx.set_fonts(fonts());
            let mut style = (*ctx.style()).clone();
            style.visuals = egui::Visuals::dark();
            style.visuals.panel_fill = theme::INK;
            style.visuals.extreme_bg_color = theme::WELL;
            style.visuals.selection.bg_fill = theme::AMBER.gamma_multiply(0.45);
            style.visuals.selection.stroke = Stroke::new(1.0_f32, theme::PAPER);
            style.visuals.text_cursor.stroke = Stroke::new(2.0_f32, theme::AMBER);
            ctx.set_style(style);
            st.header = decode_png(HEADER_PNG)
                .map(|img| ctx.load_texture("dawsend-header", img, TextureOptions::LINEAR));
            st.github = decode_png(GITHUB_PNG)
                .map(|img| ctx.load_texture("dawsend-github", img, TextureOptions::LINEAR));
        },
        draw,
    )
}

/// Familjen Grotesk pour le texte, Geist Mono pour les chiffres ; les polices d'egui
/// restent en secours pour les caractères absents.
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let add = |fonts: &mut FontDefinitions, name: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add(&mut fonts, "familjen", FONT_SANS);
    add(&mut fonts, "familjen-semibold", FONT_SANS_SEMIBOLD);
    add(&mut fonts, "geist-mono", FONT_MONO);

    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
        list.insert(0, "familjen".to_owned());
    }
    if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
        list.insert(0, "geist-mono".to_owned());
    }
    let mut semi = vec!["familjen-semibold".to_owned()];
    semi.extend(fallback);
    fonts
        .families
        .insert(FontFamily::Name("semibold".into()), semi);
    fonts
}

fn decode_png(bytes: &[u8]) -> Option<ColorImage> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf[..w * h * 4].to_vec(),
        png::ColorType::Rgb => buf[..w * h * 3]
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        _ => return None,
    };
    Some(ColorImage::from_rgba_unmultiplied([w, h], &rgba))
}

/// État affiché par le voyant de gauche : couleur, titre, explication.
struct Status {
    color: Color32,
    title: &'static str,
    hint: String,
}

fn status(st: &UiState, send: bool) -> Status {
    let t = st.texts;
    let s = |color, title, hint: &str| Status {
        color,
        title,
        hint: hint.to_owned(),
    };
    if !send {
        return s(theme::DIM, t.send_off, t.send_off_hint);
    }
    match st.shared.link() {
        LinkView::Link(LinkStatus::Listening) if st.shared.muted_in_app() => {
            s(theme::WARN, t.listening_muted, t.listening_muted_hint)
        }
        LinkView::Link(LinkStatus::Listening) => s(theme::OK, t.listening, t.listening_hint),
        LinkView::Link(LinkStatus::AppConnected) => s(theme::WARN, t.connected, t.connected_hint),
        LinkView::Link(LinkStatus::AppAbsent) => s(theme::BAD, t.app_absent, t.app_absent_hint),
        LinkView::Unavailable(e) => Status {
            color: theme::BAD,
            title: t.unavailable,
            hint: e,
        },
        LinkView::NotActive => s(theme::DIM, t.not_active, t.not_active_hint),
    }
}

fn draw(ctx: &egui::Context, setter: &ParamSetter, st: &mut UiState) {
    // Vumètre : attaque immédiate, retombée de 24 dB/s.
    let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
    let fall = 10f32.powf(-24.0 * dt / 20.0);
    let peaks = st.shared.take_peaks();
    for (m, p) in st.meter.iter_mut().zip(peaks) {
        *m = p.max(*m * fall);
    }
    if !st.editing {
        st.name_buf = st.params.wanted_name();
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::default().fill(theme::INK))
        .show(ctx, |ui| {
            let full = ui.max_rect();
            let o = full.min;
            let at = |x: f32, y: f32| o + vec2(x, y);
            let send = st.params.send.value();

            header(ui, st, Rect::from_min_size(o, vec2(full.width(), 41.0)));

            // Colonne de gauche : nom de la source et état de la liaison.
            let left = 14.0;
            let col_w = 282.0;
            caps(
                ui.painter(),
                at(left, 53.0),
                Align2::LEFT_TOP,
                st.texts.source_name,
            );
            name_field(
                ui,
                st,
                Rect::from_min_size(at(left, 72.0), vec2(col_w, 35.0)),
            );

            let s = status(st, send);
            led(ui.painter(), at(left + 5.0, 125.0), s.color);
            ui.painter().text(
                at(left + 17.0, 125.0),
                Align2::LEFT_CENTER,
                s.title,
                semibold(13.0),
                theme::PAPER,
            );
            let hint = ui
                .painter()
                .layout(s.hint, FontId::proportional(12.0), theme::MUTED, col_w);
            ui.painter().galley(at(left, 139.0), hint, theme::MUTED);

            // Colonne de droite : commutateur Envoi.
            let cx = 338.0;
            led(
                ui.painter(),
                at(cx, 60.0),
                if send { theme::OK } else { theme::UNLIT },
            );
            footswitch(ui, setter, st, send, at(cx, 99.0));
            caps(
                ui.painter(),
                at(cx, 133.0),
                Align2::CENTER_TOP,
                st.texts.send,
            );

            // Bas : vumètre stéréo.
            let mono = FontId::monospace(8.0);
            ui.painter().text(
                at(left, 207.0),
                Align2::LEFT_CENTER,
                "L",
                mono.clone(),
                theme::DIM,
            );
            ui.painter()
                .text(at(left, 215.0), Align2::LEFT_CENTER, "R", mono, theme::DIM);
            meter(
                ui.painter(),
                Rect::from_min_max(at(27.0, 204.5), at(366.0, 217.5)),
                st.meter,
                send,
            );
        });
    // 30 images par seconde suffisent au vumètre et au voyant, et ménagent le DAW.
    ctx.request_repaint_after(std::time::Duration::from_millis(33));
}

/// Étiquette en petites capitales espacées (« SOURCE », « ENVOI »).
fn caps(p: &egui::Painter, pos: Pos2, align: Align2, text: &str) {
    let job = LayoutJob::single_section(
        text.to_uppercase(),
        TextFormat {
            font_id: semibold(10.0),
            color: theme::MUTED,
            extra_letter_spacing: 0.8,
            ..Default::default()
        },
    );
    let galley = p.ctx().fonts(|f| f.layout_job(job));
    let rect = align.anchor_size(pos, galley.size());
    p.galley(rect.min, galley, theme::MUTED);
}

/// Bandeau : symbole et logotype à gauche, version et lien vers GitHub à droite, filet en
/// bas.
fn header(ui: &mut egui::Ui, st: &UiState, rect: Rect) {
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, theme::FACE);
    p.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0_f32, theme::LINE),
    );
    match &st.header {
        Some(tex) => {
            let size = tex.size_vec2();
            let h = 20.0;
            let logo =
                Rect::from_min_size(rect.min + vec2(12.0, 10.0), vec2(size.x * h / size.y, h));
            p.image(
                tex.id(),
                logo,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        None => {
            p.text(
                rect.left_center() + vec2(12.0, 0.0),
                Align2::LEFT_CENTER,
                "Dawsend",
                semibold(17.0),
                theme::PAPER,
            );
        }
    }
    let version_right = match &st.github {
        Some(tex) => {
            github_link(ui, st, tex, rect);
            rect.right() - 12.0 - 16.0 - 10.0
        }
        None => rect.right() - 12.0,
    };
    ui.painter().text(
        pos2(version_right, rect.center().y),
        Align2::RIGHT_CENTER,
        concat!("v", env!("CARGO_PKG_VERSION")),
        FontId::monospace(11.0),
        theme::DIM,
    );
}

/// Marque GitHub cliquable : ouvre la page du projet dans le navigateur. egui-baseview
/// lance le navigateur sans attendre (`open::that_detached`) : rien ne bloque le DAW.
fn github_link(ui: &mut egui::Ui, st: &UiState, tex: &TextureHandle, band: Rect) {
    let icon = Rect::from_center_size(
        pos2(band.right() - 12.0 - 8.0, band.center().y),
        vec2(16.0, 16.0),
    );
    let resp = ui
        .interact(icon.expand(4.0), ui.id().with("github"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(st.texts.github);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Link, true, st.texts.github));
    let tint = if resp.hovered() || resp.has_focus() {
        theme::PAPER
    } else {
        theme::MUTED
    };
    ui.painter().image(
        tex.id(),
        icon,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        tint,
    );
    if resp.clicked() {
        ui.ctx()
            .open_url(egui::OpenUrl::new_tab(env!("CARGO_PKG_REPOSITORY")));
    }
}

/// Champ du nom : creux sombre, filet ambre au survol et pendant la saisie.
fn name_field(ui: &mut egui::Ui, st: &mut UiState, rect: Rect) {
    let id = egui::Id::new("dawsend-source-name");
    let focused = ui.memory(|m| m.has_focus(id));
    let hovered = ui.rect_contains_pointer(rect);
    let border = if focused {
        theme::AMBER
    } else if hovered {
        theme::DIM
    } else {
        theme::LINE
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(6), theme::WELL);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );

    let resp = ui.put(
        rect.shrink2(vec2(11.0, 7.0)),
        egui::TextEdit::singleline(&mut st.name_buf)
            .id(id)
            .frame(false)
            .char_limit(48)
            .font(semibold(15.0))
            .text_color(theme::PAPER)
            .desired_width(f32::INFINITY),
    );
    if resp.gained_focus() {
        st.editing = true;
    }
    if resp.lost_focus() {
        st.editing = false;
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if !escape {
            st.params.rename(&st.shared, &st.name_buf);
        }
    }
    resp.on_hover_text(st.texts.rename_hint);
}

/// Commutateur au pied : bague ambre quand l'envoi est actif. Clic, Espace ou Entrée.
fn footswitch(ui: &mut egui::Ui, setter: &ParamSetter, st: &UiState, send: bool, center: Pos2) {
    let rect = Rect::from_center_size(center, Vec2::splat(56.0));
    let resp = ui
        .interact(rect, egui::Id::new("dawsend-send"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let label = st.texts.send;
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, send, label));

    let p = ui.painter();
    let ring = if send {
        theme::AMBER
    } else if resp.hovered() {
        theme::DIM
    } else {
        theme::CTRL
    };
    p.circle_filled(center, 28.0, theme::WELL);
    p.circle_stroke(center, 25.5, Stroke::new(3.0_f32, ring));
    // Capuchon : deux disques décalés imitent l'éclairage par le haut.
    let pressed = resp.is_pointer_button_down_on();
    let cap = if pressed { 19.0 } else { 20.0 };
    p.circle_filled(center, cap + 1.0, Color32::from_rgb(0x3A, 0x3F, 0x48));
    p.circle_filled(
        center + vec2(0.0, 1.5),
        cap,
        Color32::from_rgb(0x1F, 0x23, 0x29),
    );
    p.circle_stroke(
        center,
        cap + 1.0,
        Stroke::new(1.0_f32, Color32::from_white_alpha(14)),
    );
    if resp.has_focus() {
        p.circle_stroke(center, 29.5, Stroke::new(1.0_f32, theme::PAPER));
    }

    let resp = resp.on_hover_text(st.texts.send_hint);
    if resp.clicked() {
        setter.begin_set_parameter(&st.params.send);
        setter.set_parameter(&st.params.send, !send);
        setter.end_set_parameter(&st.params.send);
    }
}

/// Voyant de 10 px avec un halo de sa couleur (sauf éteint).
fn led(p: &egui::Painter, center: Pos2, color: Color32) {
    if color != theme::UNLIT && color != theme::DIM {
        p.circle_filled(center, 8.0, color.gamma_multiply(0.16));
    }
    p.circle_filled(center, 5.0, color);
    if color == theme::UNLIT {
        p.circle_stroke(center, 5.0, Stroke::new(1.0_f32, theme::LINE));
    }
}

/// Deux rangées de segments de 3 px au pas de 4 px, de -60 à 0 dBFS : vert jusqu'à -9,
/// ambre jusqu'à -1, rouge au-dessus. Gris quand l'envoi est coupé.
fn meter(p: &egui::Painter, rect: Rect, level: [f32; 2], active: bool) {
    const SEG: f32 = 3.0;
    const PITCH: f32 = 4.0;
    const BAR: f32 = 5.0;
    let count = ((rect.width() + PITCH - SEG) / PITCH).floor().max(1.0) as usize;
    for (c, v) in level.into_iter().enumerate() {
        let y = rect.top() + c as f32 * (BAR + 3.0);
        let db = if v > 0.0 { 20.0 * v.log10() } else { -120.0 };
        let frac = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let lit = (frac * count as f32).round() as usize;
        for i in 0..count {
            let seg_db = -60.0 + 60.0 * (i as f32 + 0.5) / count as f32;
            let color = if i >= lit {
                theme::UNLIT
            } else if !active {
                theme::DIM
            } else if seg_db > -1.0 {
                theme::BAD
            } else if seg_db > -9.0 {
                theme::WARN
            } else {
                theme::OK
            };
            let x = rect.left() + i as f32 * PITCH;
            p.rect_filled(
                Rect::from_min_size(pos2(x, y), vec2(SEG, BAR)),
                CornerRadius::ZERO,
                color,
            );
        }
    }
}
