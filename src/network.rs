//! Network module for collaborative pattern editing.
//!
//! This module provides shared types for client-server communication.

use serde::{Deserialize, Serialize};

/// Messages sent from client to server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Client sends their code update to the server.
    CodeUpdate { code: String },
}

/// Messages sent from server to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Server sends an individual client's update to all clients.
    ClientUpdate { client_ip: String, code: String },
    /// Server notifies that a client disconnected.
    ClientDisconnected { client_ip: String },
}
