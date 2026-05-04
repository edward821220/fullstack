"use client";

import { useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { useUsers } from "@/hooks/useUsers";
import type { PaginatedUserResponse, User } from "@/lib/api/types";

const dateFormatter = new Intl.DateTimeFormat("zh-TW", {
  dateStyle: "medium",
  timeStyle: "short",
});

function roleVariant(role: string) {
  switch (role.toLowerCase()) {
    case "admin":
      return "destructive" as const;
    case "manager":
      return "warning" as const;
    default:
      return "secondary" as const;
  }
}

export function UsersTable({ initialData }: { initialData: PaginatedUserResponse }) {
  const [page, setPage] = useState(initialData.page || 1);
  const [perPage] = useState(initialData.per_page || 8);
  const { data, error, isLoading } = useUsers(page, perPage, initialData);

  const summary = useMemo(() => {
    const users = data?.data ?? [];
    return {
      total: data?.total ?? 0,
      verified: users.filter((user) => user.email_verified).length,
      elevated: users.filter((user) => ["admin", "manager"].includes(user.role.toLowerCase()))
        .length,
      pageCount: users.length,
    };
  }, [data]);

  const canGoPrevious = page > 1;
  const canGoNext = Boolean(data && page * perPage < data.total);

  return (
    <div className="space-y-6" id="users">
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          description="Directory size"
          title="Total users"
          value={summary.total.toString()}
        />
        <MetricCard
          description="Current page"
          title="Verified"
          value={summary.verified.toString()}
        />
        <MetricCard
          description="Admin + manager"
          title="Elevated roles"
          value={summary.elevated.toString()}
        />
        <MetricCard
          description={`Page ${page}`}
          title="Visible records"
          value={summary.pageCount.toString()}
        />
      </div>

      <Card>
        <CardHeader className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <CardTitle>Users slice</CardTitle>
            <CardDescription>
              SWR client cache with server-rendered bootstrap data and backend OpenAPI types.
            </CardDescription>
          </div>
          <div className="text-sm text-muted-foreground">Page {data?.page ?? page}</div>
        </CardHeader>
        <CardContent className="space-y-4">
          {error ? (
            <div className="rounded-xl border border-destructive/20 bg-destructive/8 px-4 py-3 text-sm text-destructive">
              Failed to load users from the backend API.
            </div>
          ) : null}

          <div className="overflow-hidden rounded-xl border border-border">
            <div className="overflow-x-auto">
              <table className="min-w-full text-left text-sm">
                <thead className="bg-secondary/60 text-muted-foreground">
                  <tr>
                    <th className="px-4 py-3 font-medium">Name</th>
                    <th className="px-4 py-3 font-medium">Email</th>
                    <th className="px-4 py-3 font-medium">Role</th>
                    <th className="px-4 py-3 font-medium">Verified</th>
                    <th className="px-4 py-3 font-medium">Created</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border bg-card">
                  {isLoading
                    ? Array.from({ length: 5 }, (_, index) => (
                        <LoadingRow key={`loading-${index}`} />
                      ))
                    : (data?.data ?? []).map((user) => <UserRow key={user.id} user={user} />)}
                </tbody>
              </table>
            </div>
          </div>

          {!isLoading && (data?.data.length ?? 0) === 0 ? (
            <div className="rounded-xl border border-dashed border-border bg-background px-4 py-10 text-center text-sm text-muted-foreground">
              No users available yet.
            </div>
          ) : null}

          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <p className="text-sm text-muted-foreground">
              Showing {data?.data.length ?? 0} of {data?.total ?? 0} users
            </p>
            <div className="flex gap-2">
              <Button
                disabled={!canGoPrevious || isLoading}
                onClick={() => setPage((value: number) => Math.max(1, value - 1))}
                variant="outline"
              >
                Previous
              </Button>
              <Button
                disabled={!canGoNext || isLoading}
                onClick={() => setPage((value: number) => value + 1)}
                variant="outline"
              >
                Next
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function MetricCard({
  description,
  title,
  value,
}: {
  description: string;
  title: string;
  value: string;
}) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardDescription>{description}</CardDescription>
        <CardTitle className="text-base">{title}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="text-3xl font-semibold tracking-tight text-foreground">{value}</div>
      </CardContent>
    </Card>
  );
}

function UserRow({ user }: { user: User }) {
  return (
    <tr className="hover:bg-secondary/30">
      <td className="px-4 py-3 align-middle">
        <div className="font-medium text-foreground">{user.display_name}</div>
      </td>
      <td className="px-4 py-3 align-middle text-muted-foreground">{user.email}</td>
      <td className="px-4 py-3 align-middle">
        <Badge variant={roleVariant(user.role)}>{user.role}</Badge>
      </td>
      <td className="px-4 py-3 align-middle">
        <Badge variant={user.email_verified ? "success" : "secondary"}>
          {user.email_verified ? "Verified" : "Pending"}
        </Badge>
      </td>
      <td className="px-4 py-3 align-middle text-muted-foreground">
        {dateFormatter.format(new Date(user.created_at))}
      </td>
    </tr>
  );
}

function LoadingRow() {
  return (
    <tr>
      <td className="px-4 py-3">
        <Skeleton className="h-5 w-28" />
      </td>
      <td className="px-4 py-3">
        <Skeleton className="h-5 w-52" />
      </td>
      <td className="px-4 py-3">
        <Skeleton className="h-5 w-20" />
      </td>
      <td className="px-4 py-3">
        <Skeleton className="h-5 w-16" />
      </td>
      <td className="px-4 py-3">
        <Skeleton className="h-5 w-32" />
      </td>
    </tr>
  );
}
