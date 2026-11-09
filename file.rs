use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct SecurityEvent {
    event_type: String,
    severity: u8,
    source_ip: String,
}