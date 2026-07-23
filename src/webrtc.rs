use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of media track in a WebRTC session.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum TrackType {
    /// Audio track (microphone, system audio).
    Audio,
    /// Video track (camera, screen share).
    Video,
}

/// Metadata describing a single media track.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TrackInfo {
    /// Unique identifier for the track.
    pub track_id: String,
    /// Type of media (Audio or Video).
    pub track_type: TrackType,
    /// Mute status of the track.
    pub is_muted: bool,
}

/// The state of a participant in a WebRTC room.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ParticipantState {
    /// Unique identifier of the user.
    pub user_id: Uuid,
    /// Type of device used by the participant (e.g. "Android", "Web", "Desktop").
    pub device_type: String,
    /// List of media tracks published by the participant.
    pub tracks: Vec<TrackInfo>,
}

/// Overall state of a WebRTC room.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RoomState {
    /// Unique identifier of the WebRTC room.
    pub room_id: Uuid,
    /// List of participants currently in the room.
    pub participants: Vec<ParticipantState>,
}

/// Payload containing WebRTC signaling data.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum SignalPayload {
    /// SDP Offer description string.
    Offer(String),
    /// SDP Answer description string.
    Answer(String),
    /// Serialization of an ICE Candidate.
    IceCandidate(String),
    /// Request to initiate WebRTC renegotiation (re-invite).
    Renegotiate,
}

/// A container for WebRTC signaling messages routed between participants.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct WebRtcSignal {
    /// Target user ID to route the signaling message to.
    pub target_user_id: Uuid,
    /// The signaling payload (SDP/ICE).
    pub payload: SignalPayload,
}
