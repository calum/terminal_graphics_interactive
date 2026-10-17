//! Multiplayer: deterministic simulations, lockstep and transports.

mod codec;
mod hash;
mod ids;
mod lockstep;
#[cfg(test)]
mod lossy;
mod setup;
mod sim;
mod tcp;
mod transport;

pub use codec::{Decode, DecodeError, Encode, Reader};
pub use hash::Hasher;
pub use ids::{PeerId, PlayerId};
pub use lockstep::{Config, Desync, Lockstep, Progress};
pub use setup::{announce, await_message};
pub use sim::Sim;
pub use tcp::{Host, Tcp};
pub use transport::{Local, Transport};
