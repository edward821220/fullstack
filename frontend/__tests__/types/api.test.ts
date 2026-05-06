import { describe, it, expect } from "vitest";

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
      version: 1,
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
