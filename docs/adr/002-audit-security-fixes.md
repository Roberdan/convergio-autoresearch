# ADR-002: Security Audit Fixes

**Status:** Accepted  
**Date:** 2025-07-14  
**Author:** Copilot (Security Audit)

## Context

Security-first audit of convergio-autoresearch identified several vulnerabilities
and hardening opportunities in the HTTP routes, experiment runner, and string
handling.

## Findings & Fixes

| # | Category | Severity | Location | Fix |
|---|----------|----------|----------|-----|
| 1 | SQL injection pattern | Medium | `routes.rs` `handle_experiments` | Replaced `format!()` SQL with parameterized `?1 ?2` placeholders |
| 2 | SSRF | Critical | `runner.rs` `ask_for_proposal` | Added `validate_daemon_url()` — restricts to localhost `http/https` only |
| 3 | String truncation panic | Medium | `runner.rs` `update_proposal` | Replaced byte-slice `[..5000]` with `safe_truncate()` respecting UTF-8 boundaries |
| 4 | No HTTP timeout | Medium | `runner.rs` `ask_for_proposal` | Added 10s connect + 60s request timeout via `reqwest::Client::builder()` |
| 5 | Error info leakage | Low | `routes.rs` all handlers | Replaced `e.to_string()` in JSON responses with generic `"internal error"`, log actual error via `tracing::warn!` |
| 6 | Unbounded offset | Low | `routes.rs` `handle_experiments` | Capped offset to `10_000` |

## Not applicable (verified clean)

- **SQL injection via user strings**: All DB queries use `rusqlite::params![]` — safe.
- **Path traversal**: `repo_root` is set from env/cwd at startup, not from HTTP input.
- **Command injection**: All `Command::new()` calls use array args, not shell interpolation.
- **Auth/AuthZ**: Routes are protected by SDK `required_roles` in manifest (`orchestrator`, `all`).
- **Unsafe blocks**: None found.
- **Race conditions**: SQLite serialized mode + `ConnPool`; `tokio::spawn` for async — acceptable.
- **Secret exposure**: No secrets in code or config defaults.

## Tests added

- `validate_daemon_url_allows_localhost` — accepts localhost variants
- `validate_daemon_url_rejects_external` — rejects external hosts, bad schemes
- `safe_truncate_ascii` — basic truncation
- `safe_truncate_multibyte` — UTF-8 boundary safety

## Decision

All findings fixed. Test count: 19 → 25.
