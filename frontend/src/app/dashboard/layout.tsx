import { redirect } from "next/navigation";
import { getServerSession } from "next-auth";
import { authOptions } from "@/lib/auth/config";

export default async function DashboardLayout({ children }: { children: React.ReactNode }) {
  const session = await getServerSession(authOptions);

  if (!session) {
    redirect("/api/auth/signin/oidc?callbackUrl=/dashboard");
  }

  return (
    <div className="flex min-h-screen flex-col">
      <header className="flex items-center justify-between border-b px-6 py-3">
        <span className="font-semibold">Dashboard</span>
        <span className="text-sm text-gray-500">{session.user?.name}</span>
      </header>
      <main className="flex-1 p-6">{children}</main>
    </div>
  );
}
