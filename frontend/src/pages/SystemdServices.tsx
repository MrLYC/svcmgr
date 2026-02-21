import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/services/api";
import { useI18n } from "@/i18n";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { StatusBadge } from "@/components/StatusBadge";
import { Play, Square, RotateCcw, FileText, Plus, ChevronRight } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { toast } from "@/hooks/use-toast";
import { PageBreadcrumb } from "@/components/PageBreadcrumb";
import { ConfirmDeleteButton } from "@/components/ConfirmDeleteButton";
import type { SystemdService } from "@/types/api";

export default function SystemdServices() {
  const [selectedService, setSelectedService] = useState<SystemdService | null>(null);

  if (selectedService) {
    return <ServiceDetailView service={selectedService} onBack={() => setSelectedService(null)} />;
  }
  return <ServiceListView onSelect={setSelectedService} />;
}

function ServiceListView({ onSelect }: { onSelect: (s: SystemdService) => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const { data: services, isLoading } = useQuery({ queryKey: ["systemd-services"], queryFn: api.getServices });
  const [createOpen, setCreateOpen] = useState(false);

  const controlMutation = useMutation({
    mutationFn: ({ name, action }: { name: string; action: "start" | "stop" | "restart" }) => api.controlService(name, action),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["systemd-services"] }); toast({ title: t("systemd.action_completed") }); },
  });

  const toggleMutation = useMutation({
    mutationFn: ({ name, enabled }: { name: string; enabled: boolean }) => api.toggleServiceEnabled(name, enabled),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["systemd-services"] }),
  });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">{t("systemd.title")}</h1>
        <Dialog open={createOpen} onOpenChange={setCreateOpen}>
          <DialogTrigger asChild><Button size="sm"><Plus className="mr-1 h-3.5 w-3.5" />{t("systemd.create_service")}</Button></DialogTrigger>
          <DialogContent className="max-w-2xl">
            <DialogHeader><DialogTitle>{t("systemd.create_service")}</DialogTitle></DialogHeader>
            <CreateServiceForm onClose={() => setCreateOpen(false)} />
          </DialogContent>
        </Dialog>
      </div>

      <div className="rounded-lg border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("common.name")}</TableHead>
              <TableHead>{t("common.status")}</TableHead>
              <TableHead>{t("common.enabled")}</TableHead>
              <TableHead>{t("systemd.pid")}</TableHead>
              <TableHead>{t("systemd.memory")}</TableHead>
              <TableHead>{t("common.uptime")}</TableHead>
              <TableHead className="text-right">{t("common.actions")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow><TableCell colSpan={7} className="text-center text-muted-foreground">{t("common.loading")}</TableCell></TableRow>
            ) : services?.map(s => (
              <TableRow key={s.name} className="cursor-pointer hover:bg-muted/50" onClick={() => onSelect(s)}>
                <TableCell>
                  <div><div className="font-medium">{s.name}</div>{s.description && <div className="text-xs text-muted-foreground">{s.description}</div>}</div>
                </TableCell>
                <TableCell><StatusBadge status={s.status} /></TableCell>
                <TableCell onClick={e => e.stopPropagation()}><Switch checked={s.enabled} onCheckedChange={(v) => toggleMutation.mutate({ name: s.name, enabled: v })} /></TableCell>
                <TableCell className="font-mono text-xs">{s.pid ?? "—"}</TableCell>
                <TableCell className="text-sm">{s.memory ?? "—"}</TableCell>
                <TableCell className="text-sm">{s.uptime ?? "—"}</TableCell>
                <TableCell onClick={e => e.stopPropagation()}>
                  <div className="flex justify-end gap-1">
                    {s.status !== "running" && <Button variant="ghost" size="icon" onClick={() => controlMutation.mutate({ name: s.name, action: "start" })}><Play className="h-3.5 w-3.5" /></Button>}
                    {s.status === "running" && <Button variant="ghost" size="icon" onClick={() => controlMutation.mutate({ name: s.name, action: "stop" })}><Square className="h-3.5 w-3.5" /></Button>}
                    <Button variant="ghost" size="icon" onClick={() => controlMutation.mutate({ name: s.name, action: "restart" })}><RotateCcw className="h-3.5 w-3.5" /></Button>
                    <ChevronRight className="h-4 w-4 text-muted-foreground mt-2" />
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function ServiceDetailView({ service, onBack }: { service: SystemdService; onBack: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [logOpen, setLogOpen] = useState(false);
  const [form, setForm] = useState({
    description: service.description ?? "",
    exec_start: service.exec_start ?? "",
    working_directory: service.working_directory ?? "",
    restart_policy: service.restart_policy ?? "on-failure",
    envVars: service.environment ? Object.entries(service.environment).map(([k, v]) => `${k}=${v}`).join("\n") : "",
  });

  const updateMutation = useMutation({
    mutationFn: () => api.updateService(service.name, {
      description: form.description, exec_start: form.exec_start,
      working_directory: form.working_directory, restart_policy: form.restart_policy,
    }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["systemd-services"] }); toast({ title: t("systemd.service_updated") }); },
  });

  const deleteMutation = useMutation({
    mutationFn: () => api.deleteService(service.name),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["systemd-services"] }); toast({ title: t("systemd.service_deleted") }); onBack(); },
  });

  const controlMutation = useMutation({
    mutationFn: (action: "start" | "stop" | "restart") => api.controlService(service.name, action),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["systemd-services"] }); toast({ title: t("systemd.action_completed") }); },
  });

  const serviceFilePreview = `[Unit]\nDescription=${form.description}\n\n[Service]\nExecStart=${form.exec_start}\nWorkingDirectory=${form.working_directory}\nRestart=${form.restart_policy}\n${form.envVars ? form.envVars.split("\n").map(e => `Environment=${e}`).join("\n") : ""}\n\n[Install]\nWantedBy=multi-user.target`;

  return (
    <div className="space-y-6">
      <PageBreadcrumb items={[
        { label: t("nav.services"), onClick: onBack },
        { label: t("nav.systemd"), onClick: onBack },
        { label: service.name },
      ]} />

      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{service.name}</h1>
          {service.description && <p className="text-sm text-muted-foreground">{service.description}</p>}
        </div>
        <div className="flex items-center gap-2">
          <StatusBadge status={service.status} />
          <div className="flex gap-1">
            {service.status !== "running" && <Button variant="outline" size="sm" onClick={() => controlMutation.mutate("start")}><Play className="mr-1 h-3.5 w-3.5" />{t("common.start")}</Button>}
            {service.status === "running" && <Button variant="outline" size="sm" onClick={() => controlMutation.mutate("stop")}><Square className="mr-1 h-3.5 w-3.5" />{t("common.stop")}</Button>}
            <Button variant="outline" size="sm" onClick={() => controlMutation.mutate("restart")}><RotateCcw className="mr-1 h-3.5 w-3.5" />{t("common.restart")}</Button>
            <Button variant="outline" size="sm" onClick={() => setLogOpen(true)}><FileText className="mr-1 h-3.5 w-3.5" />{t("systemd.logs")}</Button>
          </div>
        </div>
      </div>

      <Card>
        <CardContent className="flex items-center gap-6 py-4 text-sm">
          <div><span className="text-muted-foreground">PID:</span> <span className="font-mono">{service.pid ?? "—"}</span></div>
          <div><span className="text-muted-foreground">{t("systemd.memory")}:</span> {service.memory ?? "—"}</div>
          <div><span className="text-muted-foreground">{t("common.uptime")}:</span> {service.uptime ?? "—"}</div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle className="text-base">{t("systemd.edit_service")}</CardTitle></CardHeader>
        <CardContent>
          <Tabs defaultValue="form">
            <TabsList className="mb-4">
              <TabsTrigger value="form">{t("systemd.form")}</TabsTrigger>
              <TabsTrigger value="text">{t("systemd.preview")}</TabsTrigger>
            </TabsList>
            <TabsContent value="form" className="space-y-4">
              <div className="grid gap-4 sm:grid-cols-2">
                <div><Label>{t("common.description")}</Label><Input value={form.description} onChange={e => setForm(f => ({ ...f, description: e.target.value }))} /></div>
                <div><Label>{t("systemd.restart_policy")}</Label>
                  <Select value={form.restart_policy} onValueChange={v => setForm(f => ({ ...f, restart_policy: v }))}>
                    <SelectTrigger><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="no">No</SelectItem>
                      <SelectItem value="on-failure">On Failure</SelectItem>
                      <SelectItem value="always">Always</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
              <div><Label>{t("systemd.exec_start")}</Label><Input value={form.exec_start} onChange={e => setForm(f => ({ ...f, exec_start: e.target.value }))} className="font-mono" /></div>
              <div><Label>{t("systemd.working_dir")}</Label><Input value={form.working_directory} onChange={e => setForm(f => ({ ...f, working_directory: e.target.value }))} className="font-mono" /></div>
              <div><Label>{t("systemd.env_vars")}</Label><Textarea value={form.envVars} onChange={e => setForm(f => ({ ...f, envVars: e.target.value }))} className="font-mono text-xs" rows={3} /></div>
              <div className="flex gap-2">
                <Button onClick={() => updateMutation.mutate()}>{t("common.save_changes")}</Button>
                <ConfirmDeleteButton onConfirm={() => deleteMutation.mutate()} />
              </div>
            </TabsContent>
            <TabsContent value="text">
              <pre className="rounded-md bg-muted p-4 font-mono text-xs whitespace-pre-wrap">{serviceFilePreview}</pre>
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>

      <Sheet open={logOpen} onOpenChange={setLogOpen}>
        <SheetContent className="w-[500px] sm:w-[600px]">
          <SheetHeader><SheetTitle>{t("systemd.logs")}: {service.name}</SheetTitle></SheetHeader>
          <ServiceLogs name={service.name} />
        </SheetContent>
      </Sheet>
    </div>
  );
}

function ServiceLogs({ name }: { name: string }) {
  const { data: logs } = useQuery({ queryKey: ["systemd-logs", name], queryFn: () => api.getServiceLogs(name) });
  return (
    <div className="mt-4 space-y-2 font-mono text-xs">
      {logs?.map((log, i) => (
        <div key={i} className="flex gap-2">
          <span className="text-muted-foreground whitespace-nowrap">{new Date(log.timestamp).toLocaleTimeString()}</span>
          <StatusBadge status={log.level === "error" ? "failed" : log.level === "warning" ? "degraded" : "running"} className="text-[10px] px-1.5 py-0" />
          <span>{log.message}</span>
        </div>
      ))}
    </div>
  );
}

function CreateServiceForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ name: "", description: "", exec_start: "", working_directory: "", restart_policy: "on-failure", envVars: "" });
  const [mode, setMode] = useState<"form" | "text">("form");

  const createMutation = useMutation({
    mutationFn: (data: Partial<SystemdService>) => api.createService(data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["systemd-services"] }); toast({ title: t("systemd.service_created") }); onClose(); },
  });

  const serviceFilePreview = `[Unit]\nDescription=${form.description}\n\n[Service]\nExecStart=${form.exec_start}\nWorkingDirectory=${form.working_directory}\nRestart=${form.restart_policy}\n${form.envVars ? form.envVars.split("\n").map(e => `Environment=${e}`).join("\n") : ""}\n\n[Install]\nWantedBy=multi-user.target`;

  return (
    <Tabs value={mode} onValueChange={(v) => setMode(v as "form" | "text")}>
      <TabsList className="mb-4">
        <TabsTrigger value="form">{t("systemd.form")}</TabsTrigger>
        <TabsTrigger value="text">{t("systemd.preview")}</TabsTrigger>
      </TabsList>
      <TabsContent value="form" className="space-y-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <div><Label>{t("systemd.service_name")}</Label><Input value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} placeholder="my-app" /></div>
          <div><Label>{t("systemd.restart_policy")}</Label>
            <Select value={form.restart_policy} onValueChange={v => setForm(f => ({ ...f, restart_policy: v }))}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="no">No</SelectItem>
                <SelectItem value="on-failure">On Failure</SelectItem>
                <SelectItem value="always">Always</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>
        <div><Label>{t("common.description")}</Label><Input value={form.description} onChange={e => setForm(f => ({ ...f, description: e.target.value }))} /></div>
        <div><Label>{t("systemd.exec_start")}</Label><Input value={form.exec_start} onChange={e => setForm(f => ({ ...f, exec_start: e.target.value }))} placeholder="/usr/bin/node app.js" className="font-mono" /></div>
        <div><Label>{t("systemd.working_dir")}</Label><Input value={form.working_directory} onChange={e => setForm(f => ({ ...f, working_directory: e.target.value }))} placeholder="/opt/my-app" className="font-mono" /></div>
        <div><Label>{t("systemd.env_vars")}</Label><Textarea value={form.envVars} onChange={e => setForm(f => ({ ...f, envVars: e.target.value }))} placeholder="PORT=3000&#10;NODE_ENV=production" className="font-mono text-xs" rows={3} /></div>
        <Button onClick={() => createMutation.mutate(form)} className="w-full">{t("systemd.create_service")}</Button>
      </TabsContent>
      <TabsContent value="text">
        <pre className="rounded-md bg-muted p-4 font-mono text-xs whitespace-pre-wrap">{serviceFilePreview}</pre>
      </TabsContent>
    </Tabs>
  );
}
