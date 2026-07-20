# SAPA-AI CRM API

A full-featured CRM API built with Rust nightly, designed to manage leads, contacts, companies, deals, activities, quotes, tickets, campaigns, and more. It connects to a MySQL database and includes an in-process WhatsApp Web integration for messaging leads and customers.

## Stack

- **[Axum](https://docs.rs/axum)** – HTTP framework built on Tower.
- **[Tokio](https://docs.rs/tokio)** – async runtime.
- **[mysql](https://docs.rs/mysql)** – MySQL driver.
- **[bcrypt](https://docs.rs/bcrypt)** – password hashing.
- **[whatsapp-rust](https://github.com/jlucaso1/whatsapp-rust)** – in-process WhatsApp Web session.
- **[Tower HTTP](https://docs.rs/tower-http)** – CORS, timeouts, tracing.

## Quick start

```bash
cd api_sapaai

# Copy and edit environment values (MySQL credentials)
cp .env.example .env

# Run inside the NixOS development shell
nix-shell --run "cargo run"
```

The server starts on `http://0.0.0.0:5790` by default.

## Configuration

Configuration is centralized in `src/config.rs`. Values are read from environment variables or a `.env` file.

| Variable | Default | Description |
|---|---|---|
| `APP_NAME` | `api_sapaai_crm` | Application name used in logs |
| `APP_SERVER_HOST` | `0.0.0.0` | Bind host |
| `APP_SERVER_PORT` | `5790` | Bind port |
| `DATABASE_HOST` | `127.0.0.1` | MySQL host |
| `DATABASE_PORT` | `3306` | MySQL port |
| `DATABASE_NAME` | `crm_sapaai` | MySQL database |
| `DATABASE_USERNAME` | `root` | MySQL user |
| `DATABASE_PASSWORD` | `` | MySQL password |
| `SERVER_BASE_URL` | `http://localhost:5790` | Public base URL |
| `RUST_LOG` | `info` | Logging filter |

## Project layout

```text
src/
├── main.rs              # Entry point
├── lib.rs               # Module declarations
├── config.rs            # Environment configuration
├── error.rs             # Central AppError type
├── state.rs             # Shared application state (pool + WA registry)
├── server.rs            # TCP listener + graceful shutdown
├── middleware.rs        # Request id + access log
├── response.rs          # ApiResponse<T> envelopes
├── database/
│   ├── database.rs      # init_db() with CRM schema
│   ├── scheme.rs        # Domain structs
│   └── mod.rs           # AppState re-export
├── handlers/            # Route handlers (CRUD + auth + WA)
├── models/              # Request/response DTOs
├── routes/              # Router composition
├── utils/               # Logging macros
└── whatsapp/            # WhatsApp Web session registry
```

## Build helpers

A `Makefile` is provided for common tasks:

```bash
make run     # cargo run (dev build)
make check   # cargo check
make test    # cargo test
make fmt     # cargo fmt
make clean   # cargo clean
```

## Testing

```bash
# Run all inline unit tests
nix-shell --run "cargo test"

# Run with logging visible
nix-shell --run "cargo test -- --nocapture"
```

## API documentation

See [APIDOCS.md](APIDOCS.md) for the full endpoint reference.

## License

MIT OR Apache-2.0
