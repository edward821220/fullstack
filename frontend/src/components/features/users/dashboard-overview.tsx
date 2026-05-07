import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export function DashboardOverview({
  userName,
  userEmail,
}: {
  userName: string;
  userEmail: string;
}) {
  return (
    <section className="space-y-6" id="overview">
      <Card className="overflow-hidden border-none bg-[radial-gradient(circle_at_top_left,_rgba(37,99,235,0.18),_transparent_45%),linear-gradient(135deg,_rgba(15,23,42,0.98),_rgba(30,41,59,0.94))] text-white shadow-xl shadow-slate-900/10">
        <CardHeader className="gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div className="max-w-3xl space-y-4">
            <Badge className="bg-white/10 text-white" variant="secondary">
              Template candidate → production-ready slice
            </Badge>
            <div className="space-y-2">
              <CardTitle className="text-3xl leading-tight sm:text-4xl">
                Welcome back, {userName || "Operator"}
              </CardTitle>
              <CardDescription className="max-w-2xl text-sm leading-6 text-slate-200 sm:text-base">
                This dashboard demonstrates the baseline for an authenticated back-office module:
                protected routes, typed API calls, reusable UI primitives, and an extendable admin
                information architecture.
              </CardDescription>
            </div>
          </div>
          <div className="rounded-2xl border border-white/10 bg-white/5 px-5 py-4 backdrop-blur">
            <p className="text-xs uppercase tracking-[0.2em] text-slate-300">Signed in as</p>
            <p className="mt-2 text-base font-medium text-white">{userEmail}</p>
          </div>
        </CardHeader>
      </Card>

      <div className="grid gap-4 xl:grid-cols-[1.5fr_1fr]" id="endpoints">
        <Card>
          <CardHeader>
            <CardTitle>Template guarantees</CardTitle>
            <CardDescription>What this slice is already proving for future teams.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-3 sm:grid-cols-2">
            <Capability
              title="Protected app shell"
              description="Server session gate plus client navigation shell."
            />
            <Capability
              title="Typed API contract"
              description="Frontend user types are still sourced from backend OpenAPI."
            />
            <Capability
              title="Client/server data path"
              description="Server bootstrap fetch plus SWR revalidation pattern."
            />
            <Capability
              title="Layer-ready UX"
              description="Simple cards, badges, tables, and empty/loading states."
            />
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Service endpoints</CardTitle>
            <CardDescription>Useful defaults exposed by the backend template.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-3 text-sm leading-6 text-muted-foreground">
            <EndpointRow label="REST health" value="GET /health" />
            <EndpointRow label="REST readiness" value="GET /health/ready" />
            <EndpointRow label="REST metrics" value="GET /metrics" />
            <EndpointRow label="REST users" value="GET /api/v1/users" />
            <EndpointRow label="gRPC users" value="users.v1.UsersService" />
          </CardContent>
        </Card>
      </div>
    </section>
  );
}

function Capability({ description, title }: { description: string; title: string }) {
  return (
    <div className="rounded-xl border border-border bg-background/70 p-4">
      <h3 className="text-sm font-semibold text-foreground">{title}</h3>
      <p className="mt-2 text-sm leading-6 text-muted-foreground">{description}</p>
    </div>
  );
}

function EndpointRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-lg border border-border bg-background px-3 py-2">
      <span className="font-medium text-foreground">{label}</span>
      <code className="text-xs text-muted-foreground">{value}</code>
    </div>
  );
}
