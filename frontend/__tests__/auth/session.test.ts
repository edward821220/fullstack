import { describe, it, expect } from "vitest";

import type { Session } from "next-auth";

describe("Auth session types", () => {
  it("Session should not expose accessToken to the client", () => {
    const session: Session = {
      user: { name: "Test", email: "test@example.com", role: "admin" },
      expires: new Date().toISOString(),
      error: "RefreshAccessTokenError",
    };
    // @ts-expect-error accessToken is intentionally removed from session payload
    expect(session.accessToken).toBeUndefined();
    expect(session.error).toBe("RefreshAccessTokenError");
  });

  it("Session should be valid without error", () => {
    const session: Session = {
      user: { name: "Test", email: "test@example.com", role: "user" },
      expires: new Date().toISOString(),
    };
    // @ts-expect-error accessToken is intentionally removed from session payload
    expect(session.accessToken).toBeUndefined();
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
