import { beforeEach, describe, expect, it, vi } from "vitest";

const cookiesMock = { getAll: vi.fn() };
const getTokenMock = vi.fn();

vi.mock("next/headers", () => ({
  cookies: vi.fn(async () => cookiesMock),
}));

vi.mock("next-auth/jwt", () => ({
  getToken: getTokenMock,
}));

describe("server API auth helpers", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it("passes the Next.js cookie store to next-auth getToken", async () => {
    getTokenMock.mockResolvedValue({ accessToken: "session-token" });

    const { getServerAccessToken } = await import("@/lib/api/server");
    const accessToken = await getServerAccessToken();

    expect(accessToken).toBe("session-token");
    expect(getTokenMock).toHaveBeenCalledWith(
      expect.objectContaining({
        req: expect.objectContaining({
          cookies: cookiesMock,
        }),
      }),
    );
  });
});
