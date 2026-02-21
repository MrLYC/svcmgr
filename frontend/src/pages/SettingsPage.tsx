import { useState } from "react";
import type { SettingsConfig } from "@/types/api";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/services/api";
import { useI18n } from "@/i18n";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Check, X, AlertTriangle } from "lucide-react";
import { toast } from "@/hooks/use-toast";

export default function SettingsPage() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const { data: settings } = useQuery({ queryKey: ["settings"], queryFn: api.getSettings });
  const { data: tools } = useQuery({ queryKey: ["tool-statuses"], queryFn: api.getToolStatuses });

  const [form, setForm] = useState<Partial<SettingsConfig>>({});
  const [resetConfirm, setResetConfirm] = useState("");

  const merged = { ...settings, ...form };

  const updateMutation = useMutation({
    mutationFn: () => api.updateSettings(form),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["settings"] }); toast({ title: t("settings.saved") }); setForm({}); },
  });

  const resetMutation = useMutation({
    mutationFn: () => api.resetSystem(),
    onSuccess: () => { toast({ title: t("settings.reset_done") }); setResetConfirm(""); },
  });

  return (
    <div className="space-y-6 max-w-2xl">
      <h1 className="text-2xl font-bold tracking-tight">{t("settings.title")}</h1>

      <Card>
        <CardHeader><CardTitle className="text-base">{t("settings.general")}</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 sm:grid-cols-2">
            <div><Label>{t("settings.nginx_port")}</Label><Input type="number" value={merged.nginx_port ?? 80} onChange={e => setForm(f => ({ ...f, nginx_port: parseInt(e.target.value) }))} /></div>
            <div><Label>{t("settings.log_level")}</Label>
              <Select value={merged.log_level ?? "info"} onValueChange={v => setForm(f => ({ ...f, log_level: v as SettingsConfig["log_level"] }))}>
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="debug">Debug</SelectItem>
                  <SelectItem value="info">Info</SelectItem>
                  <SelectItem value="warn">Warning</SelectItem>
                  <SelectItem value="error">Error</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <div><Label>{t("settings.config_dir")}</Label><Input value={merged.config_dir ?? ""} onChange={e => setForm(f => ({ ...f, config_dir: e.target.value }))} className="font-mono" /></div>
          <div className="flex items-center justify-between">
            <Label>{t("settings.auto_commit")}</Label>
            <Switch checked={merged.auto_commit ?? false} onCheckedChange={v => setForm(f => ({ ...f, auto_commit: v }))} />
          </div>
          <Button onClick={() => updateMutation.mutate()} disabled={Object.keys(form).length === 0}>{t("common.save_changes")}</Button>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle className="text-base">{t("settings.external_tools")}</CardTitle></CardHeader>
        <CardContent>
          <div className="space-y-3">
            {tools?.map(tool => (
              <div key={tool.name} className="flex items-center justify-between text-sm">
                <div className="flex items-center gap-2">
                  {tool.installed ? <Check className="h-4 w-4 text-success" /> : <X className="h-4 w-4 text-destructive" />}
                  <span className="font-medium font-mono">{tool.name}</span>
                </div>
                <div className="flex items-center gap-2">
                  {tool.version && <Badge variant="outline" className="font-mono text-xs">{tool.version}</Badge>}
                  {tool.path && <span className="text-xs text-muted-foreground font-mono">{tool.path}</span>}
                  {!tool.installed && <span className="text-xs text-destructive">{t("settings.not_installed")}</span>}
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card className="border-destructive/50">
        <CardHeader>
          <CardTitle className="text-base text-destructive flex items-center gap-2"><AlertTriangle className="h-4 w-4" />{t("settings.danger_zone")}</CardTitle>
          <CardDescription>{t("settings.danger_desc")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-sm text-muted-foreground">{t("settings.reset_confirm")}</p>
          <div className="flex gap-2">
            <Input value={resetConfirm} onChange={e => setResetConfirm(e.target.value)} placeholder="Type RESET to confirm" className="max-w-[200px]" />
            <Button variant="destructive" disabled={resetConfirm !== "RESET"} onClick={() => resetMutation.mutate()}>{t("settings.reset_system")}</Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
