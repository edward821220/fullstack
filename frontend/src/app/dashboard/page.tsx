import { getServerSession } from "next-auth";
import { redirect } from "next/navigation";
import { authOptions } from "@/lib/auth/config";
import { SignOutButton } from "./sign-out-button";

export default async function DashboardPage() {
  const session = await getServerSession(authOptions);

  if (!session) {
    redirect("/api/auth/signin/oidc?callbackUrl=/dashboard");
  }

  return (
    <div>
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Welcome, {session.user?.name ?? "User"}</h1>
        <SignOutButton />
      </div>
      <p className="mt-2 text-gray-500">This is your dashboard.</p>
    </div>
  );
}
