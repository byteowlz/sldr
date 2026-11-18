# sldr-server

Standalone HTTP API with an optional prebuilt Studio frontend. Start it with no
arguments; existing environment-based startup, routes, authentication, TLS and
bind defaults are unchanged. This does not resolve the separate `sldr serve`
entrypoint or choose a canonical Studio launcher (trx-h4xt remains open).

`--help` / `-h` and `--version` / `-V` exit successfully before constructing the
async runtime, reading configuration or library files, resolving TLS, or opening
listeners. Unknown arguments and multiple arguments exit with status 2, also
before startup. Only a single informational flag is accepted; server settings
remain environment-driven. No argument-parser dependency or lockfile change is
needed for this bounded interface.

Version output includes the package version, compile-time git description
(including a dirty marker when applicable), and Cargo target triple. Builds
without git/source history explicitly report unknown provenance. There is no
runtime git lookup or build-version environment override.

## Verification

```sh
cargo check --locked -p sldr-server
cargo test --locked -p sldr-server
```

`tests/startup_flags.rs` launches only Cargo's freshly built binary, with a
cleared environment, synthetic HOME/XDG paths, a deadline and child cleanup.
A test-owned occupied loopback port prevents even a regressed binary from
exposing a server. Correct outputs/status and an empty temporary tree rule out
startup/config/certificate creation; malformed server settings verify that argv
wins over environment validation. Unix FIFO read traps cover existing config,
private library slides and explicit TLS files. The no-argument probe confirms
that environment-driven server validation still runs. These tests do not claim
to verify live-service behavior, all networking syscalls, or the remaining
canonical-launch/Board integration work in trx-h4xt.
