//! Plan mémoire v1 de la zone partagée (voir `docs/ARCHITECTURE.md` § 3).
//!
//! Tous les champs sont des atomiques : la zone est modifiée par d'autres
//! processus pendant qu'on en détient une référence, ce qui rend toute
//! référence partagée à un champ non atomique formellement indéfinie en Rust.
//! Sur x86-64 et ARM64, un accès atomique `Relaxed` coûte autant qu'un accès normal.

use std::mem::{offset_of, size_of};
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64};

/// `b"DAWSEND\0"` lu en little-endian.
pub const MAGIC: u64 = u64::from_le_bytes(*b"DAWSEND\0");
pub const LAYOUT_VERSION: u32 = 1;

pub const HEADER_SIZE: usize = 256;
pub const SLOT_COUNT: usize = 32;
pub const SLOT_HEADER_SIZE: usize = 256;
pub const MAX_CHANNELS: usize = 2;
/// Capacité d'un ring en trames. Puissance de 2 (le modulo devient un masque).
pub const RING_CAPACITY_FRAMES: usize = 32_768;
pub const NAME_LEN: usize = 64;
/// Début des données, aligné sur une page de 4 Kio.
pub const DATA_OFFSET: usize = 12_288;
pub const RING_SAMPLES: usize = RING_CAPACITY_FRAMES * MAX_CHANNELS;
pub const RING_BYTES: usize = RING_SAMPLES * size_of::<f32>();
pub const TOTAL_SIZE: usize = DATA_OFFSET + SLOT_COUNT * RING_BYTES;

/// Fréquences d'échantillonnage acceptées (tout le reste est considéré comme corrompu).
pub const MIN_SAMPLE_RATE: u32 = 8_000;
pub const MAX_SAMPLE_RATE: u32 = 768_000;

// --- init_state ---
pub const INIT_BLANK: u32 = 0;
pub const INIT_IN_PROGRESS: u32 = 1;
pub const INIT_READY: u32 = 2;

// --- SlotHeader::state ---
pub const SLOT_FREE: u32 = 0;
pub const SLOT_CLAIMING: u32 = 1;
pub const SLOT_ACTIVE: u32 = 2;

// --- SlotHeader::flags ---
pub const SLOT_FLAG_SEND_ENABLED: u32 = 1 << 0;

// --- GlobalHeader::app_flags ---
pub const APP_FLAG_PLAYING: u32 = 1 << 0;

// --- SlotHeader::consumer_flags ---
pub const CONSUMER_FLAG_LISTENING: u32 = 1 << 0;
pub const CONSUMER_FLAG_MUTED: u32 = 1 << 1;

/// En-tête global, offset 0.
#[repr(C, align(64))]
pub struct GlobalHeader {
    pub magic: AtomicU64,
    pub layout_version: AtomicU32,
    pub init_state: AtomicU32,
    pub header_size: AtomicU32,
    pub slot_count: AtomicU32,
    pub slot_header_size: AtomicU32,
    pub max_channels: AtomicU32,
    pub ring_capacity_frames: AtomicU32,
    pub _reserved0: AtomicU32,
    pub data_offset: AtomicU64,
    pub total_size: AtomicU64,
    pub _reserved1: AtomicU64,
    // --- ligne de cache écrite par l'appli ---
    pub app_pid: AtomicU32,
    pub app_flags: AtomicU32,
    pub app_start_time: AtomicU64,
    pub app_heartbeat_ns: AtomicU64,
    pub _reserved2: [AtomicU64; 21],
}

/// En-tête d'un slot (une source). Chaque ligne de cache n'a qu'un écrivain.
#[repr(C, align(64))]
pub struct SlotHeader {
    // --- ligne 0 : métadonnées, protégées par `seq` (écrivain : plugin, hors thread audio) ---
    pub seq: AtomicU32,
    pub state: AtomicU32,
    pub owner_pid: AtomicU32,
    pub sample_rate: AtomicU32,
    pub owner_start_time: AtomicU64,
    pub instance_id: AtomicU64,
    pub channels: AtomicU32,
    pub flags: AtomicU32,
    pub generation: AtomicU32,
    pub _reserved0: [AtomicU32; 5],
    // --- ligne 1 : nom UTF-8 complété par des NUL, protégé par `seq` ---
    pub name: [AtomicU8; NAME_LEN],
    // --- ligne 2 : écrite par le thread audio du plugin ---
    pub write_pos: AtomicU64,
    pub producer_heartbeat_ns: AtomicU64,
    pub _reserved1: [AtomicU64; 6],
    // --- ligne 3 : écrite par l'appli ---
    pub read_pos: AtomicU64,
    pub consumer_heartbeat_ns: AtomicU64,
    pub consumer_flags: AtomicU32,
    pub _reserved2: AtomicU32,
    pub _reserved3: [AtomicU64; 5],
}

// Vérifications à la compilation : le format est un contrat binaire entre processus.
const _: () = {
    assert!(size_of::<GlobalHeader>() == HEADER_SIZE);
    assert!(offset_of!(GlobalHeader, magic) == 0);
    assert!(offset_of!(GlobalHeader, layout_version) == 8);
    assert!(offset_of!(GlobalHeader, init_state) == 12);
    assert!(offset_of!(GlobalHeader, header_size) == 16);
    assert!(offset_of!(GlobalHeader, slot_count) == 20);
    assert!(offset_of!(GlobalHeader, slot_header_size) == 24);
    assert!(offset_of!(GlobalHeader, max_channels) == 28);
    assert!(offset_of!(GlobalHeader, ring_capacity_frames) == 32);
    assert!(offset_of!(GlobalHeader, data_offset) == 40);
    assert!(offset_of!(GlobalHeader, total_size) == 48);
    assert!(offset_of!(GlobalHeader, app_pid) == 64);
    assert!(offset_of!(GlobalHeader, app_flags) == 68);
    assert!(offset_of!(GlobalHeader, app_start_time) == 72);
    assert!(offset_of!(GlobalHeader, app_heartbeat_ns) == 80);

    assert!(size_of::<SlotHeader>() == SLOT_HEADER_SIZE);
    assert!(offset_of!(SlotHeader, seq) == 0);
    assert!(offset_of!(SlotHeader, state) == 4);
    assert!(offset_of!(SlotHeader, owner_pid) == 8);
    assert!(offset_of!(SlotHeader, sample_rate) == 12);
    assert!(offset_of!(SlotHeader, owner_start_time) == 16);
    assert!(offset_of!(SlotHeader, instance_id) == 24);
    assert!(offset_of!(SlotHeader, channels) == 32);
    assert!(offset_of!(SlotHeader, flags) == 36);
    assert!(offset_of!(SlotHeader, generation) == 40);
    assert!(offset_of!(SlotHeader, name) == 64);
    assert!(offset_of!(SlotHeader, write_pos) == 128);
    assert!(offset_of!(SlotHeader, producer_heartbeat_ns) == 136);
    assert!(offset_of!(SlotHeader, read_pos) == 192);
    assert!(offset_of!(SlotHeader, consumer_heartbeat_ns) == 200);
    assert!(offset_of!(SlotHeader, consumer_flags) == 208);

    assert!(HEADER_SIZE + SLOT_COUNT * SLOT_HEADER_SIZE <= DATA_OFFSET);
    assert!(DATA_OFFSET % 4096 == 0);
    assert!(RING_CAPACITY_FRAMES.is_power_of_two());
    assert!(TOTAL_SIZE == 8_400_896);
};

/// Offset (en octets depuis le début de la zone) de l'en-tête du slot `index`.
pub const fn slot_offset(index: usize) -> usize {
    HEADER_SIZE + index * SLOT_HEADER_SIZE
}

/// Offset (en octets depuis le début de la zone) du ring du slot `index`.
pub const fn ring_offset(index: usize) -> usize {
    DATA_OFFSET + index * RING_BYTES
}
