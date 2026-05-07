import useSWR from "swr";
import { getUsersPage } from "@/lib/api/users/client";
import type { PaginatedUserResponse } from "@/lib/api/gen/types.gen";

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
