"use client";

import { useMemo } from "react";
import { useAppStore } from "@/stores/app";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { SignOutButton } from "@/app/dashboard/sign-out-button";
import { AuthError } from "@/components/features/auth-error";

const navigation = [
  { href: "#overview", label: "Overview" },
  { href: "#users", label: "Users" },
  { href: "#endpoints", label: "Endpoints" },
];

export function DashboardShell({
  children,
  userName,
  userEmail,
}: {
  children: React.ReactNode;
  userName: string;
  userEmail: string;
}) {
  const sidebarOpen = useAppStore((state) => state.sidebarOpen);
  const setSidebarOpen = useAppStore((state) => state.setSidebarOpen);
  const initials = useMemo(() => {
    const source = userName.trim() || userEmail.trim() || "FS";
    return source
      .split(/\s+/)
      .slice(0, 2)
      .map((part) => part.charAt(0).toUpperCase())
      .join("");
  }, [userEmail, userName]);

  return (
    <div className="min-h-dvh bg-background">
      <a
        className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:bg-primary focus:px-4 focus:py-2 focus:text-primary-foreground"
        href="#dashboard-content"
      >
        Skip to content
      </a>
      <div className="mx-auto flex min-h-dvh w-full max-w-[1600px]">
        <aside
          className={cn(
            "fixed inset-y-0 left-0 z-40 w-72 border-r border-border bg-card/95 px-5 py-6 backdrop-blur transition-transform lg:static lg:translate-x-0",
            sidebarOpen ? "translate-x-0" : "-translate-x-full lg:translate-x-0",
          )}
        >
          <div className="flex items-center justify-between">
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                Fullstack Template
              </p>
              <h1 className="mt-2 text-xl font-semibold text-foreground">Bank Starter Console</h1>
            </div>
            <Button
              className="lg:hidden"
              onClick={() => setSidebarOpen(false)}
              size="sm"
              variant="ghost"
            >
              Close
            </Button>
          </div>
          <div className="mt-8 rounded-xl border border-border bg-background/80 p-4">
            <div className="flex items-center gap-3">
              <div className="flex h-11 w-11 items-center justify-center rounded-full bg-primary text-sm font-semibold text-primary-foreground">
                {initials}
              </div>
              <div className="min-w-0">
                <p className="truncate text-sm font-medium text-foreground">{userName}</p>
                <p className="truncate text-sm text-muted-foreground">{userEmail}</p>
              </div>
            </div>
          </div>
          <nav aria-label="Primary" className="mt-8 space-y-2">
            {navigation.map((item) => (
              <a
                key={item.href}
                className="flex min-h-11 items-center rounded-lg px-3 text-sm font-medium text-muted-foreground hover:bg-secondary hover:text-foreground"
                href={item.href}
                onClick={() => setSidebarOpen(false)}
              >
                {item.label}
              </a>
            ))}
          </nav>
          <div className="mt-8 rounded-xl border border-dashed border-border bg-background/80 p-4 text-sm leading-6 text-muted-foreground">
            This slice demonstrates protected layout, typed API access, and reusable UI primitives.
          </div>
        </aside>
        <div className="flex min-h-dvh flex-1 flex-col lg:pl-0">
          <header className="sticky top-0 z-30 border-b border-border bg-background/90 backdrop-blur">
            <div className="flex min-h-16 items-center justify-between gap-3 px-4 sm:px-6 lg:px-10">
              <div className="flex items-center gap-3">
                <Button onClick={() => setSidebarOpen(!sidebarOpen)} size="sm" variant="outline">
                  Menu
                </Button>
                <div>
                  <p className="text-sm font-medium text-foreground">Protected dashboard</p>
                  <p className="text-xs text-muted-foreground">
                    Built as a reusable on-prem starter slice
                  </p>
                </div>
              </div>
              <SignOutButton />
            </div>
          </header>
          <main className="flex-1 px-4 py-6 sm:px-6 lg:px-10 lg:py-8" id="dashboard-content">
            <AuthError />
            {children}
          </main>
        </div>
      </div>
    </div>
  );
}
