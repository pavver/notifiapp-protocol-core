use thiserror::Error;

/// Errors that can occur during serialization or deserialization.
#[derive(Debug, Error)]
pub enum CodecError {
    #[error("Serialization failed: {0}")]
    Serialization(String),

    #[error("Deserialization failed: {0}")]
    Deserialization(String),
}

/// A trait defining serialization and deserialization interface for protocol payloads.
pub trait ProtocolCodec: Send + Sync + 'static {
    /// Serialize a value to raw bytes.
    fn serialize<T: serde::Serialize>(val: &T) -> Result<Vec<u8>, CodecError>;

    /// Deserialize a value from raw bytes.
    fn deserialize<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, CodecError>;
}

/// Codec implementing compact binary serialization using Postcard.
pub struct PostcardCodec;

impl ProtocolCodec for PostcardCodec {
    fn serialize<T: serde::Serialize>(val: &T) -> Result<Vec<u8>, CodecError> {
        postcard::to_stdvec(val).map_err(|e| CodecError::Serialization(e.to_string()))
    }

    fn deserialize<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, CodecError> {
        postcard::from_bytes(bytes).map_err(|e| CodecError::Deserialization(e.to_string()))
    }
}

/// Codec implementing readable text serialization using JSON.
pub struct JsonCodec;

impl ProtocolCodec for JsonCodec {
    fn serialize<T: serde::Serialize>(val: &T) -> Result<Vec<u8>, CodecError> {
        serde_json::to_vec(val).map_err(|e| CodecError::Serialization(e.to_string()))
    }

    fn deserialize<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, CodecError> {
        serde_json::from_slice(bytes).map_err(|e| CodecError::Deserialization(e.to_string()))
    }
}
