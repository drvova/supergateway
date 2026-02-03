# Plan: Keystone

## Goal
Deliver a Rust rewrite of Supergateway with full transport parity (stdio↔SSE/WS/StreamableHTTP including stateful mode), runtime MCP args injection (interactive CLI prompt + local-only HTTP endpoint), per-session overrides + global defaults, and mixed lifecycle rules (restart for env/CLI changes, live updates for headers/tokens).

## Context Snapshot (Current TypeScript)
- Entry point: `src/index.ts` with `yargs` CLI, transport routing, logging, headers, CORS, and stateful Streamable HTTP.
- Gateways:
  - stdio→SSE: `src/gateways/stdioToSse.ts` (Express + SSEServerTransport + child stdio)
  - stdio→WS: `src/gateways/stdioToWs.ts` (Express + WebSocketServerTransport + child stdio)
  - stdio→StreamableHTTP: `src/gateways/stdioToStatelessStreamableHttp.ts` + `stdioToStatefulStreamableHttp.ts`
  - SSE→stdio: `src/gateways/sseToStdio.ts`
  - StreamableHTTP→stdio: `src/gateways/streamableHttpToStdio.ts`
- Shared utilities: `src/lib/headers.ts`, `corsOrigin.ts`, `getLogger.ts`, `sessionAccessCounter.ts`, `onSignals.ts`.
- Tests: node test runner; transport- and protocol-version coverage in `tests/`.

## Assumptions
- “MCP args injection” targets the child MCP server process for stdio-backed modes: add/override CLI args, environment variables, and request headers/tokens at runtime.
- Local-only HTTP endpoint must bind to loopback only (e.g. `127.0.0.1`) and reject non-local callers.
- Rust implementation can introduce a new binary crate while keeping the existing TS package for compatibility during migration.
- Transport parity implies matching CLI flags, headers behavior, session management, and logging semantics.

## Non-Goals
- Replacing external MCP server implementations.
- Changing protocol semantics beyond parity (no new RPCs unless needed for runtime overrides).
- Rewriting Docker assets unless required by the Rust build.

## Requirements (Functional)
1. Transport parity
   - stdio→SSE
   - stdio→WS
   - stdio→StreamableHTTP (stateless + stateful)
   - SSE→stdio
   - StreamableHTTP→stdio
2. Runtime MCP args injection
   - Interactive CLI prompt (stdin-driven) to update runtime args (CLI args/env/headers)
   - Local-only HTTP endpoint to update runtime args (CLI args/env/headers)
3. Config layering
   - Global defaults (CLI + env)
   - Per-session overrides (by session ID for stateful modes)
4. Mixed lifecycle
   - Changes to env/CLI require restart
   - Changes to headers/tokens (runtime update) apply live without restart

## Requirements (Non-Functional)
- Minimal overhead and robust error propagation (no silent failures).
- Clear logging levels similar to TS (`debug`, `info`, `none`).
- Provide deterministic session behavior for stateful Streamable HTTP.
- Maintain or improve test coverage.

## Proposed Architecture (Rust)
- Crate layout
  - `src/main.rs` CLI + config assembly + dispatch
  - `src/config/` layered config (env/cli/runtime)
  - `src/runtime/` live-update registry (headers/tokens/args)
  - `src/transports/` stdio, sse, ws, streamable_http
  - `src/session/` session registry + stateful streamable http + access counters
  - `src/logging/` log facade and levels
- Libraries (candidates)
  - `tokio`, `serde`, `serde_json`, `clap` or `argh`
  - `axum` or `hyper` for HTTP + SSE
  - `tokio-tungstenite` or `axum` WS
  - `reqwest` for client transports (SSE/StreamableHTTP)
- Core data path
  - Child process spawned via `tokio::process::Command` with piped stdio
  - Line-delimited JSON-RPC framing on stdio
  - Transport adapters translate between HTTP/SSE/WS and stdio
- Runtime updates
  - Global `RuntimeConfig` guarded by `tokio::sync::RwLock`
  - Per-session override map: `HashMap<SessionId, RuntimeOverrides>`
  - Update endpoints mutate runtime config without process restart

## CLI and Config Layering
- CLI
  - Mirror TS flags: `--stdio`, `--sse`, `--streamableHttp`, `--outputTransport`, `--port`, `--baseUrl`, `--ssePath`, `--messagePath`, `--streamableHttpPath`, `--stateful`, `--sessionTimeout`, `--header`, `--oauth2Bearer`, `--logLevel`, `--cors`, `--healthEndpoint`, `--protocolVersion`.
  - Add runtime injection flags: `--runtimePrompt`, `--runtimeAdminPort` (local-only).
- Env
  - Only used for defaults on startup. Changes require restart.
- Runtime (live updates)
  - Allowed: headers/bearer tokens, extra CLI args, env overrides
  - Disallowed: transport selection, ports, paths, stdio command base

## API Design (Local Runtime Endpoint)
- Bind: `127.0.0.1:<port>`
- Endpoints (proposal)
  - `POST /runtime/defaults` update global runtime defaults (args/env/headers)
  - `POST /runtime/session/{id}` update per-session overrides (args/env/headers)
  - `GET /runtime/sessions` list sessions
- Input validation and logging on every update.

## Migration Strategy
1. Implement Rust crate alongside TS; keep TS CLI intact.
2. Add feature parity tests or port tests to Rust binary invocations.
3. Provide a phased switch (optional `--rust` flag or separate package).

## Test Plan
- Unit tests
  - Config layering and override precedence
  - Session access counter logic
  - JSON-RPC parsing/serialization
- Integration tests
  - Each transport direction using the local mock MCP server
  - Stateful StreamableHTTP: session initialization, reuse, timeout
  - Runtime updates: headers/tokens applied live, env/CLI changes require restart
- Parity checks
  - CLI arg parsing matches existing TS behavior

## Risks & Mitigations
- MCP SDK parity in Rust: if no official crate, define minimal JSON-RPC handling and adapt to protocol changes via tests.
- SSE/StreamableHTTP specifics: ensure headers and session IDs match current behavior.
- Runtime config race conditions: use RwLock and per-session snapshot on request boundaries.

## Work Breakdown
1. Baseline parity audit
   - Extract exact behavior of each transport, session semantics, and header handling.
2. Rust scaffolding
   - Create crate, CLI parsing, logging, config layering.
3. Transport implementations
   - stdio↔SSE
   - stdio↔WS
   - stdio↔StreamableHTTP (stateless + stateful)
4. Runtime injection
   - CLI prompt loop
   - Local-only HTTP runtime API
   - Override store and application points
5. Tests and parity validation
   - Port or recreate existing tests
6. Docs and migration guidance

## Milestones
- M1: CLI + config + logging parity
- M2: stdio↔SSE and stdio↔WS parity
- M3: StreamableHTTP stateless + stateful parity
- M4: Runtime injection (prompt + local endpoint) with overrides
- M5: Test suite + docs complete

## Open Questions
- Do we need a config file format (JSON/TOML) beyond env/CLI/runtime?
- Expected behavior when runtime overrides conflict with per-session state in StreamableHTTP?
