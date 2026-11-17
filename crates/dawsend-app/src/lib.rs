//! Appli Dawsend : moteur (indépendant de la plateforme) et sortie audio (isolée par
//! plateforme). Les binaires (console, puis interface Tauri) s'appuient sur cette bibliothèque.

pub mod engine;
pub mod output;
pub mod service;
