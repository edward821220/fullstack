# Domain Glossary

This document defines the domain language for the fullstack-template backend.
When naming modules, seams, or concepts during architecture work, use these terms exactly.

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

- **AppState** — The application's shared state held by the HTTP/gRPC servers. Contains `UserService`, `OidcValidator`, and `ProvisioningPolicy`.
- **ProblemResponse** — The RFC 9457 Problem Details error response format used across all HTTP APIs.
- **AuditEvent** — Security-relevant events (auth success/failure, role denied, user created/updated/deleted) emitted for observability.

## Test Concepts

- **MockUserRepo** — The canonical test double for `UserRepo`. Provided by the `repo` crate under the `test-helpers` feature. Maintains in-memory state and optional call-spy vectors for orchestration tests.
