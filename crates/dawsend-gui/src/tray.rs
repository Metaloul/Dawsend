//! Icône et menu de la zone de notification : ouvrir la fenêtre, couper le son, choisir
//! les sources jouées et la sortie, démarrage avec Windows, quitter.
//!
//! Un thread rafraîchit le menu (une fois par seconde, et la liste des sorties toutes les
//! 5 s) seulement quand son contenu change.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use dawsend_app::output;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_autostart::ManagerExt as _;

use crate::{AppState, i18n, quit, set_autostart_inner, show_main};

const TRAY_PNG: &[u8] = include_bytes!("../../../assets/tray-32.png");

/// Sorties listées dans le menu, dans l'ordre des identifiants `dev:<n>`.
static DEVICES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Ce qui détermine le contenu du menu.
#[derive(Clone, PartialEq, Default)]
struct MenuModel {
    lang: &'static str,
    sources: Vec<(String, bool)>,
    master_muted: bool,
    device: Option<String>,
    devices: Vec<String>,
    autostart: bool,
    /// Version disponible, s'il y en a une.
    update: Option<String>,
}

pub fn install(app: &AppHandle, state: Arc<AppState>) -> tauri::Result<()> {
    let model = MenuModel {
        lang: i18n::resolve(&state.prefs().language),
        ..MenuModel::default()
    };
    let menu = build_menu(app, &model)?;
    let tray = TrayIconBuilder::with_id("dawsend")
        .icon(Image::from_bytes(TRAY_PNG)?)
        .tooltip(i18n::texts(model.lang).tip_waiting)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    log::info!("icône de la zone de notification installée");

    let app = app.clone();
    std::thread::Builder::new()
        .name("dawsend-tray".into())
        .spawn(move || refresh_loop(app, state, tray, model))?;
    Ok(())
}

fn refresh_loop(app: AppHandle, state: Arc<AppState>, tray: TrayIcon<Wry>, mut current: MenuModel) {
    let mut devices: Vec<String> = Vec::new();
    let mut autostart = false;
    let mut next_slow = Instant::now();
    let mut tooltip = String::new();
    loop {
        if Instant::now() >= next_slow {
            next_slow = Instant::now() + Duration::from_secs(5);
            match output::list_devices() {
                Ok(list) => devices = list.into_iter().map(|(n, _)| n).collect(),
                Err(e) => log::warn!("liste des sorties impossible : {e}"),
            }
            autostart = app.autolaunch().is_enabled().unwrap_or(false);
        }
        let lang = i18n::resolve(&state.prefs().language);
        let t = i18n::texts(lang);
        let settings = state.engine_settings().unwrap_or_default();
        let view = state.engine_view().unwrap_or_default();
        let model = MenuModel {
            lang,
            sources: view
                .sources
                .iter()
                .map(|s| (s.name.clone(), s.enabled))
                .collect(),
            master_muted: settings.master_muted,
            device: settings.device.clone(),
            devices: devices.clone(),
            autostart,
            update: state.updates.status().available.map(|u| u.version),
        };
        if model != current {
            match build_menu(&app, &model) {
                Ok(menu) => {
                    if let Err(e) = tray.set_menu(Some(menu)) {
                        log::warn!("mise à jour du menu impossible : {e}");
                    } else {
                        log::debug!("menu de la zone de notification mis à jour");
                        current = model;
                    }
                }
                Err(e) => log::warn!("construction du menu impossible : {e}"),
            }
        }

        let playing: Vec<&str> = view
            .sources
            .iter()
            .filter(|s| s.playing)
            .map(|s| s.name.as_str())
            .collect();
        let tip = if state.engine_settings().is_none()
            || (view.output.is_none() && view.output_error.is_some())
        {
            t.tip_error.to_owned()
        } else if settings.master_muted {
            t.tip_muted.to_owned()
        } else if playing.is_empty() {
            t.tip_waiting.to_owned()
        } else {
            format!("{} : {}", t.tip_playing, playing.join(", "))
        };
        if tip != tooltip {
            let _ = tray.set_tooltip(Some(&tip));
            tooltip = tip;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn build_menu(app: &AppHandle, m: &MenuModel) -> tauri::Result<Menu<Wry>> {
    let t = i18n::texts(m.lang);
    let open = MenuItem::with_id(app, "open", t.open, true, None::<&str>)?;
    let mute = CheckMenuItem::with_id(app, "mute", t.mute_all, true, m.master_muted, None::<&str>)?;

    let sources = Submenu::with_id(app, "sources", t.sources, true)?;
    if m.sources.is_empty() {
        sources.append(&MenuItem::with_id(
            app,
            "none",
            t.no_source,
            false,
            None::<&str>,
        )?)?;
    }
    for (name, enabled) in &m.sources {
        sources.append(&CheckMenuItem::with_id(
            app,
            format!("src:{name}"),
            name,
            true,
            *enabled,
            None::<&str>,
        )?)?;
    }

    let outputs = Submenu::with_id(app, "outputs", t.output, true)?;
    outputs.append(&CheckMenuItem::with_id(
        app,
        "dev:default",
        t.default_output,
        true,
        m.device.is_none(),
        None::<&str>,
    )?)?;
    outputs.append(&PredefinedMenuItem::separator(app)?)?;
    for (i, name) in m.devices.iter().enumerate() {
        outputs.append(&CheckMenuItem::with_id(
            app,
            format!("dev:{i}"),
            name,
            true,
            m.device.as_deref() == Some(name.as_str()),
            None::<&str>,
        )?)?;
    }
    *DEVICES.lock().unwrap_or_else(PoisonError::into_inner) = m.devices.clone();

    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        t.autostart,
        true,
        m.autostart,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", t.quit, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &PredefinedMenuItem::separator(app)?,
            &mute,
            &sources,
            &outputs,
            &PredefinedMenuItem::separator(app)?,
            &autostart,
            &quit,
        ],
    )?;
    if let Some(v) = &m.update {
        let text = t.update_available.replace("{v}", v);
        menu.insert(
            &MenuItem::with_id(app, "update", text, true, None::<&str>)?,
            1,
        )?;
    }
    Ok(menu)
}

fn on_menu(app: &AppHandle, id: &str) {
    log::debug!("menu de la zone de notification : {id}");
    let Some(state) = app.try_state::<Arc<AppState>>() else {
        return;
    };
    let result = match id {
        "open" | "update" => {
            show_main(app);
            Ok(())
        }
        "quit" => {
            quit(app);
            Ok(())
        }
        "mute" => state.update_engine(|s| {
            s.master_muted = !s.master_muted;
            log::info!("menu : son coupé {}", s.master_muted);
        }),
        "autostart" => {
            // Registre : hors du thread principal (celui de l'interface).
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let on = app.autolaunch().is_enabled().unwrap_or(false);
                if let Err(e) = set_autostart_inner(&app, !on) {
                    log::warn!("menu « autostart » : {e}");
                }
            });
            Ok(())
        }
        "dev:default" => state.update_engine(|s| {
            log::info!("menu : sortie de Windows par défaut");
            s.device = None;
        }),
        _ => {
            if let Some(name) = id.strip_prefix("src:") {
                state.update_engine(|s| {
                    let mut src = s.source(name);
                    src.enabled = !src.enabled;
                    log::info!("menu : source « {name} » active {}", src.enabled);
                    s.sources.insert(name.to_owned(), src);
                })
            } else if let Some(i) = id
                .strip_prefix("dev:")
                .and_then(|n| n.parse::<usize>().ok())
            {
                let name = DEVICES
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get(i)
                    .cloned();
                match name {
                    Some(name) => state.update_engine(|s| {
                        log::info!("menu : sortie « {name} »");
                        s.device = Some(name);
                    }),
                    None => Ok(()),
                }
            } else {
                Ok(())
            }
        }
    };
    if let Err(e) = result {
        log::warn!("menu « {id} » : {e}");
    }
}
