use hex::{decode, encode};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error, Visitor},
};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    // Construct a CompactSize from a u64 value
    pub fn new(value: u64) -> Self {
        CompactSize { value }
    }

    // Encode according to Bitcoin's CompactSize format:
    pub fn to_bytes(&self) -> Vec<u8> {
        // [0x00–0xFC] => 1 byte
        // [0xFDxxxx] => 0xFD + u16 (2 bytes)
        // [0xFExxxxxxxx] => 0xFE + u32 (4 bytes)
        // [0xFFxxxxxxxxxxxxxxxx] => 0xFF + u64 (8 bytes)

        let val = self.value;

        match val {
            // Concat slice containing header byte (0xFx) and slice repr of integer-bytes conversion
            0..=252 => u8::to_le_bytes(val as u8).into(),
            253..=65535 => [&[253u8], u16::to_le_bytes(val as u16).as_slice()].concat(),
            65536..=4294967295 => [&[254u8], u32::to_le_bytes(val as u32).as_slice()].concat(),
            4294967296..=u64::MAX => [&[255u8], val.to_le_bytes().as_slice()].concat(),
        }
    }

    // Decode CompactSize, returning value and number of bytes consumed.
    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        // First check if bytes is empty.
        let len = bytes.len();

        if len == 0 {
            Err(BitcoinError::InsufficientBytes)
        } else {
            // Check that enough bytes are available based on prefix.
            let prefix = bytes[0];

            match prefix {
                0..=252 => {
                    if len != 1 {
                        Err(BitcoinError::InvalidFormat)
                    } else {
                        let value = u8::from_le_bytes([prefix as u8]);
                        Ok((Self::new(value as u64), len))
                    }
                }
                253 => {
                    if len != 3 {
                        Err(BitcoinError::InvalidFormat)
                    } else {
                        let mut bytes_array = [0; 2];
                        bytes_array.copy_from_slice(&bytes[1..]);

                        let value = u16::from_le_bytes(bytes_array);
                        Ok((Self::new(value as u64), len))
                    }
                }
                254 => {
                    if len != 5 {
                        Err(BitcoinError::InvalidFormat)
                    } else {
                        let mut bytes_array = [0; 4];
                        bytes_array.copy_from_slice(&bytes[1..]);

                        let value = u32::from_le_bytes(bytes_array);
                        Ok((Self::new(value as u64), len))
                    }
                }
                255 => {
                    if len != 9 {
                        Err(BitcoinError::InvalidFormat)
                    } else {
                        let mut bytes_array = [0; 8];
                        bytes_array.copy_from_slice(&bytes[1..]);

                        let value = u64::from_le_bytes(bytes_array);
                        Ok((Self::new(value), len))
                    }
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    // Serialize Txid byte field as a hex-encoded string (32 bytes => 64 hex chars)
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_str = encode(self.0);
        serializer.serialize_str(&hex_str)
    }
}

struct StringVisitor;

impl<'de> Visitor<'de> for StringVisitor {
    // Implement string deserializer using visitor pattern
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a hex string with 64 characters")
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(String::from(value))
    }
}

impl<'de> Deserialize<'de> for Txid {
    // Deserialize hex string to an array of bytes in Txid struct
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Call deserializer string method to obtain string from visitor
        let hex_str = deserializer.deserialize_string(StringVisitor).unwrap();

        // Parse hex string into 32-byte array
        let raw_bytes = decode(hex_str).unwrap();

        // Validate length of hex bytes after decoding
        if raw_bytes.len() != 32 {
            Err(Error::custom("Invalid hex string. Could not decode"))
        } else {
            // Convert bytes vector to array and return
            let bytes_array = raw_bytes.try_into().unwrap();
            Ok(Txid(bytes_array))
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    // Create an OutPoint from raw txid bytes and output index
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        Self {
            txid: Txid(txid),
            vout,
        }
    }

    // Serialize as: txid (32 bytes) + vout (4 bytes, little-endian)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes_vec = vec![0; 32];
        bytes_vec.copy_from_slice(&self.txid.0);
        bytes_vec.extend_from_slice(&self.vout.to_le_bytes());
        bytes_vec
    }

    // Deserialize 36 bytes: txid[0..32], vout[32..36]
    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        // Return error if insufficient bytes
        if bytes.len() < 36 {
            Err(BitcoinError::InsufficientBytes)
        } else {
            // Create txid and vout byte arrays from bytes slice
            let txid_array: [u8; 32] = bytes[0..32].try_into().unwrap();
            let vout_array: [u8; 4] = bytes[32..36].try_into().unwrap();

            // 
            let vout = u32::from_le_bytes(vout_array);
            let txid = Txid(txid_array);

            Ok((OutPoint { txid, vout }, 36))
        }
    }
}

// #[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
// pub struct Script {
//     pub bytes: Vec<u8>,
// }

// impl Script {
//     pub fn new(bytes: Vec<u8>) -> Self {
//         // TODO: Simple constructor
//     }

//     pub fn to_bytes(&self) -> Vec<u8> {
//         // TODO: Prefix with CompactSize (length), then raw bytes
//     }

//     pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
//         // TODO: Parse CompactSize prefix, then read that many bytes
//         // Return error if not enough bytes
//     }
// }

// impl Deref for Script {
//     type Target = Vec<u8>;
//     fn deref(&self) -> &Self::Target {
//         // TODO: Allow &Script to be used as &[u8]
//     }
// }

// #[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
// pub struct TransactionInput {
//     pub previous_output: OutPoint,
//     pub script_sig: Script,
//     pub sequence: u32,
// }

// impl TransactionInput {
//     pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
//         // TODO: Basic constructor
//     }

//     pub fn to_bytes(&self) -> Vec<u8> {
//         // TODO: Serialize: OutPoint + Script (with CompactSize) + sequence (4 bytes LE)
//     }

//     pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
//         // TODO: Deserialize in order:
//         // - OutPoint (36 bytes)
//         // - Script (with CompactSize)
//         // - Sequence (4 bytes)
//     }
// }

// #[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
// pub struct BitcoinTransaction {
//     pub version: u32,
//     pub inputs: Vec<TransactionInput>,
//     pub lock_time: u32,
// }

// impl BitcoinTransaction {
//     pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
//         // TODO: Construct a transaction from parts
//     }

//     pub fn to_bytes(&self) -> Vec<u8> {
//         // TODO: Format:
//         // - version (4 bytes LE)
//         // - CompactSize (number of inputs)
//         // - each input serialized
//         // - lock_time (4 bytes LE)
//     }

//     pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
//         // TODO: Read version, CompactSize for input count
//         // Parse inputs one by one
//         // Read final 4 bytes for lock_time
//     }
// }

// impl fmt::Display for BitcoinTransaction {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         // TODO: Format a user-friendly string showing version, inputs, lock_time
//         // Display scriptSig length and bytes, and previous output info
//     }
// }
