use core::fmt;
use std::{collections::HashMap, error};

use ::serde::Serialize;
use k256::Scalar;

/// Represents an error which can happen when running a protocol.
#[derive(Debug)]
pub enum ProtocolError {
    /// Some assertion in the protocol failed.
    AssertionFailed(String),
    /// Some generic error happened.
    Other(Box<dyn error::Error>),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::Other(e) => write!(f, "{}", e),
            ProtocolError::AssertionFailed(e) => write!(f, "assertion failed {}", e),
        }
    }
}

impl error::Error for ProtocolError {}

impl From<Box<dyn error::Error>> for ProtocolError {
    fn from(e: Box<dyn error::Error>) -> Self {
        Self::Other(e)
    }
}

/// Represents an error which can happen when *initializing* a protocol.
///
/// These are related to bad parameters for the protocol, and things like that.
///
/// These are usually more recoverable than other protocol errors.
#[derive(Debug)]
pub enum InitializationError {
    BadParameters(String),
}

impl fmt::Display for InitializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitializationError::BadParameters(s) => write!(f, "bad parameters: {}", s),
        }
    }
}

impl error::Error for InitializationError {}

/// Represents a participant in the protocol.
///
/// Each participant should be uniquely identified by some number, which this
/// struct holds. In our case, we use a `u32`, which is enough for billions of
/// participants. That said, you won't actually be able to make the protocols
/// work with billions of users.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Hash)]
pub struct Participant(u32);

impl Participant {
    /// Return this participant as little endian bytes.
    pub fn bytes(&self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    /// Return the scalar associated with this participant.
    pub fn scalar(&self) -> Scalar {
        Scalar::from(self.0 as u64 + 1)
    }
}

impl From<Participant> for u32 {
    fn from(p: Participant) -> Self {
        p.0
    }
}

impl From<u32> for Participant {
    fn from(x: u32) -> Self {
        Participant(x)
    }
}

/// Represents the data making up a message.
///
/// We choose to just represent messages as opaque vectors of bytes, with all
/// the serialization logic handled internally.
pub type MessageData = Vec<u8>;

/// Represents an action by a participant in the protocol.
///
/// The basic flow is that each participant receives messages from other participants,
/// and then reacts with some kind of action.
///
/// This action can consist of sending a message, doing nothing, etc.
///
/// Eventually, the participant returns a value, ending the protocol.
#[derive(Debug, Clone)]
pub enum Action<T> {
    /// Don't do anything.
    Wait,
    /// Send a message to all other participants.
    ///
    /// Participants *never* sends messages to themselves.
    SendMany(MessageData),
    /// Send a private message to another participant.
    ///
    /// It's imperactive that only this participant can read this message,
    /// so you might want to use some form of encryption.
    SendPrivate(Participant, MessageData),
    /// End the protocol by returning a value.
    Return(T),
}

/// A trait for protocols.
///
/// Basically, this represents a struct for the behavior of a single participant
/// in a protocol. The idea is that the computation of that participant is driven
/// mainly by receiving messages from other participants.
pub trait Protocol {
    type Output;

    /// Poke the protocol, receiving a new action.
    ///
    /// The idea is that the protocol should be poked until it returns an error,
    /// or it returns an action with a return value, or it returns a wait action.
    ///
    /// Upon returning a wait action, that protocol will not advance any further
    /// until a new message arrives.
    fn poke(&mut self) -> Result<Action<Self::Output>, ProtocolError>;

    /// Inform the protocol of a new message.
    fn message(&mut self, from: Participant, data: MessageData);
}

/// Run a protocol to completion, synchronously.
///
/// This works by executing each participant in order.
pub fn run_protocol<T: std::fmt::Debug>(
    mut ps: Vec<(Participant, Box<dyn Protocol<Output = T>>)>,
) -> Result<Vec<(Participant, T)>, ProtocolError> {
    let indices: HashMap<Participant, usize> =
        ps.iter().enumerate().map(|(i, (p, _))| (*p, i)).collect();

    let size = ps.len();
    let mut out = Vec::with_capacity(size);
    while out.len() < size {
        for i in 0..size {
            while {
                let action = ps[i].1.poke()?;
                match action {
                    Action::Wait => false,
                    Action::SendMany(m) => {
                        for j in 0..size {
                            if i == j {
                                continue;
                            }
                            let from = ps[i].0;
                            ps[j].1.message(from, m.clone());
                        }
                        true
                    }
                    Action::SendPrivate(to, m) => {
                        let from = ps[i].0;
                        ps[indices[&to]].1.message(from, m);
                        true
                    }
                    Action::Return(r) => {
                        out.push((ps[i].0, r));
                        false
                    }
                }
            } {}
        }
    }

    Ok(out)
}

pub(crate) mod internal;
