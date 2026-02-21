import { useState, useRef, useEffect, useCallback } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/services/api";
import { useI18n } from "@/i18n";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Play, Plus, Trash2, ArrowLeft, ChevronRight, Download, ArrowRightLeft, Terminal } from "lucide-react";
import { PageBreadcrumb } from "@/components/PageBreadcrumb";
import { ConfirmDeleteButton } from "@/components/ConfirmDeleteButton";
import { toast } from "@/hooks/use-toast";
import type { MiseDependency, MiseTask } from "@/types/api";

// Simulate streaming log output for install/uninstall operations
function useMockLogs() {
  const [logs, setLogs] = useState<string[]>([]);
  const [running, setRunning] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs]);

  const runLogs = useCallback((lines: string[], onDone?: () => void) => {
    setLogs([]);
    setRunning(true);
    let i = 0;
    const interval = setInterval(() => {
      if (i < lines.length) {
        setLogs(prev => [...prev, lines[i]]);
        i++;
      } else {
        clearInterval(interval);
        setRunning(false);
        onDone?.();
      }
    }, 300);
    return () => clearInterval(interval);
  }, []);

  return { logs, running, scrollRef, runLogs, setLogs };
}

function LogPanel({ logs, running, scrollRef }: { logs: string[]; running: boolean; scrollRef: React.RefObject<HTMLDivElement> }) {
  const { t } = useI18n();
  if (logs.length === 0 && !running) return null;
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-base flex items-center gap-2">
          <Terminal className="h-4 w-4" />
          {t("mise.operation_log")}
          {running && <Badge variant="outline" className="animate-pulse text-xs">running</Badge>}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div ref={scrollRef} className="bg-muted/50 rounded-lg p-3 max-h-[240px] overflow-y-auto font-mono text-xs space-y-0.5">
          {logs.map((line, i) => (
            <div key={i} className={line?.startsWith("✓") ? "text-green-500" : line?.startsWith("✗") ? "text-destructive" : "text-foreground/80"}>
              {line}
            </div>
          ))}
          {running && <div className="animate-pulse text-muted-foreground">▌</div>}
        </div>
      </CardContent>
    </Card>
  );
}

type DetailView = 
  | { type: "dep"; item: MiseDependency }
  | { type: "task"; item: MiseTask }
  | null;

export default function MiseTasks() {
  const [detail, setDetail] = useState<DetailView>(null);

  if (detail?.type === "dep") {
    return <DepDetailView dep={detail.item} onBack={() => setDetail(null)} />;
  }
  if (detail?.type === "task") {
    return <TaskDetailView task={detail.item} onBack={() => setDetail(null)} />;
  }
  return <MiseListView onSelectDep={d => setDetail({ type: "dep", item: d })} onSelectTask={t => setDetail({ type: "task", item: t })} />;
}

function MiseListView({ onSelectDep, onSelectTask }: { onSelectDep: (d: MiseDependency) => void; onSelectTask: (t: MiseTask) => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const { data: deps } = useQuery({ queryKey: ["mise-deps"], queryFn: api.getMiseDeps });
  const { data: tasks } = useQuery({ queryKey: ["mise-tasks"], queryFn: api.getMiseTasks });
  const [addDepOpen, setAddDepOpen] = useState(false);
  const [addTaskOpen, setAddTaskOpen] = useState(false);

  const runMutation = useMutation({
    mutationFn: (name: string) => api.runMiseTask(name),
    onSuccess: () => toast({ title: t("mise.task_executed") }),
  });

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-bold tracking-tight">{t("mise.title")}</h1>
      <Tabs defaultValue="deps">
        <TabsList>
          <TabsTrigger value="deps">{t("mise.dependencies")}</TabsTrigger>
          <TabsTrigger value="tasks">{t("mise.tasks")}</TabsTrigger>
        </TabsList>

        <TabsContent value="deps">
          <div className="mb-3 flex justify-end">
            <Dialog open={addDepOpen} onOpenChange={setAddDepOpen}>
              <DialogTrigger asChild><Button size="sm"><Plus className="mr-1 h-3.5 w-3.5" />{t("mise.add_dep")}</Button></DialogTrigger>
              <DialogContent>
                <DialogHeader><DialogTitle>{t("mise.add_dep")}</DialogTitle></DialogHeader>
                <AddDepForm onClose={() => setAddDepOpen(false)} />
              </DialogContent>
            </Dialog>
          </div>
          <div className="rounded-lg border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("mise.tool")}</TableHead>
                  <TableHead>{t("mise.current")}</TableHead>
                  <TableHead>{t("mise.latest")}</TableHead>
                  <TableHead>{t("mise.source")}</TableHead>
                  <TableHead className="text-right">{t("common.actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {deps?.map(d => (
                  <TableRow key={d.id} className="cursor-pointer hover:bg-muted/50" onClick={() => onSelectDep(d)}>
                    <TableCell className="font-medium">{d.name}</TableCell>
                    <TableCell><code className="font-mono text-xs bg-muted px-1.5 py-0.5 rounded">{d.current_version}</code></TableCell>
                    <TableCell>
                      {d.latest_version && d.latest_version !== d.current_version
                        ? <Badge variant="outline" className="font-mono text-xs">{d.latest_version}</Badge>
                        : <span className="text-xs text-muted-foreground">{t("mise.up_to_date")}</span>}
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground">{d.source}</TableCell>
                    <TableCell className="text-right"><ChevronRight className="h-4 w-4 text-muted-foreground inline-block" /></TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </TabsContent>

        <TabsContent value="tasks">
          <div className="mb-3 flex justify-end">
            <Dialog open={addTaskOpen} onOpenChange={setAddTaskOpen}>
              <DialogTrigger asChild><Button size="sm"><Plus className="mr-1 h-3.5 w-3.5" />{t("mise.add_task")}</Button></DialogTrigger>
              <DialogContent>
                <DialogHeader><DialogTitle>{t("mise.add_task")}</DialogTitle></DialogHeader>
                <CreateTaskForm onClose={() => setAddTaskOpen(false)} />
              </DialogContent>
            </Dialog>
          </div>
          <div className="rounded-lg border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("common.name")}</TableHead>
                  <TableHead>{t("common.description")}</TableHead>
                  <TableHead>{t("common.command")}</TableHead>
                  <TableHead className="text-right">{t("common.actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {tasks?.map(task => (
                  <TableRow key={task.id} className="cursor-pointer hover:bg-muted/50" onClick={() => onSelectTask(task)}>
                    <TableCell className="font-medium">{task.name}</TableCell>
                    <TableCell className="text-sm text-muted-foreground">{task.description ?? "—"}</TableCell>
                    <TableCell className="font-mono text-xs max-w-[300px] truncate">{task.command}</TableCell>
                    <TableCell onClick={e => e.stopPropagation()}>
                      <div className="flex justify-end gap-1">
                        <Button variant="ghost" size="icon" onClick={() => runMutation.mutate(task.name)}><Play className="h-3.5 w-3.5" /></Button>
                        <ChevronRight className="h-4 w-4 text-muted-foreground mt-2" />
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </TabsContent>
      </Tabs>
    </div>
  );
}

function DepDetailView({ dep, onBack }: { dep: MiseDependency; onBack: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [installOpen, setInstallOpen] = useState(false);
  const { logs, running, scrollRef, runLogs } = useMockLogs();

  const { data: availableVersions } = useQuery({
    queryKey: ["mise-available-versions", dep.name],
    queryFn: () => api.getAvailableVersions(dep.name),
  });

  const switchMutation = useMutation({
    mutationFn: (version: string) => api.switchMiseDepVersion(dep.id, version),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["mise-deps"] }); toast({ title: t("mise.dep_updated") }); },
  });

  const handleUninstallVersion = (version: string) => {
    const logLines = [
      `$ mise uninstall ${dep.name}@${version}`,
      `${t("mise.uninstalling")} ${dep.name}@${version}...`,
      `Removing ${dep.name} ${version} from ~/.local/share/mise/installs/${dep.name}/${version}`,
      `Cleaning up shims...`,
      `Updating registry...`,
      `✓ ${dep.name}@${version} uninstalled successfully`,
    ];
    runLogs(logLines, () => {
      queryClient.invalidateQueries({ queryKey: ["mise-deps"] });
      toast({ title: t("mise.version_uninstalled") });
    });
    api.uninstallMiseDepVersion(dep.id, version);
  };

  const handleDeleteAll = () => {
    const logLines = [
      `$ mise uninstall ${dep.name} --all`,
      `${t("mise.uninstalling")} all versions of ${dep.name}...`,
      ...(dep.installed_versions ?? []).map(v => `  Removing ${dep.name}@${v}...`),
      `Cleaning up shims and registry...`,
      `Removing tool configuration...`,
      `✓ ${dep.name} completely uninstalled`,
    ];
    runLogs(logLines, () => {
      queryClient.invalidateQueries({ queryKey: ["mise-deps"] });
      toast({ title: t("mise.dep_deleted") });
      onBack();
    });
    api.deleteMiseDep(dep.id);
  };

  const handleInstallDone = (version: string) => {
    const logLines = [
      `$ mise install ${dep.name}@${version}`,
      `${t("mise.installing")} ${dep.name}@${version}...`,
      `Downloading ${dep.name} ${version} from registry...`,
      `Extracting archive...`,
      `Installing to ~/.local/share/mise/installs/${dep.name}/${version}`,
      `Creating shims...`,
      `Verifying installation...`,
      `✓ ${dep.name}@${version} installed successfully`,
    ];
    setInstallOpen(false);
    runLogs(logLines, () => {
      queryClient.invalidateQueries({ queryKey: ["mise-deps"] });
      toast({ title: t("mise.dep_added") });
    });
    api.createMiseDep({ name: dep.name, version });
  };

  const installedVersions = dep.installed_versions ?? [];
  const installableVersions = (availableVersions ?? []).filter(v => !installedVersions.includes(v));

  return (
    <div className="space-y-6">
      <PageBreadcrumb items={[
        { label: t("nav.services"), onClick: onBack },
        { label: t("nav.mise"), onClick: onBack },
        { label: dep.name },
      ]} />

      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{dep.name}</h1>
          <p className="text-sm text-muted-foreground">{t("mise.source")}: {dep.source}</p>
        </div>
        <div className="flex items-center gap-2">
          {dep.latest_version && dep.latest_version !== dep.current_version && (
            <Badge variant="outline" className="font-mono">{t("mise.latest")}: {dep.latest_version}</Badge>
          )}
          <Dialog open={installOpen} onOpenChange={setInstallOpen}>
            <DialogTrigger asChild><Button size="sm"><Download className="mr-1 h-3.5 w-3.5" />{t("mise.install_version")}</Button></DialogTrigger>
            <DialogContent>
              <DialogHeader><DialogTitle>{t("mise.install_version")}: {dep.name}</DialogTitle></DialogHeader>
              <InstallVersionForm depName={dep.name} versions={installableVersions} onInstall={handleInstallDone} />
            </DialogContent>
          </Dialog>
        </div>
      </div>

      <Card>
        <CardHeader><CardTitle className="text-base">{t("mise.installed_versions")}</CardTitle></CardHeader>
        <CardContent>
          <div className="rounded-lg border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("mise.version")}</TableHead>
                  <TableHead>{t("common.status")}</TableHead>
                  <TableHead className="text-right">{t("common.actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {installedVersions.map(v => (
                  <TableRow key={v}>
                    <TableCell><code className="font-mono text-sm">{v}</code></TableCell>
                    <TableCell>
                      {v === dep.current_version
                        ? <Badge>{t("mise.active")}</Badge>
                        : <span className="text-xs text-muted-foreground">{t("mise.inactive")}</span>}
                    </TableCell>
                    <TableCell>
                      <div className="flex justify-end gap-1">
                        {v !== dep.current_version && (
                          <Button variant="outline" size="sm" onClick={() => switchMutation.mutate(v)} disabled={running}>
                            <ArrowRightLeft className="mr-1 h-3.5 w-3.5" />{t("mise.activate")}
                          </Button>
                        )}
                        {v !== dep.current_version && (
                          <Button variant="ghost" size="icon" className="text-destructive" onClick={() => handleUninstallVersion(v)} disabled={running}>
                            <Trash2 className="h-3.5 w-3.5" />
                          </Button>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        </CardContent>
      </Card>

      <LogPanel logs={logs} running={running} scrollRef={scrollRef} />

      <Card>
        <CardContent className="py-4">
          <ConfirmDeleteButton onConfirm={handleDeleteAll} disabled={running} label={t("mise.uninstall")} />
        </CardContent>
      </Card>
    </div>
  );
}

function InstallVersionForm({ depName, versions, onInstall }: { depName: string; versions: string[]; onInstall: (version: string) => void }) {
  const { t } = useI18n();
  const [selected, setSelected] = useState("");

  return (
    <div className="space-y-4">
      <div>
        <Label>{t("mise.version")}</Label>
        <Select value={selected} onValueChange={setSelected}>
          <SelectTrigger><SelectValue placeholder={t("mise.select_version")} /></SelectTrigger>
          <SelectContent className="bg-popover z-50">
            {versions.map(v => <SelectItem key={v} value={v}>{v}</SelectItem>)}
          </SelectContent>
        </Select>
      </div>
      <Button onClick={() => onInstall(selected)} className="w-full" disabled={!selected}>
        <Download className="mr-1 h-3.5 w-3.5" />{t("mise.install")}
      </Button>
    </div>
  );
}

function TaskDetailView({ task, onBack }: { task: MiseTask; onBack: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [name, setName] = useState(task.name);
  const [description, setDescription] = useState(task.description ?? "");
  const [command, setCommand] = useState(task.command);
  const { logs, running, scrollRef, runLogs } = useMockLogs();

  const updateMutation = useMutation({
    mutationFn: () => api.updateMiseTask(task.id, { name, description, command }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["mise-tasks"] }); toast({ title: t("mise.task_updated") }); },
  });

  const deleteMutation = useMutation({
    mutationFn: () => api.deleteMiseTask(task.id),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["mise-tasks"] }); toast({ title: t("mise.task_deleted") }); onBack(); },
  });

  const handleRun = () => {
    const logLines = [
      `$ mise run ${task.name}`,
      `> ${task.command}`,
      ``,
      `Running task "${task.name}"...`,
      `[stdout] Processing...`,
      `[stdout] Done.`,
      `✓ Task "${task.name}" completed successfully (exit code 0)`,
    ];
    runLogs(logLines, () => {
      toast({ title: t("mise.task_executed") });
    });
    api.runMiseTask(task.name);
  };

  return (
    <div className="space-y-6">
      <PageBreadcrumb items={[
        { label: t("nav.services"), onClick: onBack },
        { label: t("nav.mise"), onClick: onBack },
        { label: task.name },
      ]} />

      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{task.name}</h1>
          {task.description && <p className="text-sm text-muted-foreground">{task.description}</p>}
        </div>
        <Button variant="outline" size="sm" onClick={handleRun} disabled={running}><Play className="mr-1 h-3.5 w-3.5" />{t("mise.run_task")}</Button>
      </div>

      <LogPanel logs={logs} running={running} scrollRef={scrollRef} />

      <Card>
        <CardHeader><CardTitle className="text-base">{t("mise.edit_task")}</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          <div><Label>{t("common.name")}</Label><Input value={name} onChange={e => setName(e.target.value)} /></div>
          <div><Label>{t("common.description")}</Label><Input value={description} onChange={e => setDescription(e.target.value)} /></div>
          <div><Label>{t("common.command")}</Label><Textarea value={command} onChange={e => setCommand(e.target.value)} className="font-mono text-xs" rows={5} /></div>
          <div className="flex gap-2">
            <Button onClick={() => updateMutation.mutate()}>{t("common.save_changes")}</Button>
            <ConfirmDeleteButton onConfirm={() => deleteMutation.mutate()} />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function AddDepForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [version, setVersion] = useState("");

  const createMutation = useMutation({
    mutationFn: () => api.createMiseDep({ name, version }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["mise-deps"] }); toast({ title: t("mise.dep_added") }); onClose(); },
  });

  return (
    <div className="space-y-4">
      <div><Label>{t("mise.tool_name")}</Label><Input value={name} onChange={e => setName(e.target.value)} placeholder="node" /></div>
      <div><Label>{t("mise.version")}</Label><Input value={version} onChange={e => setVersion(e.target.value)} placeholder="20.11.0" className="font-mono" /></div>
      <Button onClick={() => createMutation.mutate()} className="w-full" disabled={!name || !version}>{t("mise.add_dep")}</Button>
    </div>
  );
}

function CreateTaskForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [command, setCommand] = useState("");

  const createMutation = useMutation({
    mutationFn: () => api.createMiseTask({ name, description, command }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["mise-tasks"] }); toast({ title: t("mise.task_created") }); onClose(); },
  });

  return (
    <div className="space-y-4">
      <div><Label>{t("common.name")}</Label><Input value={name} onChange={e => setName(e.target.value)} placeholder="db:backup" /></div>
      <div><Label>{t("common.description")}</Label><Input value={description} onChange={e => setDescription(e.target.value)} /></div>
      <div><Label>{t("common.command")}</Label><Textarea value={command} onChange={e => setCommand(e.target.value)} className="font-mono text-xs" rows={3} placeholder="pg_dump -U postgres mydb > backup.sql" /></div>
      <Button onClick={() => createMutation.mutate()} className="w-full" disabled={!name || !command}>{t("mise.add_task")}</Button>
    </div>
  );
}
