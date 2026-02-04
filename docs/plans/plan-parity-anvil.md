# Parity Anvil — Feature Integration Plan

## Objective
- Achieve full TypeScript→Rust transport and CLI behavior parity, while keeping Rust implementations idiomatic and reliable, and document any intentional deviations.

## Scope
- In scope:
- Align Rust CLI flags, defaults, and validation with `src/index.ts` and related TS helpers.
- Align Rust transport behaviors with TS gateways for stdio↔SSE/WS/StreamableHTTP, SSE→stdio, StreamableHTTP→stdio.
- Close parity gaps in CORS, headers, baseUrl endpoint announcements, session handling, and health endpoints.
- Add Rust-focused tests or parity checks aligned with existing TS tests.
- Out of scope:
- Changing TS behavior except where explicitly required to resolve parity ambiguities.
- Re-architecting Rust runtime admin/prompt features beyond ensuring they don’t impact default TS parity.

## Assumptions
- Rust preview should match TS externally visible behavior unless noted as an explicit deviation.
- Rust runtime prompt/admin and telemetry remain Rust-only features, but must be inert unless enabled.
- No legacy compatibility is required; we can remove or refactor mismatched behavior rather than preserve it.

## Findings
- Target codebase:
- TS entry point: `src/index.ts` with yargs CLI and transport dispatch.
- TS gateways: `src/gateways/*.ts` (stdio↔SSE/WS/StreamableHTTP, SSE→stdio, StreamableHTTP→stdio).
- TS helpers: `src/lib/*` for headers, CORS, signal handling, version, and session timeout.
- Rust entry point: `rust/src/main.rs` with clap CLI and transport dispatch.
- Rust gateways: `rust/src/gateways/*.rs` for stdio↔SSE/WS/StreamableHTTP and SSE/StreamableHTTP clients.
- Rust support: `rust/src/support/*` for CORS, signals, child process, telemetry, session counter.
- Tests: TS-only under `tests/` covering baseUrl, protocol version, streamable HTTP behaviors, concurrency.

### Parity Matrix
- CLI flags
- `--stdio/--sse/--streamableHttp/--outputTransport`: parity mostly aligned; Rust errors if outputTransport cannot be inferred. TS default resolves via yargs default function. Confirm match.
- `--baseUrl`: TS used to build SSE endpoint event; Rust logs but does not apply. Gap.
- `--cors`: TS supports `--cors` (allow-all), `--cors "*"` (allow-all), and regex `/.../`. Rust allow-all when flag present without value but treats `"*"` as exact match. Gap.
- `--header/--oauth2Bearer`: TS logs invalid headers and still adds Authorization even when token undefined; Rust silently ignores invalid headers and only adds Authorization when token provided. Gap + bug discrepancy.
- `--healthEndpoint`: TS stdio→WS returns 500 if not ready or child dead; Rust always 200. Gap.
- `--sessionTimeout`: TS stateful uses access counter tied to response lifecycle; Rust only increments/decrements for POST and never decrements for SSE GET. Gap.
- `--protocolVersion`: TS uses for auto-init in stateless; Rust uses in stateless and should also pass through in client modes if needed. Partial parity.

- Transport behaviors
- stdio→SSE: Rust sends endpoint event without baseUrl, does not validate session existence for POST. TS uses baseUrl in endpoint and 503 for unknown session. Gap.
- stdio→WS: Rust health endpoints always OK; TS tracks readiness and child liveness. Gap.
- stdio→StreamableHTTP (stateless): Rust implements auto-init and request correlation; generally aligned. Verify error responses and method-not-allowed behavior match TS.
- stdio→StreamableHTTP (stateful): Rust session counter does not dec on GET/DELETE and does not tie to response close, so timeouts can stall. Gap. Also confirm CORS exposed headers and Mcp-Session-Id exposure.
- SSE→stdio: TS uses MCP SDK client/server with proper JSON-RPC handling; Rust is raw JSON line forwarding with minimal protocol handling. Gap.
- StreamableHTTP→stdio: same as above; Rust does not mirror TS MCP SDK behavior, including init protocol version forwarding and structured error responses. Gap.

### Explicit Deviations (to keep or fix)
- Keep Rust-only runtime prompt/admin and telemetry as optional features; ensure no effect on default parity unless enabled. (Deviation: extra capability not present in TS.)
- Fix Authorization header handling in Rust to only add when a token is provided (best-practice), and consider updating TS to match to remove "Bearer undefined" behavior. (Deviation: bug fix; document in release notes.)

## Proposed Integration
### Architecture Fit
- Keep the current Rust gateway structure but align request/response semantics and session handling to mirror TS behaviors. Avoid introducing compatibility shims; update single codepaths to match parity.

### Data & State
- Align session access counting (increment/decrement tied to response completion/close) for stateful StreamableHTTP, including GET SSE streams and DELETE termination.
- Ensure session existence validation for stdio→SSE POST message handling.
- Ensure WS readiness and child liveness are reflected in health endpoints.

### APIs & Contracts
- SSE endpoint event must include `baseUrl + messagePath` when provided; otherwise default to relative path.
- StreamableHTTP stateful responses must include `Mcp-Session-Id` and expose it via CORS.
- SSE/StreamableHTTP→stdio must implement JSON-RPC request/response wrapping like TS, including initialize flow and protocol version passthrough.

### UI/UX (If Applicable)
- N/A (CLI only).

### Background Jobs & Async
- Add lifecycle hooks for SSE GET to decrement session counters on disconnect.
- Ensure spawned child read loops and SSE/WS relays propagate errors consistently.

### Config & Feature Flags
- No feature flags; direct parity alignment. Runtime prompt/admin remain opt-in by flags.

### Observability & Error Handling
- Match TS error surface (explicit errors on invalid headers, missing sessionId, transport mismatches).
- Keep tracing; align error messages for parity tests where relevant.

## Files To Touch
- `rust/src/config.rs`
- `rust/src/gateways/stdio_to_sse.rs`
- `rust/src/gateways/stdio_to_ws.rs`
- `rust/src/gateways/stdio_to_streamable_http.rs`
- `rust/src/gateways/sse_to_stdio.rs`
- `rust/src/gateways/streamable_http_to_stdio.rs`
- `rust/src/support/cors.rs`
- `rust/src/support/session_access_counter.rs`
- `rust/src/support/stdio_child.rs`
- `rust/src/support/telemetry.rs` (only if needed for parity logging semantics)
- `rust/src/types.rs`
- `tests/` (add Rust parity tests or new Rust-targeted test harness)

## Work Plan
1. CLI and config parity
2. CORS and headers parity
3. stdio→SSE parity fixes
4. stdio→WS parity fixes
5. stdio→StreamableHTTP parity fixes (stateful lifecycle + CORS exposure)
6. SSE/StreamableHTTP→stdio parity (JSON-RPC handling)
7. Test parity additions and validation

## Risks & Mitigations
- Risk: Implementing MCP JSON-RPC behavior without a Rust SDK may diverge from TS SDK behavior.
  Mitigation: Reproduce TS behavior via explicit tests and strict protocol handling for initialize/notifications/responses.
- Risk: Session lifecycle handling in Rust could introduce leaks or premature cleanup.
  Mitigation: Mirror TS access counter semantics tied to response close events and add regression tests.
- Risk: Changing header/CORS behavior could affect downstream clients.
  Mitigation: Document deviations and keep defaults unchanged unless explicitly set by flags.

## Open Questions
- None. Proceeding with stated assumptions.

## Test & Validation Plan
- Unit tests:
- Config parsing parity (defaults and invalid args).
- CORS origin handling (`--cors`, `--cors "*"`, regex).
- Header parsing and oauth2Bearer behavior.
- Integration tests:
- Rust equivalents of TS tests for baseUrl, protocolVersion passthrough, streamable HTTP stateful/stateless, and concurrency.
- Manual verification:
- Start Rust binary in each transport mode and validate request/response flow against `tests/helpers/mock-mcp-server.js`.

## Rollout
- No migration steps; Rust preview behavior becomes parity-complete for current TS behaviors. Runtime admin/prompt remain opt-in.

## Approval
- Status: Pending
- Notes: No questions asked; proceed once approved.
