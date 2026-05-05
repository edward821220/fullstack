import { notFound } from "next/navigation";
import { getServerSession } from "next-auth";
import { redirect } from "next/navigation";
import { UserForm } from "@/components/features/users/user-form";
import { authOptions } from "@/lib/auth/config";
import { serverFetch } from "@/lib/api/fetcher";
import { userResponseSchema } from "@/schemas";
import type { UserResponse } from "@/schemas";

async function fetchUser(accessToken: string, id: string): Promise<UserResponse | null> {
  try {
    return await serverFetch(`/users/${id}`, userResponseSchema, accessToken);
  } catch {
    return null;
  }
}

export default async function EditUserPage({ params }: { params: Promise<{ id: string }> }) {
  const session = await getServerSession(authOptions);
  if (!session) {
    redirect("/api/auth/signin/oidc?callbackUrl=/dashboard");
  }

  const { id } = await params;
  const accessToken = session.accessToken ?? "";
  const user = await fetchUser(accessToken, id);
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
