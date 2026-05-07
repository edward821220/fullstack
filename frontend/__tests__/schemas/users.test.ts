import { describe, it, expect } from "vitest";

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
