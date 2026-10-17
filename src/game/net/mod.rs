//! Multiplayer: deterministic simulations, lockstep and transports.

mod ids;
#[cfg(test)]
mod lossy;
mod tcp;
mod transport;

pub use ids::{PeerId, PlayerId};
pub use tcp::{Host, Tcp};
pub use transport::{Local, Transport};
