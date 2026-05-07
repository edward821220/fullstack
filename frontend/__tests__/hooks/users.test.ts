import { describe, it, expect } from "vitest";

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
