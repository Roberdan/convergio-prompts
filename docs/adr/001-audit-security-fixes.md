# ADR-001: Security Audit & Hardening

**Status**: Accepted  
**Date**: 2025-07-21  
**Author**: Security Audit (automated)

## Context

First security audit of `convergio-prompts` after initial extraction from the
monorepo. The crate exposes HTTP routes for prompt template CRUD, skill
registry, A/B testing, spawn freezing, and pipelines — all backed by SQLite.

## Findings & Decisions

### 1. Error information leakage (HIGH → Fixed)

**Before**: Route handlers returned raw `rusqlite::Error` and pool error
messages to HTTP clients via `.to_string()`, exposing table names, SQL
fragments, and internal state.

**After**: All error responses now return generic `"internal error"` to clients.
Original errors are logged via `tracing::error!` for observability.

### 2. Missing input validation (HIGH → Fixed)

**Before**: `PromptInput`, `SkillInput`, `PipelineInput` accepted unbounded
strings — names of any length, arbitrary characters, empty bodies.

**After**: Added `validate()` methods enforcing:
- Name fields: non-empty, ≤256 chars, alphanumeric + `- _ . space`
- Body fields: non-empty, ≤100k chars
- Variables: max 50 per prompt
- Pipeline steps: max 100
- Confidence: finite `f64` in `[0.0, 1.0]`

Routes call `validate()` before any DB operation, returning 422 on failure.

### 3. Confidence value unbounded (MEDIUM → Fixed)

**Before**: `SkillInput.confidence` accepted NaN, Infinity, negative values.
`update_confidence` could produce unbounded drift.

**After**: Input validation rejects non-finite or out-of-range values.
`update_confidence` clamps both input `rating` and output to `[0.0, 1.0]`.

### 4. A/B test winner not validated (MEDIUM → Fixed)

**Before**: `declare_winner` accepted any arbitrary string as winner.

**After**: Winner must be `"A"` or `"B"`. Returns `InvalidParameterName` error
otherwise.

### 5. Short UUID IDs — collision risk (MEDIUM → Fixed)

**Before**: All entity IDs used 8-char UUID prefix (32 bits of entropy) —
unacceptable collision probability at scale.

**After**: All ID generation now uses full UUIDv4 (122 bits of entropy).

### 6. SQL Injection — Not Vulnerable

All SQL queries use parameterized `?N` placeholders via `rusqlite::params!`.
Dynamic query building in `list_prompts` and `search_skills` concatenates only
parameter position numbers, never user input. **No action needed.**

### 7. Prompt injection via template variables (Accepted Risk)

Template variables are substituted verbatim into prompts by design. This is the
intended behavior for a prompt management system — the rendered prompt is then
passed to an LLM. Sanitization at the template layer would break legitimate
use cases. Mitigation is the responsibility of the calling layer (the daemon's
agent runtime applies output guards).

### 8. No auth on HTTP routes (Accepted — platform-level concern)

Routes are mounted inside the daemon process which handles auth at the gateway
layer. Adding per-route auth here would duplicate the platform middleware.
Documented as an architectural invariant.

### 9. Unpinned CI workflow refs (LOW — Noted)

Reusable workflows reference `@main` branch. This is an acceptable trade-off
for a single-owner org with branch protection. Pinning to SHA would be
recommended for external contributors.

## Consequences

- All HTTP error responses are now opaque to clients
- Input payloads are validated before hitting the database
- Entity IDs are cryptographically unique
- Confidence values are bounded and deterministic
- A/B test logic is constrained to valid states
