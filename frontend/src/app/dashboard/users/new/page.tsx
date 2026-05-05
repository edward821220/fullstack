import { getServerSession } from "next-auth";
import { redirect } from "next/navigation";
import { UserForm } from "@/components/features/users/user-form";
import { authOptions } from "@/lib/auth/config";

export default async function NewUserPage() {
  const session = await getServerSession(authOptions);
  if (!session) {
    redirect("/api/auth/signin/oidc?callbackUrl=/dashboard/users/new");
  }

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold">Create User</h1>
      <UserForm mode="create" />
    </div>
  );
}
