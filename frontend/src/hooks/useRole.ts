import { useSession } from "next-auth/react";

export function useRole() {
  const { data: session } = useSession();
  const role = (session?.user as { role?: string } | undefined)?.role ?? "user";
  const isAdmin = role === "admin";
  const isManager = role === "manager" || isAdmin;
  return { role, isAdmin, isManager };
}
