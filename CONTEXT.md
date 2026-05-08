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

- **Infra** — The shared infrastructure crate. Holds code used by multiple server binaries (`server`, `grpc-server`): telemetry initialization (`init_tracing`), health checkers (`DbHealthChecker`, `AlwaysHealthy`), and audit exporters (`NoopExporter`, `SyslogExporter`, `OtelLogsExporter`). Depends on `config` and `svc`.
- **Combined server** — The `server` binary. Owns the main bootstrap flow for REST and may also co-host gRPC when `grpc.enabled: true`.
- **Standalone gRPC server** — The `grpc-server` binary. Runs only the gRPC service and reuses `infra` for shared runtime concerns.
- **AppState** — The shared state for the HTTP server. Contains `UserService`, the health checker, `OidcValidator`, `ProvisioningPolicy`, and `AuditService`.
- **ProblemResponse** — The RFC 9457 Problem Details error response format used across all HTTP APIs.
- **AuditEvent** — Security-relevant events emitted by `svc` and `server` layers. Lives in `svc::audit` so business logic can record audits without knowing HTTP.
- **AuditExporter** — Strategy-pattern trait in `svc::audit`. Implementations (`NoopExporter`, `SyslogExporter`, `OtelLogsExporter`) live in `infra::audit`. `infra::create_audit_exporter()` is the shared factory used by both REST and gRPC servers.
- **AuditService** — Async channel-based audit dispatcher in `svc::audit`. Held in `AppState` and used by handlers and middleware. Not injected into `UserService`.
- **AuditEventProxy / AuditEventCtx** — Serializable structs in `infra::audit` used by exporters to output structured JSON with optional HTTP context.
- **AppError** — The HTTP error seam in `server::error`. Wraps `svc::Error` and is the only place where service failures are translated into status codes plus `ProblemResponse`.

## Transaction & Type Erasure

- **Transaction** — The transaction seam. `commit()` and `rollback()` methods. Implemented by `PgTransaction` (wraps `sqlx::Transaction<'static>`) and `MssqlTransaction` (dedicated `tiberius::Client` TCP connection). Auto-rollback on drop for both adapters.
- **AnyUserRepo** — Type-erased enum over `PostgresUserRepo` and `MssqlUserRepo`. Returned by `repo::connect()` and stored in `AppState` to keep server code monomorphic.
- **AnyTransaction** — Type-erased enum over `PgTransaction` and `MssqlTransaction`. Passed into `_in_tx` methods on `UserRepo`.

## Optimistic Locking

- **version** — The `users` table column used for optimistic locking. Incremented on every successful `UPDATE`. `UPDATE` statements include `WHERE id = ? AND version = ?`; a mismatch produces `repo::Error::Conflict` mapped to HTTP `409`.

## Config

- **TlsConfig** — `server.tls` block: `enabled`, `cert_path`, `key_path`. Production defaults to enabled; local dev opts out via `local.yaml`.
- **AuditConfig** — `audit.exporter` setting (`none` | `syslog` | `otel-logs`). `none` maps to `NoopExporter`.

## Test Concepts

- **MockUserRepo** — The canonical test double for `UserRepo`. Provided by the `repo` crate under the `test-helpers` feature. Maintains in-memory state (via `Arc<Mutex<...>>`) and optional call-spy vectors for orchestration tests.
- **MockTransaction** — In-memory test double for `Transaction`. Supports staged `commit()` and `rollback()` with `committed`/`rolled_back` flags for verifying transaction boundaries in unit tests.

## Relationships

- The **Combined server** and **Standalone gRPC server** both depend on **Infra** for telemetry, health checks, and audit exporter construction.
- **AppError** translates **svc::Error** into **ProblemResponse** for HTTP clients.
- **AnyUserRepo** and **AnyTransaction** keep **AppState** and bootstrap code concrete while **UserService** still depends on the **UserRepo** seam.
- **AuditService** dispatches **AuditEvent** values through an **AuditExporter** selected by **AuditConfig**.

## Example dialogue

> **Dev:** "When I say 'the server', do I mean the REST crate or the gRPC binary?"
> **Domain expert:** "Use **Combined server** for the `server` binary that owns bootstrap and REST, and **Standalone gRPC server** for `grpc-server`. Both share **Infra**."

## Flagged ambiguities

- "server" was being used for both the `server` crate and the standalone `grpc-server` runtime — resolved: use **Combined server** for the main binary and **Standalone gRPC server** for the gRPC-only binary.
