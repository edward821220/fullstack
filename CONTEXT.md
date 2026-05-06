# Domain Glossary

This document defines the domain language for the fullstack-template backend.
When naming modules, seams, or concepts during architecture work, use these terms exactly.
Keep setup commands and implementation checklists in `README.md`, `AGENTS.md`, or `docs/how-to-add-feature.md`; this file is only for shared vocabulary and invariants.

## Core Entities

- **User** — A registered person in the system. The central domain entity.
- **UserIdentity** — A mapping between an external Identity Provider (IdP) and a local User. Links an OIDC `sub` + `issuer` to a `User` record.

## Application Layers

- **UserRepo** — The persistence seam. Defines all storage operations for `User` and `UserIdentity`. Implemented by `PostgresUserRepo` and `MssqlUserRepo`.
- **UserService** — The business-logic module. Orchestrates CRUD and JIT provisioning. Depends on the `UserRepo` seam, not on a concrete adapter.
- **ProvisioningPolicy** — The policy module that decides how an incoming OIDC user is mapped to a local `User`: email-domain whitelist, role resolution from OIDC claims, and default-role fallback.
- **OidcValidator** — The authentication module. Validates JWT tokens against JWKS, extracts claims, and drives JIT provisioning via `UserService`.

## Auth & Authorization

- **AuthUser** — The authenticated user context attached to a request after successful OIDC validation.
- **Role** — The authorization hierarchy: `Admin` > `Manager` > `User`.
- **authorize_role** — The pure function that checks whether a user's role satisfies a minimum required role.

## Shared Infrastructure

- **AppState** — The application's shared state held by the HTTP/gRPC servers. Contains `UserService`, `OidcValidator`, `ProvisioningPolicy`, and `AuditService`.
- **ProblemResponse** — The RFC 9457 Problem Details error response format used across all HTTP APIs.
- **AuditEvent** — Security-relevant events emitted by `svc` and `server` layers. Lives in `svc::audit` so business logic can record audits without knowing HTTP.
- **AuditExporter** — Strategy-pattern trait in `svc::audit`. Implementations (e.g. `StdoutExporter`) live in `server::audit` and may enrich events with HTTP context before export.
- **AuditService** — Async channel-based audit dispatcher in `svc::audit`. Held in `AppState` and injectable into `UserService`.
- **AuditEventProxy / AuditEventCtx** — Serializable structs in `server::audit` used by exporters to output structured JSON with optional HTTP context.

## Transaction & Type Erasure

- **Transaction** — The transaction seam. `commit()` and `rollback()` methods. Implemented by `PgTransaction` (wraps `sqlx::Transaction<'static>`) and `MssqlTransaction` (dedicated `tiberius::Client` TCP connection). Auto-rollback on drop for both adapters.
- **AnyUserRepo** — Type-erased enum over `PostgresUserRepo` and `MssqlUserRepo`. Returned by `repo::connect()` and stored in `AppState` to keep server code monomorphic.
- **AnyTransaction** — Type-erased enum over `PgTransaction` and `MssqlTransaction`. Passed into `_in_tx` methods on `UserRepo`.

## Optimistic Locking

- **version** — The `users` table column used for optimistic locking. Incremented on every successful `UPDATE`. `UPDATE` statements include `WHERE id = ? AND version = ?`; a mismatch produces `repo::Error::Conflict` mapped to HTTP `409`.

## Config

- **TlsConfig** — `server.tls` block: `enabled`, `cert_path`, `key_path`. Production defaults to enabled; local dev opts out via `local.yaml`.
- **AuditConfig** — `audit.exporter` setting (`stdout` | `syslog` | `otel-logs`).

## Test Concepts

- **MockUserRepo** — The canonical test double for `UserRepo`. Provided by the `repo` crate under the `test-helpers` feature. Maintains in-memory state (via `Arc<Mutex<...>>`) and optional call-spy vectors for orchestration tests.
- **MockTransaction** — In-memory test double for `Transaction`. Supports staged `commit()` and `rollback()` with `committed`/`rolled_back` flags for verifying transaction boundaries in unit tests.
