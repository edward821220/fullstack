import { useSession } from "next-auth/react";

export function useRole() {
  const { data: session } = useSession();
  const role = session?.user?.role ?? "user";
  const isAdmin = role === "admin";
  const isManager = role === "manager" || isAdmin;
  return { role, isAdmin, isManager };
}
