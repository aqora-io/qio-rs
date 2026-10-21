use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::io::{Read, Write};

type JsonMap = serde_json::Map<String, serde_json::Value>;

pub type Result<T, E = CompressionError> = std::result::Result<T, E>;

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, PartialEq)]
#[repr(u8)]
pub enum CompressionFormat {
    UnknownCompressionFormat = 0,
    None = 1,
    ZlibBase64V1 = 2,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, PartialEq)]
#[repr(u16)]
pub enum QuantumProgramSerializationFormat {
    UnknownSerializationFormat = 0,
    QasmV1 = 1,
    QasmV2 = 2,
    QasmV3 = 3,
    QirV1 = 4,
    CirqCircuitJsonV1 = 5,
    PercevalCircuitJsonV1 = 6,
    PulserSequenceJsonV1 = 7,
    // 1000+ is the Aqora-custom range; values below are reserved for the spec.
    TketCircuitJsonV1 = 1000,
    HugrV1 = 1001,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, PartialEq)]
#[repr(u8)]
pub enum QuantumNoiseModelSerializationFormat {
    UnknownSerializationFormat = 0,
    QiskitAerJsonV1 = 1,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr, Eq, PartialEq)]
#[repr(u16)]
pub enum QuantumProgramResultSerializationFormat {
    UnknownSerializationFormat = 0,
    CirqResultJsonV1 = 1,
    QiskitResultJsonV1 = 2,
    CudaqSampleResultJsonV1 = 3,
    /// QPerfect MIMIQ `QCSResults` as JSON.
    MimiqQcsrJsonV1 = 4,
    // 1000+ is the Aqora-custom range; values below are reserved for the spec.
    PytketBackendResultJsonV1 = 1000,
    /// Quantinuum's labeled QIR results text schema (`HEADER\tschema_id\tlabeled`).
    QirLabeledResultV1 = 1001,
    /// Quantinuum's QSYS shot results (hugr `QsysResult`) as a JSON array.
    QsysResultJsonV1 = 1002,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Serialization<T> {
    pub compression_format: CompressionFormat,
    pub serialization_format: T,
    pub serialization: String,
}

#[derive(Debug)]
pub enum CompressionError {
    UnknownCompressionFormat(CompressionFormat),
    UnsupportedCompressionConversion {
        from: CompressionFormat,
        to: CompressionFormat,
    },
    Base64(base64::DecodeError),
    Io(std::io::Error),
    Utf8(std::string::FromUtf8Error),
}

impl std::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownCompressionFormat(format) => {
                write!(f, "unknown compression format: {format:?}")
            }
            Self::UnsupportedCompressionConversion { from, to } => {
                write!(f, "unsupported compression conversion: {from:?} -> {to:?}")
            }
            Self::Base64(err) => write!(f, "invalid base64 payload: {err}"),
            Self::Io(err) => write!(f, "compression I/O error: {err}"),
            Self::Utf8(err) => write!(f, "decompressed payload is not valid UTF-8: {err}"),
        }
    }
}

impl std::error::Error for CompressionError {}

impl From<base64::DecodeError> for CompressionError {
    fn from(err: base64::DecodeError) -> Self {
        Self::Base64(err)
    }
}

impl From<std::io::Error> for CompressionError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<std::string::FromUtf8Error> for CompressionError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::Utf8(err)
    }
}

impl<T> Serialization<T> {
    #[inline]
    pub fn new(value: impl Into<String>, format: T) -> Self {
        Self {
            serialization_format: format,
            compression_format: CompressionFormat::None,
            serialization: value.into(),
        }
    }

    pub fn to_compression(&self, compression: CompressionFormat) -> Result<Self>
    where
        T: Clone,
    {
        let Self {
            compression_format,
            serialization_format,
            serialization,
        } = self;

        let serialization = match (compression_format, compression) {
            (CompressionFormat::None, CompressionFormat::None)
            | (CompressionFormat::ZlibBase64V1, CompressionFormat::ZlibBase64V1) => {
                serialization.clone()
            }
            (CompressionFormat::None, CompressionFormat::ZlibBase64V1) => {
                compress_zlib_base64(serialization)?
            }
            (CompressionFormat::ZlibBase64V1, CompressionFormat::None) => {
                decompress_zlib_base64(serialization)?
            }
            (CompressionFormat::UnknownCompressionFormat, _)
            | (_, CompressionFormat::UnknownCompressionFormat) => {
                return Err(CompressionError::UnknownCompressionFormat(
                    CompressionFormat::UnknownCompressionFormat,
                ));
            }
        };

        Ok(Self {
            compression_format: compression,
            serialization_format: serialization_format.clone(),
            serialization,
        })
    }
}

pub type QuantumProgram = Serialization<QuantumProgramSerializationFormat>;
pub type QuantumNoiseModel = Serialization<QuantumNoiseModelSerializationFormat>;
pub type QuantumProgramResult = Serialization<QuantumProgramResultSerializationFormat>;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ClientData {
    pub user_agent: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct BackendData {
    pub name: String,
    pub version: Option<String>,
    pub options: Option<JsonMap>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct QuantumComputationModel {
    pub programs: Vec<QuantumProgram>,
    pub noise_model: Option<QuantumNoiseModel>,
    pub client: Option<ClientData>,
    pub backend: Option<BackendData>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct QuantumComputationParameters {
    pub shots: u64,
    pub options: Option<JsonMap>,
}

fn compress_zlib_base64(data: &str) -> Result<String> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data.as_bytes())?;
    Ok(BASE64.encode(encoder.finish()?))
}

fn decompress_zlib_base64(data: &str) -> Result<String> {
    let compressed = BASE64.decode(data)?;
    let mut decoder = ZlibDecoder::new(compressed.as_slice());
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(String::from_utf8(decompressed)?)
}

const MAX_JSON_NESTING_DEPTH: usize = 8;

/// Parse a JSON payload that may be wrapped in nested JSON string encodings
/// (like qio's `from_json_str`), e.g. into a `Serialization<u16>` when the
/// serialization format must tolerate discriminants unknown to this crate.
pub fn parse_json_str<T>(data: &str) -> Result<T, serde_json::Error>
where
    T: DeserializeOwned,
{
    let mut value = serde_json::from_str::<serde_json::Value>(data)?;
    for _ in 0..MAX_JSON_NESTING_DEPTH {
        match value.as_str() {
            Some(inner) => value = serde_json::from_str(inner)?,
            None => break,
        }
    }
    serde_json::from_value(value)
}

macro_rules! impl_json_helpers {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl $ty {
                pub fn from_json_str(data: &str) -> Result<Self, serde_json::Error> {
                    parse_json_str(data)
                }

                pub fn to_json_str(&self) -> Result<String, serde_json::Error> {
                    serde_json::to_string(self)
                }
            }
        )+
    };
}

impl_json_helpers!(
    QuantumProgram,
    QuantumNoiseModel,
    QuantumComputationModel,
    QuantumComputationParameters,
    QuantumProgramResult,
);

#[cfg(test)]
mod tests {
    use super::*;

    type TestSerialization = Serialization<QuantumProgramSerializationFormat>;

    #[test]
    fn converts_between_none_and_zlib_base64() {
        let plain = TestSerialization {
            compression_format: CompressionFormat::None,
            serialization_format: QuantumProgramSerializationFormat::QasmV3,
            serialization: "OPENQASM 3.0;".to_string(),
        };

        let compressed = plain
            .clone()
            .to_compression(CompressionFormat::ZlibBase64V1)
            .unwrap();
        assert_eq!(
            compressed.compression_format,
            CompressionFormat::ZlibBase64V1
        );
        assert_ne!(compressed.serialization, plain.serialization);

        let decompressed = compressed.to_compression(CompressionFormat::None).unwrap();
        assert_eq!(decompressed, plain);
    }

    #[test]
    fn converts_tket_circuit_between_none_and_zlib_base64() {
        let plain = TestSerialization {
            compression_format: CompressionFormat::None,
            serialization_format: QuantumProgramSerializationFormat::TketCircuitJsonV1,
            serialization: r#"{"bits":[],"qubits":[],"commands":[]}"#.to_string(),
        };

        let compressed = plain
            .clone()
            .to_compression(CompressionFormat::ZlibBase64V1)
            .unwrap();
        let decompressed = compressed.to_compression(CompressionFormat::None).unwrap();
        assert_eq!(decompressed, plain);
    }

    #[test]
    fn serializes_new_format_discriminants() {
        // These discriminants are an append-only wire contract; lock them.
        assert_eq!(
            serde_json::to_string(&QuantumProgramSerializationFormat::QirV1).unwrap(),
            "4"
        );
        assert_eq!(
            serde_json::to_string(&QuantumProgramSerializationFormat::TketCircuitJsonV1).unwrap(),
            "1000"
        );
        assert_eq!(
            serde_json::to_string(&QuantumProgramSerializationFormat::HugrV1).unwrap(),
            "1001"
        );
        assert_eq!(
            serde_json::to_string(&QuantumProgramResultSerializationFormat::MimiqQcsrJsonV1)
                .unwrap(),
            "4"
        );
        assert_eq!(
            serde_json::to_string(
                &QuantumProgramResultSerializationFormat::PytketBackendResultJsonV1
            )
            .unwrap(),
            "1000"
        );
        assert_eq!(
            serde_json::to_string(&QuantumProgramResultSerializationFormat::QirLabeledResultV1)
                .unwrap(),
            "1001"
        );
        assert_eq!(
            serde_json::to_string(&QuantumProgramResultSerializationFormat::QsysResultJsonV1)
                .unwrap(),
            "1002"
        );
    }

    #[test]
    fn rejects_unknown_compression_format() {
        let plain = TestSerialization {
            compression_format: CompressionFormat::None,
            serialization_format: QuantumProgramSerializationFormat::QasmV3,
            serialization: "OPENQASM 3.0;".to_string(),
        };

        assert!(matches!(
            plain.to_compression(CompressionFormat::UnknownCompressionFormat),
            Err(CompressionError::UnknownCompressionFormat(_)),
        ));
    }
}
