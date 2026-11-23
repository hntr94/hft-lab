# E: Wire format

Phase 4 (Performance) · Depends on: A (B for sequence numbers) · Size: M

## Vision

Until now, the exchange and bot pass Rust structs to each other in memory. Real systems receive bytes over a network in a binary protocol, and decoding those bytes is a big part of the hot path. This initiative designs a small exchange protocol in the style of Nasdaq ITCH and CME SBE, encodes on the exchange side, decodes on the bot side without copying, and optionally sends it over a real UDP socket.

**Success looks like:** the bot runs entirely on decoded binary messages with identical trading results. The decoder is fuzzed and never crashes on garbage input. You can read a hex dump of a message and know what it says.

**You'll learn:**
- binary protocol design (fixed-size fields, message types, versioning, sequence numbers)
- endianness
- alignment
- zero-copy decoding
- safe parsing in Rust
- fuzz testing
- why exchanges don't use JSON
- optionally, sockets and UDP multicast basics

## Key concepts

- **Fixed-size messages:** every message type has a known length and field offsets, so decoding is a few memory reads, not parsing.
- **Endianness:** byte order of multi-byte integers. Protocols pick one (ITCH is big-endian; SBE usually little-endian). Get it wrong and prices are garbage.
- **Zero-copy:** interpret the received byte buffer directly as a struct view instead of copying fields into a new struct. `zerocopy`/`bytemuck` make this safe in Rust.
- **Framing:** how a receiver knows where one message ends, e.g. a length prefix or a packet header holding a message count and sequence number.
- **Fuzzing:** feeding millions of random or mutated inputs to the decoder to find crashes and panics.

## Design sketch

- A new `protocol` crate holding message layouts, encoder and decoder. Both exchange and bot depend on it.
- Messages:
  - market data: book update, trade, snapshot start/level/end, heartbeat
  - order entry: new order, cancel, ack, reject, fill, cancelled
- A packet header carries a sequence number (reused from B), a timestamp and a message count.
- Transport abstraction: in-memory byte buffers (default, deterministic) or a local UDP socket (wall-clock mode only).
- `cargo fuzz` target for the decoder.
- A hex-dump debugging tool: `protocol-dump <file>` prints messages from a captured byte stream.

## Task breakdown

| # | Task | Owner | Depends on |
|---|---|---|---|
| E.1 | Guide: binary protocols, ITCH/SBE overview, endianness, zero-copy; reading list (ITCH spec, SBE docs) | [Claude] | none |
| E.2 | Read part of the real Nasdaq ITCH spec together; decode one real message by hand | [Together] | E.1 |
| E.3 | Protocol spec doc: message types, field layouts, endianness, framing, versioning (`docs/protocol.md`) | [Together] | E.2 |
| E.4 | `protocol` crate skeleton: message layout types + encoder | [Claude] | E.3 |
| E.5 | Scaffolding for the decoder: round-trip tests, malformed-input tests, guide | [Claude] | E.4 |
| E.6 | **Zero-copy decoder** | [You] | E.5 |
| E.7 | Fuzz target for the decoder; run it; fix the findings | [Together] | E.6 |
| E.8 | Wire exchange → protocol → bot through in-memory byte transport; equivalence test against the struct-based path | [Claude] | E.6 |
| E.9 | Benchmark decode cost per message; compare with the struct path and with a JSON baseline (to show why exchanges avoid JSON) | [You] | E.8, A |
| E.10 | Optional: UDP transport on localhost; measure the OS networking cost | [Together] | E.8 |
| E.11 | `protocol-dump` tool | [Claude] | E.6 |

## Pitfalls

- Using `unsafe` pointer casts instead of `zerocopy`/`bytemuck`; alignment bugs crash on some CPUs.
- Panicking on malformed input. A decoder must return errors, never panic.
- Mixing endianness between encoder and decoder. Round-trip tests catch this.
- Forgetting versioning. Add a version byte now; it's painful to add later.

## Done when

- [ ] The bot runs entirely over the binary protocol with summaries identical to the struct path.
- [ ] The decoder survives a sustained fuzzing run without panics.
- [ ] Decode benchmarks (binary vs struct vs JSON) are recorded.
- [ ] `docs/protocol.md` describes the protocol fully.
