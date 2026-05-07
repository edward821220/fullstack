import { describe, it, expect, vi } from "vitest";

import { getUsersPage as getUsersPageServer } from "@/lib/api/users/server";

const mockPaginated = {
  data: [],
  total: 0,
  page: 1,
  per_page: 20,
};

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

    await expect(getUsersPageServer(1, 8, "dummy-token")).rejects.toThrow(
      "Server GET /users?page=1&per_page=8 failed (404): Not found",
    );
  });
});
