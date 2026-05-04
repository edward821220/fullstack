import useSWR from "swr";
import { getUsersPage } from "@/lib/api/users";
import type { PaginatedUserResponse } from "@/schemas";

export function useUsers(page = 1, perPage = 20, fallbackData?: PaginatedUserResponse) {
  return useSWR<PaginatedUserResponse>(
    `/users?page=${page}&per_page=${perPage}`,
    () => getUsersPage(page, perPage),
    {
      fallbackData,
      keepPreviousData: true,
    },
  );
}
