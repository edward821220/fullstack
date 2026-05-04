import { getServerSession } from "next-auth";
import { redirect } from "next/navigation";
import { DashboardOverview } from "@/components/features/users/dashboard-overview";
import { UsersTable } from "@/components/features/users/users-table";
import { fetchUsersPage } from "@/lib/api/users";
import { authOptions } from "@/lib/auth/config";

export default async function DashboardPage() {
  const session = await getServerSession(authOptions);

  if (!session) {
    redirect("/api/auth/signin/oidc?callbackUrl=/dashboard");
  }

  const initialUsers = await fetchUsersPage(session.accessToken, 1, 8);

  return (
    <div className="space-y-8">
      <DashboardOverview
        userEmail={session.user?.email ?? "operator@bank.local"}
        userName={session.user?.name ?? "Operator"}
      />
      <UsersTable initialData={initialUsers} />
    </div>
  );
}
