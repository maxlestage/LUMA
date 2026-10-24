//! Le site de LÜMA, écrit avec Paquets : les textes et les îlots partagés par le serveur
//! et le navigateur, le serveur (natif) et le bundle du navigateur (wasm32).

pub mod content;
pub mod islands;

#[cfg(not(target_arch = "wasm32"))]
pub mod server;

#[cfg(target_arch = "wasm32")]
mod client;
