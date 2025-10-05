# stdio Transport Design for Plugin Communication

## Overview

Plugins communicate with the core via gRPC messages sent over stdin/stdout. This document describes the message framing protocol and implementation strategy.

## Message Framing

Since stdin/stdout are byte streams without message boundaries, we need a framing protocol:

### Simple Length-Prefixed Framing

```
[4-byte length][protobuf message bytes]
```

- **Length**: unsigned 32-bit integer (big-endian)
- **Message**: serialized protobuf message

### Example

```
Request: Hello { core_protocol: 1, core_version: "0.2.0" }

Wire format:
[0x00, 0x00, 0x00, 0x15]  // length = 21 bytes
[protobuf bytes...]        // 21 bytes of serialized Hello message
```

## Implementation Strategy

### Phase 1: Simple Blocking I/O (Current)

For the initial implementation, use synchronous message passing:

```rust
// Core sends message to plugin
fn send_message<M: prost::Message>(msg: &M, stdout: &mut ChildStdin) -> Result<()> {
    let buf = msg.encode_to_vec();
    let len = (buf.len() as u32).to_be_bytes();
    stdout.write_all(&len)?;
    stdout.write_all(&buf)?;
    stdout.flush()?;
    Ok(())
}

// Core receives message from plugin
fn receive_message<M: prost::Message + Default>(stdin: &mut ChildStdout) -> Result<M> {
    let mut len_buf = [0u8; 4];
    stdin.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;

    let mut buf = vec![0u8; len];
    stdin.read_exact(&mut buf)?;

    let msg = M::decode(&buf[..])?;
    Ok(msg)
}
```

**Pros**:

- Simple to implement and debug
- No async complexity
- Good enough for initial validation

**Cons**:

- Blocks on I/O
- Can't handle streaming/long-running operations

### Phase 2: Async with Tokio (Future)

For production, migrate to async I/O:

```rust
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn send_message_async<M: prost::Message>(
    msg: &M,
    writer: &mut (impl AsyncWriteExt + Unpin)
) -> Result<()> {
    let buf = msg.encode_to_vec();
    let len = (buf.len() as u32).to_be_bytes();
    writer.write_all(&len).await?;
    writer.write_all(&buf).await?;
    writer.flush().await?;
    Ok(())
}
```

**Pros**:

- Non-blocking
- Can handle multiple plugins concurrently
- Better for long-running operations

**Cons**:

- More complex
- Harder to debug

## Plugin Lifecycle

### 1. Spawn

```rust
let mut child = tokio::process::Command::new("cigen-provider-github")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())  // Plugin logs go to our stderr
    .spawn()?;

let mut stdin = child.stdin.take().unwrap();
let mut stdout = child.stdout.take().unwrap();
```

### 2. Handshake

```
Core → Plugin: Hello { core_protocol: 1, core_version: "0.2.0", env: {...} }
Plugin → Core: PluginInfo { name: "provider/github", version: "0.1.0", protocol: 1, capabilities: [...] }
```

**Validation**:

- Check `protocol` matches
- Verify `capabilities` are as expected
- Store plugin metadata for routing

### 3. Hook Invocation

```
Core → Plugin: GenerateRequest { target: "github", graph: [...], schema: {...} }
Plugin → Core: GenerateResult { fragments: [...], diagnostics: [...] }
```

### 4. Shutdown

```rust
// Graceful shutdown
drop(stdin);  // Close stdin to signal plugin to exit
child.wait_with_timeout(Duration::from_secs(5))?;

// Force kill if timeout
child.kill()?;
```

## Error Handling

### Plugin Crashes

If plugin process exits unexpectedly:

```rust
match child.try_wait()? {
    Some(status) => {
        bail!("Plugin exited unexpectedly with status: {}", status);
    }
    None => {
        // Plugin still running, continue
    }
}
```

### Message Decode Errors

```rust
match M::decode(&buf[..]) {
    Ok(msg) => msg,
    Err(e) => {
        bail!("Failed to decode message from plugin: {}", e);
    }
}
```

### Timeouts

```rust
tokio::time::timeout(
    Duration::from_secs(30),
    receive_message(&mut stdout)
).await??;  // Outer ? for timeout, inner ? for I/O error
```

## Testing Strategy

### Unit Tests

Test message framing:

```rust
#[test]
fn test_message_framing() {
    let msg = Hello {
        core_protocol: 1,
        core_version: "0.2.0".to_string(),
        env: HashMap::new(),
    };

    let mut buf = Vec::new();
    send_message(&msg, &mut buf).unwrap();

    let decoded: Hello = receive_message(&mut &buf[..]).unwrap();
    assert_eq!(decoded.core_protocol, 1);
}
```

### Integration Tests

Test full plugin lifecycle:

```rust
#[tokio::test]
async fn test_plugin_handshake() {
    let mut manager = PluginManager::new();
    let plugin = manager.spawn("cigen-provider-github").await.unwrap();

    let info = plugin.handshake().await.unwrap();
    assert_eq!(info.name, "provider/github");
    assert_eq!(info.protocol, 1);
}
```

## Alternative: JSON over stdio

**Considered but rejected** for performance reasons:

```json
{ "type": "Hello", "payload": { "core_protocol": 1, "core_version": "0.2.0" } }
```

**Pros**:

- Human-readable
- Easy to debug with `echo` and `cat`
- Language-agnostic (no protobuf dep)

**Cons**:

- Slower to parse
- Larger message size
- No schema validation
- Still need message framing (newlines are fragile)

## Security Considerations

### Input Validation

- **Max message size**: Reject messages >10MB
- **Protocol version**: Only accept exact match
- **Capability validation**: Verify declared capabilities

### Process Isolation

- **No shell**: Spawn plugins directly (not via shell)
- **Limited permissions**: Plugins run as same user (for now)
- **Resource limits**: Future: cgroups, ulimits

### Malicious Plugins

For untrusted plugins (future):

- WASM sandbox (Wasmtime)
- Capability-based permissions
- Read-only filesystem access

## Performance Targets

- **Handshake**: <100ms
- **Generate hook**: <1s for typical workflow
- **Message overhead**: <1ms per message
- **Plugin spawn**: <50ms

## Alternatives Considered

### 1. Unix Domain Sockets

**Pros**: More robust than pipes, bidirectional
**Cons**: Platform-specific, harder to debug

### 2. HTTP/REST

**Pros**: Easy to debug (curl), language-agnostic
**Cons**: Need to allocate ports, more overhead

### 3. MessagePack

**Pros**: Smaller than JSON, faster than protobuf
**Cons**: Less tooling, no schema

### 4. gRPC over TCP

**Pros**: Full gRPC features (streaming, etc.)
**Cons**: Need port allocation, more complex

**Decision**: Stick with stdio + protobuf for simplicity and portability.

## Implementation Checklist

- [ ] Create `src/plugin/framing.rs` with send/receive functions
- [ ] Update `PluginManager::spawn()` to establish stdio pipes
- [ ] Implement `PluginManager::handshake()`
- [ ] Add message timeout handling
- [ ] Write unit tests for framing
- [ ] Write integration test for spawn + handshake
- [ ] Update GitHub provider plugin to use stdio server
- [ ] Test end-to-end: core → plugin → generate → output

## References

- [go-plugin](https://github.com/hashicorp/go-plugin) - Terraform's plugin system
- [prost](https://docs.rs/prost/) - Protobuf library
- [tonic](https://docs.rs/tonic/) - gRPC library
- [Length-Prefixed Message Framing](https://eli.thegreenplace.net/2011/08/02/length-prefix-framing-for-protocol-buffers)
