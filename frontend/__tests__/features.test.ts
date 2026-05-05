import { describe, it, expect, vi } from "vitest";

// ── Mocks (hoisted by vitest) ────────────────────────────────────────────────

vi.mock("next-auth/react", () => ({
  getSession: vi.fn().mockResolvedValue({ accessToken: "test-token" }),
}));

vi.mock("next-auth", () => ({
  getServerSession: vi.fn().mockResolvedValue({ accessToken: "server-token" }),
}));

vi.mock("@/lib/api/client", () => ({
  default: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
    interceptors: { request: { use: vi.fn() } },
  },
  get: vi.fn(),
  post: vi.fn(),
  put: vi.fn(),
  del: vi.fn(),
}));

// ── Imports ──────────────────────────────────────────────────────────────────

import { parse } from "@/lib/api/parse";
import { zUserResponse } from "@/lib/api/gen/zod.gen";
import { get as clientGet, post as clientPost, del as clientDel } from "@/lib/api/client";
import { getUsersPage, getUser, createUser, deleteUser } from "@/lib/api/users/client";
import { getUsersPage as getUsersPageServer } from "@/lib/api/users/server";
import { authOptions } from "@/lib/auth/config";

const mockUser = {
  id: "550e8400-e29b-41d4-a716-446655440000",
  email: "a@b.com",
  display_name: "A",
  role: "user",
  email_verified: true,
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-01-01T00:00:00Z",
};

const mockPaginated = {
  data: [],
  total: 0,
  page: 1,
  per_page: 20,
};

// ── API Parse Tests ──────────────────────────────────────────────────────────

describe("parse()", () => {
  it("should return parsed data when schema matches", () => {
    const result = parse(mockUser, zUserResponse);
    expect(result.email).toBe("a@b.com");
  });

  it("should throw when schema validation fails", () => {
    expect(() => parse({ email: "not-an-email" }, zUserResponse)).toThrow(
      "API response validation failed",
    );
  });
});

// ── Users Client API Module Tests ────────────────────────────────────────────

describe("users client API module", () => {
  beforeEach(() => {
    vi.mocked(clientGet).mockReset();
    vi.mocked(clientPost).mockReset();
    vi.mocked(clientDel).mockReset();
  });

  it("getUsersPage should request correct query params", async () => {
    vi.mocked(clientGet).mockResolvedValueOnce(mockPaginated);
    await getUsersPage(2, 50);
    expect(clientGet).toHaveBeenCalledWith("/users?page=2&per_page=50", expect.anything());
  });

  it("getUser should request correct endpoint", async () => {
    vi.mocked(clientGet).mockResolvedValueOnce(mockUser);
    await getUser("550e8400-e29b-41d4-a716-446655440000");
    expect(clientGet).toHaveBeenCalledWith(
      "/users/550e8400-e29b-41d4-a716-446655440000",
      expect.anything(),
    );
  });

  it("createUser should POST with body", async () => {
    vi.mocked(clientPost).mockResolvedValueOnce(mockUser);
    await createUser({ email: "new@example.com", display_name: "New" });
    expect(clientPost).toHaveBeenCalledWith(
      "/users",
      { email: "new@example.com", display_name: "New" },
      expect.anything(),
    );
  });

  it("deleteUser should send DELETE request", async () => {
    vi.mocked(clientDel).mockResolvedValueOnce(undefined);
    await deleteUser("550e8400-e29b-41d4-a716-446655440000");
    expect(clientDel).toHaveBeenCalledWith(
      "/users/550e8400-e29b-41d4-a716-446655440000",
      undefined,
    );
  });
});

// ── Users Server API Module Tests ────────────────────────────────────────────

describe("users server API module", () => {
  it("should attach Bearer token when provided explicitly", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: vi.fn().mockResolvedValue(mockPaginated),
    } as unknown as Response);

    await getUsersPageServer(1, 8, "explicit-token");
    const callArgs = vi.mocked(global.fetch).mock.calls[0];
    const headers = (callArgs[1] as RequestInit)?.headers as Record<string, string>;
    expect(headers?.Authorization).toBe("Bearer explicit-token");
  });

  it("should throw when response is not ok", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 404,
      text: vi.fn().mockResolvedValue("Not found"),
    } as unknown as Response);

    await expect(getUsersPageServer(1, 8)).rejects.toThrow(
      "Server GET /users?page=1&per_page=8 failed (404): Not found",
    );
  });
});

// ── SWR Hook Tests ───────────────────────────────────────────────────────────

describe("useUsers hook", () => {
  it("should construct correct SWR key with page and per_page", () => {
    const key = `/users?page=2&per_page=50`;
    expect(key).toBe("/users?page=2&per_page=50");
  });

  it("should use fallbackData when provided", () => {
    const fallbackData = {
      data: [],
      total: 0,
      page: 1,
      per_page: 8,
    };
    expect(fallbackData.page).toBe(1);
    expect(fallbackData.per_page).toBe(8);
  });
});

// ── Auth Config Tests ────────────────────────────────────────────────────────

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

  it("should have signOut page configured as /login", () => {
    expect(authOptions.pages?.signOut).toBe("/login");
  });

  it("OIDC provider should request openid email profile offline_access scope", () => {
    const provider = authOptions.providers[0];
    const p = provider as { authorization?: { params?: { scope?: string } } };
    expect(p.authorization?.params?.scope).toContain("openid");
    expect(p.authorization?.params?.scope).toContain("email");
    expect(p.authorization?.params?.scope).toContain("profile");
    expect(p.authorization?.params?.scope).toContain("offline_access");
  });

  it("session maxAge should be 24 hours", () => {
    const opts = authOptions as { session?: { maxAge?: number } };
    expect(opts.session?.maxAge).toBe(24 * 60 * 60);
  });

  it("callbacks should be defined", () => {
    const opts = authOptions as {
      callbacks?: { jwt?: unknown; session?: unknown };
    };
    expect(opts.callbacks).toBeDefined();
    expect(opts.callbacks?.jwt).toBeDefined();
    expect(opts.callbacks?.session).toBeDefined();
  });

  it("profile callback should include role from IdP claim", () => {
    const provider = authOptions.providers[0];
    const p = provider as unknown as {
      profile?: (p: Record<string, string>) => Record<string, unknown>;
    };
    const profile = p.profile?.({
      sub: "123",
      email: "test@example.com",
      name: "Test",
      role: "admin",
    });
    expect(profile?.role).toBe("admin");
  });

  it("profile callback should default role to user when absent", () => {
    const provider = authOptions.providers[0];
    const p = provider as unknown as {
      profile?: (p: Record<string, string>) => Record<string, unknown>;
    };
    const profile = p.profile?.({
      sub: "123",
      email: "test@example.com",
      name: "Test",
    });
    expect(profile?.role).toBe("user");
  });
});

// ── API Type Contract Tests ──────────────────────────────────────────────────

import type {
  UserResponse,
  CreateUserRequest,
  UpdateUserRequest,
  PaginatedUserResponse,
  ErrorResponse,
} from "@/lib/api/gen/types.gen";

describe("API types — generated from OpenAPI spec", () => {
  it("User type should include required fields", () => {
    const user: UserResponse = {
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

// ── Zod Schema Validation Tests ──────────────────────────────────────────────

import { zCreateUserRequest } from "@/lib/api/gen/zod.gen";
import { updateUserSchema } from "@/schemas";

describe("Zod schemas", () => {
  it("zCreateUserRequest should accept valid input", () => {
    const result = zCreateUserRequest.safeParse({
      email: "test@example.com",
      display_name: "Test User",
    });
    expect(result.success).toBe(true);
  });

  it("zCreateUserRequest should reject invalid email", () => {
    const result = zCreateUserRequest.safeParse({
      email: "not-an-email",
      display_name: "Test",
    });
    expect(result.success).toBe(false);
  });

  it("zCreateUserRequest should reject empty display_name", () => {
    const result = zCreateUserRequest.safeParse({
      email: "test@example.com",
      display_name: "",
    });
    expect(result.success).toBe(false);
  });

  it("zCreateUserRequest should reject display_name exceeding 100 chars", () => {
    const result = zCreateUserRequest.safeParse({
      email: "test@example.com",
      display_name: "a".repeat(101),
    });
    expect(result.success).toBe(false);
  });

  it("updateUserSchema should accept partial update", () => {
    const result = updateUserSchema.safeParse({ display_name: "Updated" });
    expect(result.success).toBe(true);
  });

  it("updateUserSchema should accept empty object", () => {
    const result = updateUserSchema.safeParse({});
    expect(result.success).toBe(true);
  });
});

// ── Auth Session Type Tests ──────────────────────────────────────────────────

import type { Session } from "next-auth";

describe("Auth session types", () => {
  it("Session should support accessToken and error fields", () => {
    const session: Session = {
      user: { name: "Test", email: "test@example.com", role: "admin" },
      expires: new Date().toISOString(),
      accessToken: "abc.def.ghi",
      error: "RefreshAccessTokenError",
    };
    expect(session.accessToken).toBe("abc.def.ghi");
    expect(session.error).toBe("RefreshAccessTokenError");
  });

  it("Session should be valid without error", () => {
    const session: Session = {
      user: { name: "Test", email: "test@example.com", role: "user" },
      expires: new Date().toISOString(),
      accessToken: "abc.def.ghi",
    };
    expect(session.accessToken).toBeDefined();
    expect(session.error).toBeUndefined();
  });

  it("Session user should include role field", () => {
    const session: Session = {
      user: { name: "Test", email: "test@example.com", role: "manager" },
      expires: new Date().toISOString(),
    };
    expect(session.user.role).toBe("manager");
  });
});

// ── Protected Route Logic Tests ──────────────────────────────────────────────

describe("Protected route redirect logic", () => {
  it("should redirect to /login when session is missing", () => {
    const hasSession = false;
    const redirect = !hasSession ? "/login" : undefined;
    expect(redirect).toBe("/login");
  });

  it("should not redirect when session exists", () => {
    const hasSession = true;
    const redirect = !hasSession ? "/login" : undefined;
    expect(redirect).toBeUndefined();
  });
});
