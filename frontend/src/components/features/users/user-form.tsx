"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { createUser, updateUser } from "@/lib/api/users/client";
import { zCreateUserRequest } from "@/lib/api/gen/zod.gen";
import { updateUserSchema } from "@/schemas";
import type { UserResponse } from "@/lib/api/gen/types.gen";

interface UserFormProps {
  mode: "create" | "edit";
  user?: UserResponse;
}

export function UserForm({ mode, user }: UserFormProps) {
  const router = useRouter();
  const [email, setEmail] = useState(user?.email ?? "");
  const [displayName, setDisplayName] = useState(user?.display_name ?? "");
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (mode === "create") {
      const result = zCreateUserRequest.safeParse({ email, display_name: displayName });
      if (!result.success) {
        setError(result.error.issues.map((issue) => issue.message).join(", "));
        return;
      }
    } else {
      const result = updateUserSchema.safeParse({ display_name: displayName || undefined });
      if (!result.success) {
        setError(result.error.issues.map((issue) => issue.message).join(", "));
        return;
      }
    }

    setIsSubmitting(true);
    try {
      if (mode === "create") {
        await createUser({ email, display_name: displayName });
      } else if (user) {
        await updateUser(user.id, { display_name: displayName || undefined });
      }
      router.push("/dashboard");
      router.refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save user");
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>{mode === "create" ? "New User" : "Edit User"}</CardTitle>
      </CardHeader>
      <CardContent>
        <form className="space-y-4" onSubmit={handleSubmit}>
          {mode === "create" && (
            <div>
              <label className="block text-sm font-medium" htmlFor="email">
                Email
              </label>
              <input
                className="mt-1 block w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                id="email"
                onChange={(e) => setEmail(e.target.value)}
                required
                type="email"
                value={email}
              />
            </div>
          )}
          <div>
            <label className="block text-sm font-medium" htmlFor="display_name">
              Display Name
            </label>
            <input
              className="mt-1 block w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              id="display_name"
              onChange={(e) => setDisplayName(e.target.value)}
              required
              type="text"
              value={displayName}
            />
          </div>
          {error && (
            <p className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {error}
            </p>
          )}
          <div className="flex gap-3">
            <Button disabled={isSubmitting} type="submit">
              {isSubmitting ? "Saving..." : mode === "create" ? "Create" : "Update"}
            </Button>
            <Button
              disabled={isSubmitting}
              onClick={() => router.push("/dashboard")}
              type="button"
              variant="outline"
            >
              Cancel
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  );
}
