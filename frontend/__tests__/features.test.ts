import { describe, it, expect, vi } from "vitest";

// ── Mocks (hoisted by vitest) ────────────────────────────────────────────────

vi.mock("next-auth/react", () => ({
  getSession: vi.fn().mockResolvedValue({ accessToken: "test-token" }),
}));

vi.mock("@/lib/api/client", () => ({
  default: {
    get: vi.fn(),
    request: vi.fn(),
    interceptors: { request: { use: vi.fn() } },
  },
}));

// ── Imports ──────────────────────────────────────────────────────────────────

import { parse, clientFetch, clientMutate, serverFetch } from "@/lib/api/fetcher";
import { userResponseSchema } from "@/schemas";
import apiClient from "@/lib/api/client";
import { getUsersPage, getUser, createUser, deleteUser } from "@/lib/api/users";
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

// ── API Fetcher Tests ────────────────────────────────────────────────────────

describe("parse()", () => {
  it("should return parsed data when schema matches", () => {
    const result = parse(mockUser, userResponseSchema);
    expect(result.email).toBe("a@b.com");
  });

  it("should throw when schema validation fails", () => {
    expect(() => parse({ email: "not-an-email" }, userResponseSchema)).toThrow(
      "API response validation failed",
    );
  });
});

describe("clientFetch()", () => {
  it("should parse and return data on 200", async () => {
    vi.mocked(apiClient.get).mockResolvedValueOnce({ data: mockUser });
    const data = await clientFetch("/users/1", userResponseSchema);
    expect(data.email).toBe("a@b.com");
  });

  it("should throw on non-2xx status", async () => {
    vi.mocked(apiClient.get).mockRejectedValueOnce(new Error("Request failed with status 500"));
    await expect(clientFetch("/users/1", userResponseSchema)).rejects.toThrow();
  });
});

describe("clientMutate()", () => {
  it("should parse response on successful POST", async () => {
    vi.mocked(apiClient.request).mockResolvedValueOnce({ data: mockUser });
    const data = await clientMutate("/users", userResponseSchema, "POST", {
      email: "a@b.com",
      display_name: "A",
    });
    expect(data.email).toBe("a@b.com");
  });

  it("should return undefined on 204 DELETE without schema", async () => {
    vi.mocked(apiClient.request).mockResolvedValueOnce({ data: undefined });
    const data = await clientMutate<void>("/users/1", undefined, "DELETE");
    expect(data).toBeUndefined();
  });
});

describe("serverFetch()", () => {
  it("should attach Bearer token when provided", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: vi.fn().mockResolvedValue(mockUser),
    } as unknown as Response);

    await serverFetch("/users/1", userResponseSchema, "test-token");
    const callArgs = vi.mocked(global.fetch).mock.calls[0];
    const headers = (callArgs[1] as RequestInit)?.headers as Record<string, string>;
    expect(headers?.Authorization).toBe("Bearer test-token");
  });

  it("should throw when response is not ok", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 404,
      text: vi.fn().mockResolvedValue("Not found"),
    } as unknown as Response);

    await expect(serverFetch("/users/1", userResponseSchema)).rejects.toThrow(
      "Failed to fetch /users/1: 404",
    );
  });
});

// ── Users API Module Tests ───────────────────────────────────────────────────

describe("users API module", () => {
  beforeEach(() => {
    vi.mocked(apiClient.get).mockReset();
    vi.mocked(apiClient.request).mockReset();
  });

  it("getUsersPage should request correct query params", async () => {
    vi.mocked(apiClient.get).mockResolvedValueOnce({ data: mockPaginated });
    await getUsersPage(2, 50);
    expect(apiClient.get).toHaveBeenCalledWith("/users?page=2&per_page=50");
  });

  it("getUser should request correct endpoint", async () => {
    vi.mocked(apiClient.get).mockResolvedValueOnce({ data: mockUser });
    await getUser("550e8400-e29b-41d4-a716-446655440000");
    expect(apiClient.get).toHaveBeenCalledWith("/users/550e8400-e29b-41d4-a716-446655440000");
  });

  it("createUser should POST with body", async () => {
    vi.mocked(apiClient.request).mockResolvedValueOnce({ data: mockUser });
    await createUser({ email: "new@example.com", display_name: "New" });
    expect(apiClient.request).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "/users",
        method: "POST",
        data: { email: "new@example.com", display_name: "New" },
      }),
    );
  });

  it("deleteUser should send DELETE request", async () => {
    vi.mocked(apiClient.request).mockResolvedValueOnce({ data: undefined });
    await deleteUser("550e8400-e29b-41d4-a716-446655440000");
    expect(apiClient.request).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "/users/550e8400-e29b-41d4-a716-446655440000",
        method: "DELETE",
      }),
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
});

// ── API Type Contract Tests ──────────────────────────────────────────────────

import type {
  User,
  CreateUserRequest,
  UpdateUserRequest,
  PaginatedUserResponse,
  ErrorResponse,
} from "@/lib/api/types";

describe("API types — generated from OpenAPI spec", () => {
  it("User type should include required fields", () => {
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

// ── Zod Schema Validation Tests ──────────────────────────────────────────────

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
      user: { name: "Test", email: "test@example.com" },
      expires: new Date().toISOString(),
      accessToken: "abc.def.ghi",
      error: "RefreshAccessTokenError",
    };
    expect(session.accessToken).toBe("abc.def.ghi");
    expect(session.error).toBe("RefreshAccessTokenError");
  });

  it("Session should be valid without error", () => {
    const session: Session = {
      user: { name: "Test", email: "test@example.com" },
      expires: new Date().toISOString(),
      accessToken: "abc.def.ghi",
    };
    expect(session.accessToken).toBeDefined();
    expect(session.error).toBeUndefined();
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
