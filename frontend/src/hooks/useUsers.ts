import useSWR from "swr";
import apiClient from "@/lib/api/client";
import type { PaginatedUsersResponse } from "@/lib/api/types";

const fetcher = async (url: string) => {
  const res = await apiClient.get<PaginatedUsersResponse>(url);
  return res.data;
};

export function useUsers(page = 1, perPage = 20) {
  return useSWR<PaginatedUsersResponse>(`/users?page=${page}&per_page=${perPage}`, fetcher);
}
