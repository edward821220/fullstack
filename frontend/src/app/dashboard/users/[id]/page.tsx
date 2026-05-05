import { notFound } from "next/navigation";
import { getServerSession } from "next-auth";
import { redirect } from "next/navigation";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { authOptions } from "@/lib/auth/config";
import { getUser } from "@/lib/api/users/server";

export default async function UserDetailPage({ params }: { params: Promise<{ id: string }> }) {
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
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">User Details</h1>
        <div className="flex gap-3">
          <Button asChild variant="outline">
            <a href={`/dashboard/users/${id}/edit`}>Edit</a>
          </Button>
          <Button asChild variant="outline">
            <a href="/dashboard">Back</a>
          </Button>
        </div>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>{user.display_name}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-muted-foreground">Email:</span>
            <span className="text-sm">{user.email}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-muted-foreground">Role:</span>
            <Badge
              variant={
                user.role === "admin"
                  ? "destructive"
                  : user.role === "manager"
                    ? "warning"
                    : "secondary"
              }
            >
              {user.role}
            </Badge>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-muted-foreground">Verified:</span>
            <span className="text-sm">{user.email_verified ? "Yes" : "No"}</span>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
