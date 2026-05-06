import { describe, it, expect } from "vitest";

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
