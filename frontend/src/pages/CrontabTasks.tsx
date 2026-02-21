import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/services/api";
import { useI18n } from "@/i18n";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Plus, ChevronRight } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { toast } from "@/hooks/use-toast";
import { formatDistanceToNow } from "date-fns";
import { PageBreadcrumb } from "@/components/PageBreadcrumb";
import { ConfirmDeleteButton } from "@/components/ConfirmDeleteButton";
import type { CrontabTask } from "@/types/api";

const CRON_PRESETS = [
  { label_key: "crontab.every_minute", value: "* * * * *" },
  { label_key: "crontab.every_5_min", value: "*/5 * * * *" },
  { label_key: "crontab.every_hour", value: "0 * * * *" },
  { label_key: "crontab.daily_midnight", value: "0 0 * * *" },
  { label_key: "crontab.daily_2am", value: "0 2 * * *" },
  { label_key: "crontab.weekly_sunday", value: "0 0 * * 0" },
  { label_key: "crontab.monthly", value: "0 0 1 * *" },
];

export default function CrontabTasks() {
  const [selectedTask, setSelectedTask] = useState<CrontabTask | null>(null);

  if (selectedTask) {
    return <TaskDetailView task={selectedTask} onBack={() => setSelectedTask(null)} />;
  }
  return <TaskListView onSelect={setSelectedTask} />;
}

function TaskListView({ onSelect }: { onSelect: (t: CrontabTask) => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const { data: tasks, isLoading } = useQuery({ queryKey: ["crontab-tasks"], queryFn: api.getCrontabs });
  const [createOpen, setCreateOpen] = useState(false);

  const toggleMutation = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) => api.toggleCrontab(id, enabled),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["crontab-tasks"] }),
  });

  const describeCron = (expr: string): string => {
    const preset = CRON_PRESETS.find(p => p.value === expr);
    return preset ? t(preset.label_key) : expr;
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">{t("crontab.title")}</h1>
        <Dialog open={createOpen} onOpenChange={setCreateOpen}>
          <DialogTrigger asChild><Button size="sm"><Plus className="mr-1 h-3.5 w-3.5" />{t("crontab.create_task")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("crontab.create_task")}</DialogTitle></DialogHeader>
            <CreateCrontabForm onClose={() => setCreateOpen(false)} />
          </DialogContent>
        </Dialog>
      </div>

      <div className="rounded-lg border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("crontab.schedule")}</TableHead>
              <TableHead>{t("common.command")}</TableHead>
              <TableHead>{t("common.enabled")}</TableHead>
              <TableHead>{t("crontab.last_run")}</TableHead>
              <TableHead className="text-right">{t("common.actions")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow><TableCell colSpan={5} className="text-center text-muted-foreground">{t("common.loading")}</TableCell></TableRow>
            ) : tasks?.map(task => (
              <TableRow key={task.id} className="cursor-pointer hover:bg-muted/50" onClick={() => onSelect(task)}>
                <TableCell>
                  <div>
                    <code className="font-mono text-xs bg-muted px-1.5 py-0.5 rounded">{task.expression}</code>
                    <div className="text-xs text-muted-foreground mt-1">{describeCron(task.expression)}</div>
                  </div>
                </TableCell>
                <TableCell className="font-mono text-xs max-w-[300px] truncate">{task.command}</TableCell>
                <TableCell onClick={e => e.stopPropagation()}><Switch checked={task.enabled} onCheckedChange={v => toggleMutation.mutate({ id: task.id, enabled: v })} /></TableCell>
                <TableCell className="text-sm text-muted-foreground">{task.last_run ? formatDistanceToNow(new Date(task.last_run), { addSuffix: true }) : "—"}</TableCell>
                <TableCell className="text-right"><ChevronRight className="h-4 w-4 text-muted-foreground inline-block" /></TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function TaskDetailView({ task, onBack }: { task: CrontabTask; onBack: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [expression, setExpression] = useState(task.expression);
  const [command, setCommand] = useState(task.command);
  const [description, setDescription] = useState(task.description ?? "");
  const [enabled, setEnabled] = useState(task.enabled);

  const updateMutation = useMutation({
    mutationFn: () => api.updateCrontab(task.id, { expression, command, description, enabled }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["crontab-tasks"] }); toast({ title: t("crontab.task_updated") }); },
  });

  const deleteMutation = useMutation({
    mutationFn: () => api.deleteCrontab(task.id),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["crontab-tasks"] }); toast({ title: t("crontab.task_deleted") }); onBack(); },
  });

  const describeCron = (expr: string): string => {
    const preset = CRON_PRESETS.find(p => p.value === expr);
    return preset ? t(preset.label_key) : expr;
  };

  return (
    <div className="space-y-6">
      <PageBreadcrumb items={[
        { label: t("nav.services"), onClick: onBack },
        { label: t("nav.crontab"), onClick: onBack },
        { label: task.description || task.command },
      ]} />

      <div>
        <h1 className="text-2xl font-bold tracking-tight">{task.description || task.command}</h1>
        <p className="text-sm text-muted-foreground font-mono">{task.expression}</p>
      </div>

      <Card>
        <CardContent className="flex items-center gap-6 py-4 text-sm">
          <div><span className="text-muted-foreground">{t("crontab.last_run")}:</span> {task.last_run ? formatDistanceToNow(new Date(task.last_run), { addSuffix: true }) : "—"}</div>
          <div className="flex items-center gap-2"><span className="text-muted-foreground">{t("common.enabled")}:</span> <Switch checked={enabled} onCheckedChange={setEnabled} /></div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle className="text-base">{t("crontab.edit_task")}</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          <div>
            <Label>{t("crontab.schedule_preset")}</Label>
            <Select value={expression} onValueChange={setExpression}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>{CRON_PRESETS.map(p => <SelectItem key={p.value} value={p.value}>{t(p.label_key)}</SelectItem>)}</SelectContent>
            </Select>
          </div>
          <div>
            <Label>{t("crontab.cron_expression")}</Label>
            <Input value={expression} onChange={e => setExpression(e.target.value)} className="font-mono" />
            <p className="text-xs text-muted-foreground mt-1">{describeCron(expression)}</p>
          </div>
          <div><Label>{t("common.command")}</Label><Input value={command} onChange={e => setCommand(e.target.value)} className="font-mono" /></div>
          <div><Label>{t("common.description")}</Label><Input value={description} onChange={e => setDescription(e.target.value)} /></div>
          <div className="flex gap-2">
            <Button onClick={() => updateMutation.mutate()}>{t("common.save_changes")}</Button>
            <ConfirmDeleteButton onConfirm={() => deleteMutation.mutate()} />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function CreateCrontabForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [expression, setExpression] = useState("0 * * * *");
  const [command, setCommand] = useState("");
  const [description, setDescription] = useState("");

  const describeCron = (expr: string): string => {
    const preset = CRON_PRESETS.find(p => p.value === expr);
    return preset ? t(preset.label_key) : expr;
  };

  const createMutation = useMutation({
    mutationFn: () => api.createCrontab({ expression, command, description, enabled: true }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["crontab-tasks"] }); toast({ title: t("crontab.task_created") }); onClose(); },
  });

  return (
    <div className="space-y-4">
      <div>
        <Label>{t("crontab.schedule_preset")}</Label>
        <Select value={expression} onValueChange={setExpression}>
          <SelectTrigger><SelectValue /></SelectTrigger>
          <SelectContent>{CRON_PRESETS.map(p => <SelectItem key={p.value} value={p.value}>{t(p.label_key)}</SelectItem>)}</SelectContent>
        </Select>
      </div>
      <div>
        <Label>{t("crontab.cron_expression")}</Label>
        <Input value={expression} onChange={e => setExpression(e.target.value)} className="font-mono" />
        <p className="text-xs text-muted-foreground mt-1">{describeCron(expression)}</p>
      </div>
      <div><Label>{t("common.command")}</Label><Input value={command} onChange={e => setCommand(e.target.value)} className="font-mono" placeholder="/usr/local/bin/script.sh" /></div>
      <div><Label>{t("common.description")}</Label><Input value={description} onChange={e => setDescription(e.target.value)} /></div>
      <Button onClick={() => createMutation.mutate()} className="w-full" disabled={!command}>{t("crontab.create_task")}</Button>
    </div>
  );
}
