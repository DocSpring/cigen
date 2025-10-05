/// Message framing for stdio-based plugin communication
///
/// This module implements a simple length-prefixed framing protocol:
/// [4-byte length (big-endian)][protobuf message bytes]
use anyhow::{Context, Result, bail};
use prost::Message;
use std::io::{Read, Write};

/// Maximum message size (10MB) to prevent memory exhaustion
const MAX_MESSAGE_SIZE: u32 = 10 * 1024 * 1024;

/// Send a protobuf message with length-prefix framing
///
/// Wire format: [4-byte length][message bytes]
/// - Length is big-endian u32
/// - Message is protobuf-encoded
pub fn send_message<M: Message, W: Write>(msg: &M, writer: &mut W) -> Result<()> {
    // Encode the message to bytes
    let buf = msg.encode_to_vec();

    // Check message size
    if buf.len() > MAX_MESSAGE_SIZE as usize {
        bail!(
            "Message too large: {} bytes (max {})",
            buf.len(),
            MAX_MESSAGE_SIZE
        );
    }

    // Write length prefix (4 bytes, big-endian)
    let len = buf.len() as u32;
    writer
        .write_all(&len.to_be_bytes())
        .context("Failed to write message length")?;

    // Write message bytes
    writer
        .write_all(&buf)
        .context("Failed to write message body")?;

    // Flush to ensure message is sent immediately
    writer.flush().context("Failed to flush writer")?;

    Ok(())
}

/// Receive a protobuf message with length-prefix framing
///
/// Reads: [4-byte length][message bytes]
/// Returns: Decoded protobuf message
pub fn receive_message<M: Message + Default, R: Read>(reader: &mut R) -> Result<M> {
    // Read length prefix (4 bytes, big-endian)
    let mut len_buf = [0u8; 4];
    reader
        .read_exact(&mut len_buf)
        .context("Failed to read message length")?;

    let len = u32::from_be_bytes(len_buf);

    // Validate message size
    if len > MAX_MESSAGE_SIZE {
        bail!(
            "Message too large: {} bytes (max {})",
            len,
            MAX_MESSAGE_SIZE
        );
    }

    // Read message bytes
    let mut buf = vec![0u8; len as usize];
    reader
        .read_exact(&mut buf)
        .context("Failed to read message body")?;

    // Decode protobuf message
    let msg = M::decode(&buf[..]).context("Failed to decode protobuf message")?;

    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::protocol::Hello;
    use std::collections::HashMap;

    #[test]
    fn test_send_receive_round_trip() {
        let original = Hello {
            core_protocol: 1,
            core_version: "0.2.0".to_string(),
            env: HashMap::new(),
        };

        // Encode to buffer
        let mut buf = Vec::new();
        send_message(&original, &mut buf).unwrap();

        // Decode from buffer
        let decoded: Hello = receive_message(&mut &buf[..]).unwrap();

        assert_eq!(decoded.core_protocol, original.core_protocol);
        assert_eq!(decoded.core_version, original.core_version);
    }

    #[test]
    fn test_message_with_data() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert("HOME".to_string(), "/home/user".to_string());

        let original = Hello {
            core_protocol: 1,
            core_version: "0.2.0".to_string(),
            env: env.clone(),
        };

        let mut buf = Vec::new();
        send_message(&original, &mut buf).unwrap();
        let decoded: Hello = receive_message(&mut &buf[..]).unwrap();

        assert_eq!(decoded.env, env);
    }

    #[test]
    fn test_message_size_limit() {
        // Create a message that's too large
        let mut huge_env = HashMap::new();
        for i in 0..1_000_000 {
            huge_env.insert(format!("KEY_{i}"), "x".repeat(100));
        }

        let msg = Hello {
            core_protocol: 1,
            core_version: "0.2.0".to_string(),
            env: huge_env,
        };

        let mut buf = Vec::new();
        let result = send_message(&msg, &mut buf);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Message too large")
        );
    }

    #[test]
    fn test_truncated_length() {
        let buf = [0x00, 0x00]; // Only 2 bytes instead of 4
        let result = receive_message::<Hello, _>(&mut &buf[..]);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to read message length")
        );
    }

    #[test]
    fn test_truncated_body() {
        // Length says 100 bytes, but only provide 10
        let mut buf = vec![0x00, 0x00, 0x00, 0x64]; // length = 100
        buf.extend_from_slice(&[0u8; 10]); // only 10 bytes of data

        let result = receive_message::<Hello, _>(&mut &buf[..]);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to read message body")
        );
    }

    #[test]
    fn test_invalid_protobuf() {
        // Valid length prefix, but garbage protobuf data
        let mut buf = vec![0x00, 0x00, 0x00, 0x0A]; // length = 10
        buf.extend_from_slice(&[0xFF; 10]); // garbage data

        let result = receive_message::<Hello, _>(&mut &buf[..]);

        // Should fail to decode
        assert!(result.is_err());
    }
}
