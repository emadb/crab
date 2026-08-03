use crate::provider::LlmError;
use futures_util::{Stream, TryStreamExt, future, stream};

/// Transforms the HTTP byte stream into a stream of `data:` payloads,
/// which ends spontaneously when `[DONE]` arrives.
pub fn sse_events<B: AsRef<[u8]>>(
    bytes: impl Stream<Item = reqwest::Result<B>>,
) -> impl Stream<Item = Result<String, LlmError>> {
    let mut parser = SseParser { buf: Vec::new() };
    bytes
        .map_ok(move |chunk| stream::iter(parser.push(chunk.as_ref()).into_iter().map(Ok)))
        .map_err(LlmError::from)
        .try_flatten()
        .try_take_while(|data| future::ready(Ok(data != "[DONE]")))
}

struct SseParser {
    buf: Vec<u8>,
}

impl SseParser {
    /// Accumulates an HTTP chunk, returns complete `data:` payloads.
    fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            if let Some(data) = line.trim().strip_prefix("data:") {
                out.push(data.trim().to_string());
            }
        }
        out
    }
}
