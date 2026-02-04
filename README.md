![Supergateway: Run stdio MCP servers over SSE and WS](https://raw.githubusercontent.com/supercorp-ai/supergateway/main/supergateway.png)

**Supergateway** runs **MCP stdio-based servers** over **SSE (Server-Sent Events)** or **WebSockets (WS)** with one command. This is useful for remote access, debugging, or connecting to clients when your MCP server only supports stdio.

Supported by [Supermachine](https://supermachine.ai) (hosted MCPs), [Superinterface](https://superinterface.ai), and [Supercorp](https://supercorp.ai).

## Installation & Usage

Build the Rust binary:

```bash
cd rust
cargo build --release
```

Run Supergateway using the built binary:

```bash
./rust/target/release/supergateway --stdio "./my-mcp-server --root ."
```

For development:

```bash
cargo run --manifest-path rust/Cargo.toml -- --stdio "./my-mcp-server --root ."
```

- **`--stdio "command"`**: Command that runs an MCP server over stdio
- **`--sse "https://mcp-server-ab71a6b2-cd55-49d0-adba-562bc85956e3.supermachine.app"`**: SSE URL to connect to (SSE→stdio mode)
- **`--streamableHttp "https://mcp-server.example.com/mcp"`**: Streamable HTTP URL to connect to (StreamableHttp→stdio mode)
- **`--outputTransport stdio | sse | ws | streamableHttp`**: Output MCP transport (default: `sse` with `--stdio`, `stdio` with `--sse` or `--streamableHttp`)
- **`--port 8000`**: Port to listen on (stdio→SSE or stdio→WS mode, default: `8000`)
- **`--baseUrl "http://localhost:8000"`**: Base URL for SSE or WS clients (stdio→SSE mode; optional)
- **`--ssePath "/sse"`**: Path for SSE subscriptions (stdio→SSE mode, default: `/sse`)
- **`--messagePath "/message"`**: Path for messages (stdio→SSE or stdio→WS mode, default: `/message`)
- **`--streamableHttpPath "/mcp"`**: Path for Streamable HTTP (stdio→Streamable HTTP mode, default: `/mcp`)
- **`--stateful`**: Run stdio→Streamable HTTP in stateful mode
- **`--sessionTimeout 60000`**: Session timeout in milliseconds (stateful stdio→Streamable HTTP mode only)
- **`--header "x-user-id: 123"`**: Add one or more headers (stdio→SSE, SSE→stdio, or Streamable HTTP→stdio mode; can be used multiple times)
- **`--oauth2Bearer "some-access-token"`**: Adds an `Authorization` header with the provided Bearer token
- **`--logLevel debug | info | none`**: Controls logging level (default: `info`). Use `debug` for more verbose logs, `none` to suppress all logs.
- **`--cors`**: Enable CORS (stdio→SSE or stdio→WS mode). Use `--cors` with no values to allow all origins, or supply one or more allowed origins (e.g. `--cors "http://example.com"` or `--cors "/example\\.com$/"` for regex matching).
- **`--healthEndpoint /healthz`**: Register one or more endpoints (stdio→SSE or stdio→WS mode; can be used multiple times) that respond with `"ok"`

## Runtime MCP Args Injection

You can update MCP server args and headers during runtime instead of only at startup:

- **Interactive prompt**: `--runtimePrompt`
- **Local admin endpoint**: `--runtimeAdminPort 7777` (binds to `127.0.0.1`)

#### Admin API

- `POST /runtime/defaults`
- `POST /runtime/session/{id}`
- `GET /runtime/sessions`

Payload example:

```json
{
  "extra_cli_args": ["--token", "abc123"],
  "env": { "API_KEY": "xyz" },
  "headers": { "Authorization": "Bearer 123" }
}
```

Notes:
- `extra_cli_args` and `env` updates trigger a child restart when applicable.
- `headers` updates are applied live.

### Telemetry (Rust)

The Rust build uses `tracing` with optional OpenTelemetry OTLP export. Logs always print locally (stdout or stderr depending on `--outputTransport`).

**OTLP env vars:**
- `OTEL_EXPORTER_OTLP_ENDPOINT` (base endpoint for traces and logs)
- `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` (override for traces)
- `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` (override for logs)

**Quick smoke test:**
```bash
cd rust
export OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4318"
cargo run -- --stdio "./my-mcp-server --root ./my-folder" --port 8000
```
You should see spans/logs in your local collector. If you want to adjust local verbosity, set `RUST_LOG=debug` or `RUST_LOG=info`.

## stdio → SSE

Expose an MCP stdio server as an SSE server:

```bash
./rust/target/release/supergateway \
    --stdio "./my-mcp-server --root ./my-folder" \
    --port 8000 --baseUrl http://localhost:8000 \
    --ssePath /sse --messagePath /message
```

- **Subscribe to events**: `GET http://localhost:8000/sse`
- **Send messages**: `POST http://localhost:8000/message`

## SSE → stdio

Connect to a remote SSE server and expose locally via stdio:

```bash
./rust/target/release/supergateway --sse "https://mcp-server-ab71a6b2-cd55-49d0-adba-562bc85956e3.supermachine.app"
```

Useful for integrating remote SSE MCP servers into local command-line environments.

You can also pass headers when sending requests. This is useful for authentication:

```bash
./rust/target/release/supergateway \
    --sse "https://mcp-server-ab71a6b2-cd55-49d0-adba-562bc85956e3.supermachine.app" \
    --oauth2Bearer "some-access-token" \
    --header "X-My-Header: another-header-value"
```

## Streamable HTTP → stdio

Connect to a remote Streamable HTTP server and expose locally via stdio:

```bash
./rust/target/release/supergateway --streamableHttp "https://mcp-server.example.com/mcp"
```

This mode is useful for connecting to MCP servers that use the newer Streamable HTTP transport protocol. Like SSE mode, you can also pass headers for authentication:

```bash
./rust/target/release/supergateway \
    --streamableHttp "https://mcp-server.example.com/mcp" \
    --oauth2Bearer "some-access-token" \
    --header "X-My-Header: another-header-value"
```

## stdio → Streamable HTTP

Expose an MCP stdio server as a Streamable HTTP server.

### Stateless mode

```bash
./rust/target/release/supergateway \
    --stdio "./my-mcp-server --root ./my-folder" \
    --outputTransport streamableHttp \
    --port 8000
```

### Stateful mode

```bash
./rust/target/release/supergateway \
    --stdio "./my-mcp-server --root ./my-folder" \
    --outputTransport streamableHttp --stateful \
    --sessionTimeout 60000 --port 8000
```

The Streamable HTTP endpoint defaults to `http://localhost:8000/mcp` (configurable via `--streamableHttpPath`).

## stdio → WS

Expose an MCP stdio server as a WebSocket server:

```bash
./rust/target/release/supergateway \
    --stdio "./my-mcp-server --root ./my-folder" \
    --port 8000 --outputTransport ws --messagePath /message
```

- **WebSocket endpoint**: `ws://localhost:8000/message`

## Using with ngrok

Use [ngrok](https://ngrok.com/) to share your local MCP server publicly:

```bash
./rust/target/release/supergateway --port 8000 --stdio "./my-mcp-server --root ."

# In another terminal:
ngrok http 8000
```

ngrok provides a public URL for remote access.

MCP server will be available at URL similar to: https://1234-567-890-12-456.ngrok-free.app/sse

## Running with Docker (Rust)

Build and run the Rust image:

```bash
docker build -f docker/Dockerfile -t supergateway .

docker run -it --rm -p 8000:8000 supergateway \
    --stdio "/usr/local/bin/my-mcp-server --root /" \
    --port 8000
```

Docker pulls dependencies during build. The MCP server runs in the container’s root directory (`/`). You can mount host directories if needed.

## Using with Claude Desktop (SSE → stdio mode)

Claude Desktop can use Supergateway’s SSE→stdio mode.

### Local Binary Example

```json
{
  "mcpServers": {
    "supermachineExample": {
      "command": "/path/to/supergateway",
      "args": ["--sse", "https://mcp-server-ab71a6b2-cd55-49d0-adba-562bc85956e3.supermachine.app"]
    }
  }
}
```

## Using with Cursor (SSE → stdio mode)

Cursor can also integrate with Supergateway in SSE→stdio mode. The configuration is similar to Claude Desktop.

### Local Binary Example for Cursor

```json
{
  "mcpServers": {
    "cursorExample": {
      "command": "/path/to/supergateway",
      "args": ["--sse", "https://mcp-server-ab71a6b2-cd55-49d0-adba-562bc85956e3.supermachine.app"]
    }
  }
}
```

**Note:** Although the setup supports sending headers via the `--header` flag, if you need to pass an Authorization header (which typically includes a space, e.g. `"Bearer 123"`), you must use the `--oauth2Bearer` flag due to a known Cursor bug with spaces in command-line arguments.

## Why MCP?

[Model Context Protocol](https://spec.modelcontextprotocol.io/) standardizes AI tool interactions. Supergateway converts MCP stdio servers into SSE or WS services, simplifying integration and debugging with web-based or remote clients.

## Advanced Configuration

Supergateway emphasizes modularity:

- Automatically manages JSON-RPC versioning.
- Retransmits package metadata where possible.
- stdio→SSE or stdio→WS mode logs via standard output; SSE→stdio mode logs via stderr.

## Additional resources

- [Superargs](https://github.com/supercorp-ai/superargs) - provide arguments to MCP servers during runtime.

## Contributors

- [@longfin](https://github.com/longfin)
- [@griffinqiu](https://github.com/griffinqiu)
- [@folkvir](https://github.com/folkvir)
- [@wizizm](https://github.com/wizizm)
- [@dtinth](https://github.com/dtinth)
- [@rajivml](https://github.com/rajivml)
- [@NicoBonaminio](https://github.com/NicoBonaminio)
- [@sibbl](https://github.com/sibbl)
- [@podarok](https://github.com/podarok)
- [@jmn8718](https://github.com/jmn8718)
- [@TraceIvan](https://github.com/TraceIvan)
- [@zhoufei0622](https://github.com/zhoufei0622)
- [@ezyang](https://github.com/ezyang)
- [@aleksadvaisly](https://github.com/aleksadvaisly)
- [@wuzhuoquan](https://github.com/wuzhuoquan)
- [@mantrakp04](https://github.com/mantrakp04)
- [@mheubi](https://github.com/mheubi)
- [@mjmendo](https://github.com/mjmendo)
- [@CyanMystery](https://github.com/CyanMystery)
- [@earonesty](https://github.com/earonesty)
- [@StefanBurscher](https://github.com/StefanBurscher)
- [@tarasyarema](https://github.com/tarasyarema)
- [@pcnfernando](https://github.com/pcnfernando)
- [@Areo-Joe](https://github.com/Areo-Joe)
- [@Joffref](https://github.com/Joffref)
- [@michaeljguarino](https://github.com/michaeljguarino)

## Contributing

Issues and PRs welcome. Please open one if you encounter problems or have feature suggestions.

## Tests

Rust build sanity checks:

```bash
cd rust
cargo check
cargo test
```

If you add Rust integration tests, keep them self-contained and offline-friendly.

## License

[MIT License](./LICENSE)
