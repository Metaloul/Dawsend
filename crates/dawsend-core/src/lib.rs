//! Cœur commun du plugin et de l'appli Dawsend : format de la mémoire partagée,
//! registre des sources et ring SPSC lock-free.
//!
//! Voir `docs/ARCHITECTURE.md` pour la spécification du format.

pub mod clock;
#[cfg(any(test, feature = "fuzzing"))]
pub mod fuzzing;
pub mod layout;
pub mod process;
pub mod region;
pub mod registry;
pub mod ring;
pub mod shm;

pub use region::{OpenError, Region};
pub use registry::{ClaimError, LinkStatus, SlotClaim, SlotMeta};
pub use ring::{Consumer, Producer, ReadError};
