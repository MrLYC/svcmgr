import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/services/api";
import { useI18n } from "@/i18n";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { StatusBadge } from "@/components/StatusBadge";
import { Plus, ExternalLink, Play, ChevronRight } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { toast } from "@/hooks/use-toast";
import { formatDistanceToNow } from "date-fns";
import { PageBreadcrumb } from "@/components/PageBreadcrumb";
import { ConfirmDeleteButton } from "@/components/ConfirmDeleteButton";
import type { TTYSession } from "@/types/api";

export default function TTYSessions() {
  const [selectedSession, setSelectedSession] = useState<TTYSession | null>(null);

  if (selectedSession) {
    return <SessionDetailView session={selectedSession} onBack={() => setSelectedSession(null)} />;
  }
  return <SessionListView onSelect={setSelectedSession} />;
}

function SessionListView({ onSelect }: { onSelect: (s: TTYSession) => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const { data: sessions, isLoading } = useQuery({ queryKey: ["tty-sessions"], queryFn: api.getTTYSessions });
  const [createOpen, setCreateOpen] = useState(false);

  const startMutation = useMutation({
    mutationFn: (id: string) => api.startTTYSession(id),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["tty-sessions"] }); toast({ title: t("tty.session_started") }); },
  });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">{t("tty.title")}</h1>
        <Dialog open={createOpen} onOpenChange={setCreateOpen}>
          <DialogTrigger asChild><Button size="sm"><Plus className="mr-1 h-3.5 w-3.5" />{t("tty.create_session")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("tty.create_session")}</DialogTitle></DialogHeader>
            <CreateSessionForm onClose={() => setCreateOpen(false)} />
          </DialogContent>
        </Dialog>
      </div>

      <div className="rounded-lg border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("common.name")}</TableHead>
              <TableHead>{t("common.command")}</TableHead>
              <TableHead>{t("common.status")}</TableHead>
              <TableHead>{t("common.created")}</TableHead>
              <TableHead className="text-right">{t("common.actions")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow><TableCell colSpan={5} className="text-center text-muted-foreground">{t("common.loading")}</TableCell></TableRow>
            ) : sessions?.map(s => (
              <TableRow key={s.id} className="cursor-pointer hover:bg-muted/50" onClick={() => onSelect(s)}>
                <TableCell className="font-medium">{s.name}</TableCell>
                <TableCell className="font-mono text-xs">{s.command}</TableCell>
                <TableCell><StatusBadge status={s.status} /></TableCell>
                <TableCell className="text-sm text-muted-foreground">{formatDistanceToNow(new Date(s.created_at), { addSuffix: true })}</TableCell>
                <TableCell onClick={e => e.stopPropagation()}>
                  <div className="flex justify-end gap-1">
                    {s.status === "running" && (
                      <Button variant="ghost" size="icon" asChild><a href={s.url} target="_blank" rel="noopener noreferrer"><ExternalLink className="h-3.5 w-3.5" /></a></Button>
                    )}
                    {s.status === "stopped" && (
                      <Button variant="ghost" size="icon" onClick={() => startMutation.mutate(s.id)}><Play className="h-3.5 w-3.5" /></Button>
                    )}
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

function SessionDetailView({ session, onBack }: { session: TTYSession; onBack: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [name, setName] = useState(session.name);
  const [command, setCommand] = useState(session.command);
  const [password, setPassword] = useState(session.password ?? false);

  const updateMutation = useMutation({
    mutationFn: () => api.updateTTYSession(session.id, { name, command, password }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["tty-sessions"] }); toast({ title: t("tty.session_updated") }); },
  });

  const deleteMutation = useMutation({
    mutationFn: () => api.deleteTTYSession(session.id),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["tty-sessions"] }); toast({ title: t("tty.session_deleted") }); onBack(); },
  });

  const startMutation = useMutation({
    mutationFn: () => api.startTTYSession(session.id),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["tty-sessions"] }); toast({ title: t("tty.session_started") }); },
  });

  return (
    <div className="space-y-6">
      <PageBreadcrumb items={[
        { label: t("nav.tty"), onClick: onBack },
        { label: session.name },
      ]} />

      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{session.name}</h1>
          <p className="text-sm text-muted-foreground font-mono">{session.command}</p>
        </div>
        <div className="flex items-center gap-2">
          <StatusBadge status={session.status} />
          {session.status === "running" && (
            <Button variant="outline" size="sm" asChild><a href={session.url} target="_blank" rel="noopener noreferrer"><ExternalLink className="mr-1 h-3.5 w-3.5" />{t("common.open")}</a></Button>
          )}
          {session.status === "stopped" && (
            <Button variant="outline" size="sm" onClick={() => startMutation.mutate()}><Play className="mr-1 h-3.5 w-3.5" />{t("tty.start_session")}</Button>
          )}
        </div>
      </div>

      <Card>
        <CardContent className="flex items-center gap-6 py-4 text-sm">
          <div><span className="text-muted-foreground">{t("common.created")}:</span> {formatDistanceToNow(new Date(session.created_at), { addSuffix: true })}</div>
          <div><span className="text-muted-foreground">URL:</span> <span className="font-mono">{session.url}</span></div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle className="text-base">{t("tty.edit_session")}</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          <div><Label>{t("tty.session_name")}</Label><Input value={name} onChange={e => setName(e.target.value)} /></div>
          <div><Label>{t("common.command")}</Label><Input value={command} onChange={e => setCommand(e.target.value)} className="font-mono" /></div>
          <div className="flex items-center justify-between">
            <Label>{t("tty.require_password")}</Label>
            <Switch checked={password} onCheckedChange={setPassword} />
          </div>
          <div className="flex gap-2">
            <Button onClick={() => updateMutation.mutate()}>{t("common.save_changes")}</Button>
            <ConfirmDeleteButton onConfirm={() => deleteMutation.mutate()} />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function CreateSessionForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [command, setCommand] = useState("/bin/bash");
  const [password, setPassword] = useState(false);

  const createMutation = useMutation({
    mutationFn: () => api.createTTYSession({ name, command, password }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["tty-sessions"] }); toast({ title: t("tty.session_created") }); onClose(); },
  });

  return (
    <div className="space-y-4">
      <div><Label>{t("tty.session_name")}</Label><Input value={name} onChange={e => setName(e.target.value)} placeholder="Main Terminal" /></div>
      <div><Label>{t("common.command")}</Label><Input value={command} onChange={e => setCommand(e.target.value)} className="font-mono" /></div>
      <div className="flex items-center justify-between">
        <Label>{t("tty.require_password")}</Label>
        <Switch checked={password} onCheckedChange={setPassword} />
      </div>
      <Button onClick={() => createMutation.mutate()} className="w-full" disabled={!name || !command}>{t("tty.create_session")}</Button>
    </div>
  );
}
