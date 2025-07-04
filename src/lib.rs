use hex::{decode, encode};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error, Visitor},
};
use std::fmt::{self, Display, Formatter};
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
            // Concat slice containing header byte (0xF?) and slice repr of integer-bytes conversion
            0..=252 => u8::to_le_bytes(val as u8).into(),
            253..=65535 => [&[253u8], u16::to_le_bytes(val as u16).as_slice()].concat(),
            65536..=4294967295 => [&[254u8], u32::to_le_bytes(val as u32).as_slice()].concat(),
            4294967296..=u64::MAX => [&[255u8], val.to_le_bytes().as_slice()].concat(),
        }
    }

    // Decode CompactSize, returning value and number of bytes consumed.
    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        // Check if bytes is empty.
        let len = bytes.len();

        if len == 0 {
            Err(BitcoinError::InsufficientBytes)
        } else {
            // Check that enough bytes are available based on prefix.
            let prefix = bytes[0];

            match prefix {
                0..=252 => {
                    let value = u8::from_le_bytes([prefix as u8]);
                    Ok((Self::new(value as u64), 1))
                }
                253 => {
                    if len < 3 {
                        Err(BitcoinError::InsufficientBytes)
                    } else {
                        let mut bytes_array = [0; 2];
                        bytes_array.copy_from_slice(&bytes[1..3]);

                        let value = u16::from_le_bytes(bytes_array);
                        Ok((Self::new(value as u64), 3))
                    }
                }
                254 => {
                    if len < 5 {
                        Err(BitcoinError::InsufficientBytes)
                    } else {
                        let mut bytes_array = [0; 4];
                        bytes_array.copy_from_slice(&bytes[1..5]);

                        let value = u32::from_le_bytes(bytes_array);
                        Ok((Self::new(value as u64), 5))
                    }
                }
                255 => {
                    if len < 9 {
                        Err(BitcoinError::InsufficientBytes)
                    } else {
                        let mut bytes_array = [0; 8];
                        bytes_array.copy_from_slice(&bytes[1..9]);

                        let value = u64::from_le_bytes(bytes_array);
                        Ok((Self::new(value), 9))
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
        Ok(value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
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
            // Create txid byte array from bytes slice and craft Txid struct instance
            let txid_array: [u8; 32] = bytes[0..32].try_into().unwrap();
            let txid = Txid(txid_array);

            // Create vout byte array from bytes slice and obtain vout integer
            let vout_array: [u8; 4] = bytes[32..36].try_into().unwrap();
            let vout = u32::from_le_bytes(vout_array);

            Ok((OutPoint { txid, vout }, 36))
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    // Simple constructor
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    // Prefix with CompactSize (length), then raw bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let len = self.bytes.len();

        // Store size of bytes slice as prefix: CompactSize { bytes_len }
        let size = CompactSize::new(len as u64);
        let mut prefix = size.to_bytes();

        // Append prefix and bytes to new vector
        let mut bytes_vec = Vec::with_capacity(len);
        bytes_vec.append(&mut prefix);
        bytes_vec.extend(&self.bytes);

        bytes_vec
    }

    // Parse CompactSize prefix, then read that many bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }

        // Parse CompactSize prefix to get script length
        let (compact_size, size_consumed) = CompactSize::from_bytes(bytes)?;
        let script_len = compact_size.value as usize;

        if bytes.len() < size_consumed + script_len {
            return Err(BitcoinError::InsufficientBytes);
        }

        // Extract script bytes
        let script_bytes = &bytes[size_consumed..size_consumed + script_len];
        let script = Script::new(Vec::from(script_bytes));

        Ok((script, size_consumed + script_len))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;

    // Allow &Script to be used as &[u8]
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    // Basic constructor
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        Self {
            previous_output,
            script_sig,
            sequence,
        }
    }

    // Serialize: OutPoint + Script (with CompactSize) + sequence (4 bytes LE)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut tx_input_bytes = Vec::with_capacity(44);
        tx_input_bytes.extend(&self.previous_output.to_bytes());
        tx_input_bytes.extend(&self.script_sig.to_bytes());
        tx_input_bytes.extend(&self.sequence.to_le_bytes());

        tx_input_bytes
    }

    // Deserialize in order:
    // - OutPoint (36 bytes)
    // - Script (with CompactSize)
    // - Sequence (4 bytes)
    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let bytes_len = bytes.len();

        if bytes_len < 36 {
            Err(BitcoinError::InsufficientBytes)
        } else {
            // Construct outpoint using its from_bytes method
            let (outpoint, outpoint_consumed) = OutPoint::from_bytes(&bytes[0..]).unwrap();
            let mut offset = outpoint_consumed;

            if outpoint_consumed != 36 {
                Err(BitcoinError::InvalidFormat)
            } else if bytes_len < offset {
                Err(BitcoinError::InsufficientBytes)
            } else {
                // Construct script signature using its from_byte method, starting from outpoint offset
                let (script_sig, script_consumed) = Script::from_bytes(&bytes[offset..]).unwrap();
                offset += script_consumed;

                if bytes_len < offset + 4 {
                    Err(BitcoinError::InsufficientBytes)
                } else {
                    // Read sequence from leftover bytes and calculate total_consumed_bytes
                    let sequence =
                        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
                    let total_bytes_consumed = offset + 4;

                    // Create tx_input struct and return
                    let tx_input = TransactionInput {
                        previous_output: outpoint,
                        script_sig,
                        sequence,
                    };

                    Ok((tx_input, total_bytes_consumed))
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    // Construct a transaction from parts
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        BitcoinTransaction {
            version,
            inputs,
            lock_time,
        }
    }

    // Format:
    // - version (4 bytes LE)
    // - CompactSize (number of inputs)
    // - each input serialized
    // - lock_time (4 bytes LE)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut btc_tx_bytes = Vec::new();

        // Convert version to bytes and add to return vec
        let version_le = self.version.to_le_bytes();
        btc_tx_bytes.extend(version_le);

        // Append size of inputs vec to return vec (bytes)
        let input_len = self.inputs.len();
        let input_size = CompactSize::new(input_len as u64).to_bytes();
        btc_tx_bytes.extend(input_size);

        // Serialize each tx_input and append to return vec
        for input in &self.inputs {
            let serialized_input = input.to_bytes();
            btc_tx_bytes.extend(serialized_input);
        }

        // Extend return vec with converted lock_time in bytes
        let lock_time = self.lock_time.to_le_bytes();
        btc_tx_bytes.extend(lock_time);

        btc_tx_bytes
    }

    // Read version, CompactSize for input count
    // Parse inputs one by one
    // Read final 4 bytes for lock_time
    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let bytes_len = bytes.len();

        if bytes.len() < 4 {
            Err(BitcoinError::InsufficientBytes)
        } else {
            // Read version from bytes
            let version = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
            let mut offset = 4;

            // Read CompactSize byte for input vector manipulation
            let (compact_size, size_consumed) = CompactSize::from_bytes(&bytes[offset..])?;
            let input_count = compact_size.value as usize;
            offset += size_consumed;

            // Parse and create transaction inputs
            let mut inputs: Vec<TransactionInput> = vec![];
            for _ in 0..input_count {
                if bytes_len < offset {
                    return Err(BitcoinError::InsufficientBytes);
                }

                let (tx_input, input_size) = TransactionInput::from_bytes(&bytes[offset..])?;
                inputs.push(tx_input);
                offset += input_size;
            }

            // Read lock_time
            if bytes_len < offset + 4 {
                Err(BitcoinError::InsufficientBytes)
            } else {
                let lock_time = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
                let total_bytes_consumed = offset + 4;

                // Return formatted BitcoinTransaction
                Ok((
                    BitcoinTransaction {
                        version,
                        inputs,
                        lock_time,
                    },
                    total_bytes_consumed,
                ))
            }
        }
    }
}

impl Display for BitcoinTransaction {
    // Format a user-friendly string showing version, inputs, lock_time
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Write "Version: " + version
        write!(f, "Version: {}\n", self.version)?;
        
        // Write "Input Count: " + inputs.len()
        write!(f, "Input Count: {}\n", self.inputs.len())?;
        
        // For each input (index i):
        for i in 0..self.inputs.len() {
            let input = &self.inputs[i];
            
            // Write "Input " + i + ":"
            write!(f, "Input {}:\n", i)?;
            
            // Write "  Previous Output Txid: " + hex(txid)
            write!(f, "  Previous Output Txid: {}\n", encode(&input.previous_output.txid.0))?;
            
            // Write "  Previous Output Vout: " + vout
            write!(f, "  Previous Output Vout: {}\n", input.previous_output.vout)?;
            
            // Write "  ScriptSig Length: " + script_sig.bytes.len()
            write!(f, "  ScriptSig Length: {}\n", input.script_sig.bytes.len())?;
            
            // Write "  ScriptSig Bytes: " + hex(script_sig.bytes)
            write!(f, "  ScriptSig Bytes: {}\n", encode(&input.script_sig.bytes))?;
            
            // Write "  Sequence: " + sequence
            write!(f, "  Sequence: {}\n", input.sequence)?;
        }
        
        // Write "Lock Time: " + lock_time
        write!(f, "Lock Time: {}", self.lock_time)?;
        
        // Return Ok
        Ok(())
    }
}
