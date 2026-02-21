import { useQuery } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Server, Clock, Globe, CloudCog, Plus, Terminal } from "lucide-react";
import { api } from "@/services/api";
import { Link } from "react-router-dom";
import { formatDistanceToNow } from "date-fns";
import { useI18n } from "@/i18n";

const typeIcons: Record<string, string> = {
  systemd: "🔧", crontab: "⏰", nginx: "🌐", cloudflare: "☁️", tty: "💻", config: "📁", system: "⚙️",
};

const typeRoutes: Record<string, string> = {
  systemd: "/services/systemd",
  crontab: "/services/crontab",
  nginx: "/proxy/nginx",
  cloudflare: "/proxy/cloudflare",
  tty: "/tty",
  config: "/config",
};

export default function Dashboard() {
  const { t } = useI18n();
  const { data: stats } = useQuery({ queryKey: ["dashboard-stats"], queryFn: api.getDashboardStats });
  const { data: activity } = useQuery({ queryKey: ["activity"], queryFn: api.getActivityLogs });

  const cards = [
    { label: t("dash.systemd_services"), value: `${stats?.systemd_running ?? 0}/${stats?.systemd_total ?? 0}`, sub: t("dash.running"), icon: Server, color: "text-success", to: "/services/systemd" },
    { label: t("dash.crontab_tasks"), value: stats?.crontab_tasks ?? 0, sub: t("dash.active"), icon: Clock, color: "text-info", to: "/services/crontab" },
    { label: t("dash.nginx_proxies"), value: stats?.nginx_proxies ?? 0, sub: t("dash.configured"), icon: Globe, color: "text-primary", to: "/proxy/nginx" },
    { label: t("dash.cf_tunnels"), value: `${stats?.cloudflare_connected ?? 0}/${stats?.cloudflare_total ?? 0}`, sub: t("dash.connected"), icon: CloudCog, color: "text-warning", to: "/proxy/cloudflare" },
  ];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">{t("dash.title")}</h1>
        <div className="flex gap-2">
          <Button size="sm" asChild><Link to="/services/systemd"><Plus className="mr-1 h-3.5 w-3.5" />{t("dash.service")}</Link></Button>
          <Button size="sm" variant="outline" asChild><Link to="/proxy/nginx"><Plus className="mr-1 h-3.5 w-3.5" />{t("dash.proxy")}</Link></Button>
          <Button size="sm" variant="outline" asChild><Link to="/tty"><Terminal className="mr-1 h-3.5 w-3.5" />{t("nav.tty")}</Link></Button>
        </div>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {cards.map(c => (
          <Link key={c.label} to={c.to}>
            <Card className="cursor-pointer transition-colors hover:bg-muted/50">
              <CardHeader className="flex flex-row items-center justify-between pb-2">
                <CardTitle className="text-sm font-medium text-muted-foreground">{c.label}</CardTitle>
                <c.icon className={`h-4 w-4 ${c.color}`} />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{c.value}</div>
                <p className="text-xs text-muted-foreground">{c.sub}</p>
              </CardContent>
            </Card>
          </Link>
        ))}
      </div>

      <Card>
        <CardHeader><CardTitle className="text-base">{t("dash.recent_activity")}</CardTitle></CardHeader>
        <CardContent>
          <div className="space-y-3">
            {activity?.map(log => (
              <Link key={log.id} to={typeRoutes[log.type] ?? "/"} className="flex items-start gap-3 text-sm hover:bg-muted/50 rounded-md p-1 -mx-1 transition-colors">
                <span className="mt-0.5 text-base">{typeIcons[log.type] || "📋"}</span>
                <div className="flex-1">
                  <p>{log.description}</p>
                  <p className="text-xs text-muted-foreground">{formatDistanceToNow(new Date(log.timestamp), { addSuffix: true })}</p>
                </div>
              </Link>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
