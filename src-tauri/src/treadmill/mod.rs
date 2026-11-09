//! Pilotage du tapis : session Bluetooth (`session`) et logique d'état pure (`state`).

mod session;
pub mod state;

pub use session::Treadmill;
