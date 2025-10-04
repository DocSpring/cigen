/// stdio transport for gRPC communication with plugins
///
/// This module implements a custom transport layer that allows tonic gRPC
/// to communicate over stdin/stdout instead of TCP sockets.
///
/// Plugins are spawned as separate processes, and the core communicates
/// with them by sending protobuf-encoded gRPC messages over stdio.
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// A bidirectional channel over stdin/stdout
///
/// This type wraps tokio's stdin and stdout to present a unified
/// AsyncRead + AsyncWrite interface for tonic.
pub struct StdioChannel {
    stdin: tokio::io::Stdin,
    stdout: tokio::io::Stdout,
}

impl StdioChannel {
    /// Create a new stdio channel
    pub fn new() -> Self {
        Self {
            stdin: tokio::io::stdin(),
            stdout: tokio::io::stdout(),
        }
    }
}

impl Default for StdioChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncRead for StdioChannel {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdin).poll_read(cx, buf)
    }
}

impl AsyncWrite for StdioChannel {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.stdout).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stdout).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stdout).poll_shutdown(cx)
    }
}

/// A connection wrapper for child process pipes
///
/// This wraps the stdin/stdout of a spawned child process to provide
/// a bidirectional channel for gRPC communication.
pub struct ChildChannel {
    stdin: tokio::process::ChildStdin,
    stdout: tokio::process::ChildStdout,
}

impl ChildChannel {
    /// Create a new child channel from process pipes
    pub fn new(stdin: tokio::process::ChildStdin, stdout: tokio::process::ChildStdout) -> Self {
        Self { stdin, stdout }
    }
}

impl AsyncRead for ChildChannel {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for ChildChannel {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.stdin).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stdin).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stdin).poll_shutdown(cx)
    }
}

// TODO: Implement tonic transport traits for these channels
// This requires implementing the tower Service trait and tonic's transport traits
// For now, we'll use a simpler approach with direct message passing
