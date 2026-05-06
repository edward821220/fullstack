import { describe, it, expect } from "vitest";

import { parse } from "@/lib/api/parse";
import { zUserResponse } from "@/lib/api/gen/zod.gen";

const mockUser = {
  id: "550e8400-e29b-41d4-a716-446655440000",
  email: "a@b.com",
  display_name: "A",
  role: "user",
  email_verified: true,
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-01-01T00:00:00Z",
  version: 1,
};

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
