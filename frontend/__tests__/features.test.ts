import { describe, it, expect } from "vitest";

// ── API type contract tests (derived from generated schema) ─────────────────

import type {
  User,
  CreateUserRequest,
  UpdateUserRequest,
  PaginatedUserResponse,
  ErrorResponse,
} from "@/lib/api/types";

describe("API types — generated from OpenAPI spec", () => {
  it("User type should include required fields", () => {
    // TypeScript compile-time check; runtime assertion on structure
    const user: User = {
      id: "00000000-0000-0000-0000-000000000001",
      email: "admin@example.com",
      display_name: "Admin",
      role: "admin",
      email_verified: true,
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    };
    expect(user.id).toBeDefined();
    expect(user.email).toBe("admin@example.com");
    expect(user.role).toBe("admin");
  });

  it("CreateUserRequest should require email and display_name", () => {
    const req: CreateUserRequest = {
      email: "new@example.com",
      display_name: "New User",
    };
    expect(req.email).toBe("new@example.com");
    expect(req.display_name).toBe("New User");
  });

  it("UpdateUserRequest should support optional display_name", () => {
    const withName: UpdateUserRequest = { display_name: "Updated" };
    const withoutName: UpdateUserRequest = {};
    expect(withName.display_name).toBe("Updated");
    expect(withoutName.display_name).toBeUndefined();
  });

  it("PaginatedUserResponse should include data array and pagination fields", () => {
    const page: PaginatedUserResponse = {
      data: [],
      total: 0,
      page: 1,
      per_page: 20,
    };
    expect(Array.isArray(page.data)).toBe(true);
    expect(page.total).toBe(0);
    expect(page.page).toBe(1);
    expect(page.per_page).toBe(20);
  });

  it("ErrorResponse should include Problem Details fields", () => {
    const err: ErrorResponse = {
      type: "about:blank",
      title: "Unauthorized",
      status: 401,
      detail: "Missing Bearer token",
    };
    expect(err.type).toBe("about:blank");
    expect(err.status).toBe(401);
    expect(err.title).toBe("Unauthorized");
  });
});

// ── Zod schema validation tests ─────────────────────────────────────────────

import { createUserSchema, updateUserSchema } from "@/schemas";

describe("Zod schemas", () => {
  it("createUserSchema should accept valid input", () => {
    const result = createUserSchema.safeParse({
      email: "test@example.com",
      display_name: "Test User",
    });
    expect(result.success).toBe(true);
  });

  it("createUserSchema should reject invalid email", () => {
    const result = createUserSchema.safeParse({
      email: "not-an-email",
      display_name: "Test",
    });
    expect(result.success).toBe(false);
  });

  it("createUserSchema should reject empty display_name", () => {
    const result = createUserSchema.safeParse({
      email: "test@example.com",
      display_name: "",
    });
    expect(result.success).toBe(false);
  });

  it("createUserSchema should reject display_name exceeding 100 chars", () => {
    const result = createUserSchema.safeParse({
      email: "test@example.com",
      display_name: "a".repeat(101),
    });
    expect(result.success).toBe(false);
  });

  it("updateUserSchema should accept partial update", () => {
    const result = updateUserSchema.safeParse({ display_name: "Updated" });
    expect(result.success).toBe(true);
  });

  it("updateUserSchema should accept empty object (no fields to update)", () => {
    const result = updateUserSchema.safeParse({});
    expect(result.success).toBe(true);
  });

  it("updateUserSchema should reject empty display_name when provided", () => {
    const result = updateUserSchema.safeParse({ display_name: "" });
    expect(result.success).toBe(false);
  });
});

// ── Auth config structure tests ─────────────────────────────────────────────

import { authOptions } from "@/lib/auth/config";

describe("Auth config", () => {
  it("should have a single OIDC provider", () => {
    const providers = authOptions.providers;
    expect(providers).toHaveLength(1);
    expect(providers[0].id).toBe("oidc");
    expect(providers[0].type).toBe("oauth");
  });

  it("should use JWT session strategy", () => {
    expect(authOptions.session?.strategy).toBe("jwt");
  });

  it("should have sign-in page configured as /login", () => {
    expect(authOptions.pages?.signIn).toBe("/login");
  });

  it("OIDC provider should request openid email profile scope", () => {
    const provider = authOptions.providers[0];
    const p = provider as { authorization?: { params?: { scope?: string } } };
    expect(p.authorization?.params?.scope).toContain("openid");
    expect(p.authorization?.params?.scope).toContain("email");
    expect(p.authorization?.params?.scope).toContain("profile");
  });

  it("session strategy should be JWT", () => {
    const opts = authOptions as { session?: { strategy?: string } };
    expect(opts.session?.strategy).toBe("jwt");
  });

  it("callbacks should be defined", () => {
    const opts = authOptions as {
      callbacks?: { jwt?: unknown; session?: unknown };
    };
    expect(opts.callbacks).toBeDefined();
    expect(opts.callbacks?.jwt).toBeDefined();
    expect(opts.callbacks?.session).toBeDefined();
  });
});
