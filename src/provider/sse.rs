use crate::provider::LlmError;
use futures_util::{Stream, TryStreamExt, future, stream};

/// Transforms an HTTP byte stream into complete SSE `data:` payloads.
///
/// Each output represents one SSE event. Multiple `data:` lines in an event are
/// joined with a newline. `[DONE]` ends the returned stream.
pub fn sse_events<B: AsRef<[u8]>>(
    bytes: impl Stream<Item = reqwest::Result<B>>,
) -> impl Stream<Item = Result<String, LlmError>> {
    let mut parser = SseParser::default();
    bytes
        .map_ok(move |chunk| stream::iter(parser.push(chunk.as_ref())))
        .map_err(LlmError::from)
        .try_flatten()
        .try_take_while(|data| future::ready(Ok(data != "[DONE]")))
}

#[derive(Default)]
struct SseParser {
    buf: Vec<u8>,
}

impl SseParser {
    /// Accumulates an HTTP chunk and returns complete SSE event payloads.
    fn push(&mut self, chunk: &[u8]) -> Vec<Result<String, LlmError>> {
        self.buf.extend_from_slice(chunk);

        let mut payloads = Vec::new();
        while let Some((event_end, delimiter_len)) = find_event_end(&self.buf) {
            let event: Vec<u8> = self.buf.drain(..event_end + delimiter_len).collect();
            match parse_event(&event[..event_end]) {
                Ok(Some(payload)) => payloads.push(Ok(payload)),
                Ok(None) => {}
                Err(error) => payloads.push(Err(error)),
            }
        }

        payloads
    }
}

fn find_event_end(bytes: &[u8]) -> Option<(usize, usize)> {
    let crlf = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|pos| (pos, 4));
    let lf = bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|pos| (pos, 2));

    match (crlf, lf) {
        (Some(crlf), Some(lf)) => Some(if crlf.0 < lf.0 { crlf } else { lf }),
        (Some(event), None) | (None, Some(event)) => Some(event),
        (None, None) => None,
    }
}

fn parse_event(event: &[u8]) -> Result<Option<String>, LlmError> {
    let event = String::from_utf8(event.to_vec())?;
    let mut data = Vec::new();

    for line in event.lines() {
        if line.starts_with(':') {
            continue;
        }

        if let Some(value) = line.strip_prefix("data:") {
            data.push(value.strip_prefix(' ').unwrap_or(value));
        }
    }

    Ok((!data.is_empty()).then(|| data.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::SseParser;

    const EVENTS: &[u8] = b": keep-alive\r\n\r\ndata: {\"text\":\"caff\xc3\xa8\"}\r\ndata: second line\r\n\r\ndata: [DONE]\n\n";

    #[test]
    fn reads_complete_events_after_every_two_chunk_split() {
        let expected = vec![
            String::from("{\"text\":\"caffè\"}\nsecond line"),
            String::from("[DONE]"),
        ];

        for split in 0..=EVENTS.len() {
            let mut parser = SseParser::default();
            let actual = parser
                .push(&EVENTS[..split])
                .into_iter()
                .chain(parser.push(&EVENTS[split..]))
                .collect::<Result<Vec<_>, _>>();

            match actual {
                Ok(actual) => assert_eq!(actual, expected, "split at byte {split}"),
                Err(error) => panic!("invalid SSE event at byte {split}: {error}"),
            }
        }
    }

    #[test]
    fn reads_events_in_their_original_delimiter_order() {
        let mut parser = SseParser::default();
        let actual = parser
            .push(b"data: first\n\ndata: second\r\n\r\n")
            .into_iter()
            .collect::<Result<Vec<_>, _>>();

        match actual {
            Ok(actual) => assert_eq!(actual, vec![String::from("first"), String::from("second")]),
            Err(error) => panic!("invalid SSE event: {error}"),
        }
    }

    #[test]
    fn ignores_comments_and_events_without_data() {
        let mut parser = SseParser::default();
        let actual = parser
            .push(b": ping\n\nretry: 1000\n\ndata: answer\n: keep-alive\n\n")
            .into_iter()
            .collect::<Result<Vec<_>, _>>();

        match actual {
            Ok(actual) => assert_eq!(actual, vec![String::from("answer")]),
            Err(error) => panic!("invalid SSE event: {error}"),
        }
    }

    #[test]
    fn reports_invalid_utf8_only_after_the_event_is_complete() {
        let mut parser = SseParser::default();

        assert!(parser.push(b"data: \xff").is_empty());

        let events = parser.push(b"\n\ndata: valid\n\n");
        assert!(events[0].is_err());
        assert!(matches!(events[1].as_deref(), Ok("valid")));
    }
}
