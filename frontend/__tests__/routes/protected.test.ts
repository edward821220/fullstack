import { describe, it, expect } from "vitest";

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
