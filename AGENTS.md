# Developer Agent Guidelines: api_sapaai

This document serves as the system rules and operational constraints for AI agents pair-programming on the `api_sapaai` Rust repository.

---

## 1. Project Rules & Workflow
1. **Understand Environment**: The host operates on NixOS. Direct Cargo compilation/execution might segfault unless wrappers/paths are loaded. Always run builds/tests inside the `nix-shell` context (e.g. `nix-shell --run "make run"` or similar, or let the user run it if tools crash).
2. **Work Tracking**:
   - Create or update implementation plans under `.implementation/your_llmname/(number)-(description).md`.
   - Update development roadmaps under `.roadmap/your_llmname/(number)-(description).md`.
   - Keep API specifications updated inside [APIDOCS.md](file:///home/hylmi/Hylmi/Pemrograman_Berorientasi_Objek/Rust/api_siabsen/APIDOCS.md).

---

## 2. Coding & Database Standards
1. **Bcrypt & UUIDs**: Use secure Bcrypt hashing for password credentials and UUID v4 for session tokens.
2. **Error Responses**: All unmapped HTTP routes, timeout handlers, and internal server errors must return structured JSON envelopes:
   ```json
   {
     "success": false,
     "message": "Detailed error context"
   }
   ```
3. **Database Configuration**:
   - Always load variables using the custom env parser in [src/config.rs](file:///home/hylmi/Hylmi/Pemrograman_Berorientasi_Objek/Rust/api_sapaai/src/config.rs).
   - Cleanly encode username and password fields using `url_encode` before building the connection URL.
   - Database tables must automatically initialize on start inside `init_db()`.
   - Always import `use mysql::params;` and use `params! { ... }` directly instead of path-calling `mysql::params!`.

---

## 3. Testing & Verification
1. **No External Test Targets**: Do not restructure the cargo target from a pure binary to a library target, and do not add external integration test targets (`tests/`) or examples (`examples/` or `test/`) unless explicitly requested.
2. **Inline Unit Testing**: Write tests inside inline `#[cfg(test)]` modules placed at the **bottom of the source files** that implement the logic:
   - Colocate tests for parser logic, string formatting, configuration variables, and model mapping directly inside the target module.
   - Run tests using `cargo test` inside the nix-shell.