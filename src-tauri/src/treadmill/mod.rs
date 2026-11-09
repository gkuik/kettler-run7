//! Pilotage du tapis : session Bluetooth (`session`), logique d'état pure
//! (`state`), tapis simulé (`sim`) et profils de course (`profile`).

pub mod profile;
mod session;
pub mod sim;
pub mod state;

pub use session::{StateEmitter, Treadmill};
