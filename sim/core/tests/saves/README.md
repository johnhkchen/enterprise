# Golden saves

One `vN.blob` for each format N the core brings forward (`save::migrate::CHAIN`), each a real save written by the core
of that format, and a `vN.digest` beside it: what the blob comes to when today's core loads it and runs it on 30 days.
`cargo test --release --test saves` fails if one doesn't load or doesn't reach its digest.

None yet: until the first release `save::FORMAT` stays 1 and dev worlds are rebuilt. `src/save/migrate/mod.rs` says what
to do at the first format bump (`SAVES=blob` before it, `SAVES=digest` after).
