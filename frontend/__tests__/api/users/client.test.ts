import { describe, it, expect, vi, beforeEach } from "vitest";

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

import { get as clientGet, post as clientPost, del as clientDel } from "@/lib/api/client";
import { getUsersPage, getUser, createUser, deleteUser } from "@/lib/api/users/client";

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

const mockPaginated = {
  data: [],
  total: 0,
  page: 1,
  per_page: 20,
};

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
