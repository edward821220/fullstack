import { redirect } from "next/navigation";
import { getServerSession } from "next-auth";
import { DashboardShell } from "@/components/features/dashboard-shell";
import { authOptions } from "@/lib/auth/config";

export default async function DashboardLayout({ children }: { children: React.ReactNode }) {
  const session = await getServerSession(authOptions);

  if (!session) {
    redirect("/api/auth/signin/oidc?callbackUrl=/dashboard");
  }

  return (
    <DashboardShell
      userEmail={session.user?.email ?? "operator@bank.local"}
      userName={session.user?.name ?? "Operator"}
    >
      {children}
    </DashboardShell>
  );
}
