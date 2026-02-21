import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/services/api";
import { useI18n } from "@/i18n";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { StatusBadge } from "@/components/StatusBadge";
import { Badge } from "@/components/ui/badge";
import { GitBranch, GitCommit, RotateCcw, Plus, FolderOpen, Trash2, ChevronRight } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Checkbox } from "@/components/ui/checkbox";
import { toast } from "@/hooks/use-toast";
import { formatDistanceToNow } from "date-fns";
import { PageBreadcrumb } from "@/components/PageBreadcrumb";
import { ConfirmDeleteButton } from "@/components/ConfirmDeleteButton";
import type { ManagedDirectory } from "@/types/api";

export default function ConfigManagement() {
  const [selectedDir, setSelectedDir] = useState<ManagedDirectory | null>(null);

  if (selectedDir) {
    return <DirDetailView dir={selectedDir} onBack={() => setSelectedDir(null)} />;
  }

  return <DirListView onSelect={setSelectedDir} />;
}

function DirListView({ onSelect }: { onSelect: (dir: ManagedDirectory) => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const { data: dirs } = useQuery({ queryKey: ["config-dirs"], queryFn: api.getManagedDirs });
  const [addDirOpen, setAddDirOpen] = useState(false);

  const removeDirMutation = useMutation({
    mutationFn: (id: string) => api.removeManagedDir(id),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["config-dirs"] }); toast({ title: t("config.dir_removed") }); },
  });

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">{t("config.title")}</h1>
        <Dialog open={addDirOpen} onOpenChange={setAddDirOpen}>
          <DialogTrigger asChild><Button size="sm" variant="outline"><Plus className="mr-1 h-3.5 w-3.5" />{t("config.add_dir")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("config.add_dir")}</DialogTitle></DialogHeader>
            <AddDirForm onClose={() => setAddDirOpen(false)} />
          </DialogContent>
        </Dialog>
      </div>

      <Card>
        <CardHeader><CardTitle className="text-base flex items-center gap-2"><FolderOpen className="h-4 w-4" />{t("config.managed_dirs")}</CardTitle></CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("common.name")}</TableHead>
                <TableHead>{t("common.path")}</TableHead>
                <TableHead>{t("common.status")}</TableHead>
                <TableHead>{t("config.last_commit")}</TableHead>
                <TableHead className="text-right">{t("common.actions")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {dirs?.map(d => (
                <TableRow key={d.id} className="cursor-pointer hover:bg-muted/50" onClick={() => onSelect(d)}>
                  <TableCell className="font-medium">{d.label}</TableCell>
                  <TableCell><code className="font-mono text-xs bg-muted px-1.5 py-0.5 rounded">{d.path}</code></TableCell>
                  <TableCell>
                    <div className="flex items-center gap-2">
                      <StatusBadge status={d.git_status ?? "clean"} />
                      {(d.uncommitted_changes ?? 0) > 0 && (
                        <span className="text-xs text-muted-foreground">{d.uncommitted_changes} {t("config.changes_count")}</span>
                      )}
                    </div>
                  </TableCell>
                  <TableCell>
                    <div className="text-sm text-muted-foreground truncate max-w-[200px]">{d.last_commit_message ?? "-"}</div>
                    {d.last_commit_time && <div className="text-xs text-muted-foreground">{formatDistanceToNow(new Date(d.last_commit_time), { addSuffix: true })}</div>}
                  </TableCell>
                  <TableCell className="text-right" onClick={e => e.stopPropagation()}>
                    <div className="flex items-center justify-end gap-1">
                      <ConfirmDeleteButton onConfirm={() => removeDirMutation.mutate(d.id)} label={t("config.remove_dir")} />
                      <ChevronRight className="h-4 w-4 text-muted-foreground" />
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}

function DirDetailView({ dir, onBack }: { dir: ManagedDirectory; onBack: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const { data: changes } = useQuery({ queryKey: ["config-changes", dir.id], queryFn: () => api.getConfigChanges(dir.id) });
  const { data: commits } = useQuery({ queryKey: ["config-commits", dir.id], queryFn: () => api.getConfigCommits(dir.id) });
  const [commitOpen, setCommitOpen] = useState(false);

  const rollbackMutation = useMutation({
    mutationFn: (hash: string) => api.rollbackConfig(hash, dir.id),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["config-changes", "config-commits"] }); toast({ title: t("config.rollback_done") }); },
  });

  return (
    <div className="space-y-6">
      <PageBreadcrumb items={[
        { label: t("nav.config"), onClick: onBack },
        { label: dir.label },
      ]} />

      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{dir.label}</h1>
          <p className="text-sm text-muted-foreground font-mono">{dir.path}</p>
        </div>
        {changes && changes.length > 0 && (
          <Dialog open={commitOpen} onOpenChange={setCommitOpen}>
            <DialogTrigger asChild><Button size="sm"><GitCommit className="mr-1 h-3.5 w-3.5" />{t("config.commit_changes")}</Button></DialogTrigger>
            <DialogContent>
              <DialogHeader><DialogTitle>{t("config.commit_changes")}</DialogTitle></DialogHeader>
              <CommitForm changes={changes} dirId={dir.id} onClose={() => setCommitOpen(false)} />
            </DialogContent>
          </Dialog>
        )}
      </div>

      <Card>
        <CardContent className="flex items-center gap-6 py-4">
          <div className="flex items-center gap-2 text-sm"><GitBranch className="h-4 w-4 text-muted-foreground" /><span className="font-mono">{dir.branch ?? "main"}</span></div>
          <StatusBadge status={dir.git_status ?? "clean"} />
          <div className="text-sm text-muted-foreground">{dir.uncommitted_changes ?? 0} {t("config.uncommitted")}</div>
        </CardContent>
      </Card>

      {changes && changes.length > 0 && (
        <Card>
          <CardHeader><CardTitle className="text-base">{t("config.pending_changes")}</CardTitle></CardHeader>
          <CardContent>
            <div className="space-y-2">
              {changes.map(c => (
                <div key={c.file} className="flex items-center gap-3 text-sm">
                  <StatusBadge status={c.status} />
                  <span className="font-mono">{c.file}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader><CardTitle className="text-base">{t("config.commit_history")}</CardTitle></CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("config.hash")}</TableHead>
                <TableHead>{t("config.message")}</TableHead>
                <TableHead>{t("config.author")}</TableHead>
                <TableHead>{t("config.time")}</TableHead>
                <TableHead className="text-right">{t("common.actions")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {commits?.map(c => (
                <TableRow key={c.hash}>
                  <TableCell><code className="font-mono text-xs bg-muted px-1.5 py-0.5 rounded">{c.hash}</code></TableCell>
                  <TableCell className="text-sm">{c.message}</TableCell>
                  <TableCell className="text-sm text-muted-foreground">{c.author}</TableCell>
                  <TableCell className="text-sm text-muted-foreground">{formatDistanceToNow(new Date(c.timestamp), { addSuffix: true })}</TableCell>
                  <TableCell className="text-right">
                    <Button variant="ghost" size="icon" onClick={() => rollbackMutation.mutate(c.hash)}><RotateCcw className="h-3.5 w-3.5" /></Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}

function AddDirForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [path, setPath] = useState("");
  const [label, setLabel] = useState("");

  const addMutation = useMutation({
    mutationFn: () => api.addManagedDir(path, label),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["config-dirs"] }); toast({ title: t("config.dir_added") }); onClose(); },
  });

  return (
    <div className="space-y-4">
      <div><Label>{t("config.dir_path")}</Label><Input value={path} onChange={e => setPath(e.target.value)} placeholder="/etc/nginx" className="font-mono" /></div>
      <div><Label>{t("common.name")}</Label><Input value={label} onChange={e => setLabel(e.target.value)} placeholder="nginx" /></div>
      <Button onClick={() => addMutation.mutate()} className="w-full" disabled={!path || !label}>{t("config.add_dir")}</Button>
    </div>
  );
}

function CommitForm({ changes, dirId, onClose }: { changes: { file: string; status: string }[]; dirId: string; onClose: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [message, setMessage] = useState("");
  const [selected, setSelected] = useState<string[]>(changes.map(c => c.file));

  const commitMutation = useMutation({
    mutationFn: () => api.commitConfig(message, selected, dirId),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["config-changes", "config-commits"] }); toast({ title: t("config.committed") }); onClose(); },
  });

  return (
    <div className="space-y-4">
      <div><Label>{t("config.commit_message")}</Label><Input value={message} onChange={e => setMessage(e.target.value)} placeholder="fix: update configuration" /></div>
      <div>
        <Label>{t("config.files")}</Label>
        <div className="mt-2 space-y-2">
          {changes.map(c => (
            <label key={c.file} className="flex items-center gap-2 text-sm">
              <Checkbox checked={selected.includes(c.file)} onCheckedChange={v => setSelected(s => v ? [...s, c.file] : s.filter(f => f !== c.file))} />
              <span className="font-mono">{c.file}</span>
            </label>
          ))}
        </div>
      </div>
      <Button onClick={() => commitMutation.mutate()} className="w-full" disabled={!message || selected.length === 0}>{t("config.commit")}</Button>
    </div>
  );
}
