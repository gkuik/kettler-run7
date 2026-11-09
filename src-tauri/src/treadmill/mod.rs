//! Pilotage du tapis : session Bluetooth (`session`), logique d'état pure
//! (`state`) et tapis simulé (`sim`).

mod session;
pub mod sim;
pub mod state;

pub use session::{StateEmitter, Treadmill};
