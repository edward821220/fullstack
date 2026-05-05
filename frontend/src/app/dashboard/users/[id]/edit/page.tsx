import { notFound } from "next/navigation";
import { getServerSession } from "next-auth";
import { redirect } from "next/navigation";
import { UserForm } from "@/components/features/users/user-form";
import { authOptions } from "@/lib/auth/config";
import { getUser } from "@/lib/api/users/server";

export default async function EditUserPage({ params }: { params: Promise<{ id: string }> }) {
  const session = await getServerSession(authOptions);
  if (!session) {
    redirect("/api/auth/signin/oidc?callbackUrl=/dashboard");
  }

  const { id } = await params;
  const accessToken = session.accessToken ?? "";
  const user = await getUser(id, accessToken).catch(() => null);
  if (!user) {
    notFound();
  }

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold">Edit User</h1>
      <UserForm mode="edit" user={user} />
    </div>
  );
}
