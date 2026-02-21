import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/services/api";
import { useI18n } from "@/i18n";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { StatusBadge } from "@/components/StatusBadge";
import { Plus, Zap, ChevronRight, Lock } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { toast } from "@/hooks/use-toast";
import { PageBreadcrumb } from "@/components/PageBreadcrumb";
import { ConfirmDeleteButton } from "@/components/ConfirmDeleteButton";
import type { NginxProxy } from "@/types/api";

export default function NginxProxies() {
  const [selectedProxy, setSelectedProxy] = useState<NginxProxy | null>(null);

  if (selectedProxy) {
    return <ProxyDetailView proxy={selectedProxy} onBack={() => setSelectedProxy(null)} />;
  }
  return <ProxyListView onSelect={setSelectedProxy} />;
}

function ProxyListView({ onSelect }: { onSelect: (p: NginxProxy) => void }) {
  const { t } = useI18n();
  const [createOpen, setCreateOpen] = useState(false);
  const { data: proxies, isLoading } = useQuery({ queryKey: ["nginx-proxies"], queryFn: api.getNginxProxies });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">{t("nginx.title")}</h1>
        <Dialog open={createOpen} onOpenChange={setCreateOpen}>
          <DialogTrigger asChild><Button size="sm"><Plus className="mr-1 h-3.5 w-3.5" />{t("nginx.create_proxy")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("nginx.create_proxy")}</DialogTitle></DialogHeader>
            <CreateProxyForm onClose={() => setCreateOpen(false)} />
          </DialogContent>
        </Dialog>
      </div>

      <div className="rounded-lg border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("common.path")}</TableHead>
              <TableHead>{t("common.type")}</TableHead>
              <TableHead>{t("common.target")}</TableHead>
              <TableHead>{t("common.status")}</TableHead>
              <TableHead className="text-right">{t("common.actions")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow><TableCell colSpan={5} className="text-center text-muted-foreground">{t("common.loading")}</TableCell></TableRow>
            ) : proxies?.map(p => (
              <TableRow key={p.id} className="cursor-pointer hover:bg-muted/50" onClick={() => onSelect(p)}>
                <TableCell className="font-mono text-sm flex items-center gap-2">
                  {p.path}
                  {p.built_in && <Badge variant="secondary" className="text-[10px] px-1.5 py-0 gap-1"><Lock className="h-2.5 w-2.5" />{t("nginx.built_in")}</Badge>}
                </TableCell>
                <TableCell><Badge variant="outline" className="capitalize">{p.proxy_type}</Badge></TableCell>
                <TableCell className="font-mono text-xs">{p.target}</TableCell>
                <TableCell><StatusBadge status={p.status} /></TableCell>
                <TableCell className="text-right"><ChevronRight className="h-4 w-4 text-muted-foreground inline-block" /></TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function ProxyDetailView({ proxy, onBack }: { proxy: NginxProxy; onBack: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [proxyType, setProxyType] = useState(proxy.proxy_type);
  const [path, setPath] = useState(proxy.path);
  const [target, setTarget] = useState(proxy.target);

  const updateMutation = useMutation({
    mutationFn: () => api.updateNginxProxy(proxy.id, { path, proxy_type: proxyType, target }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["nginx-proxies"] }); toast({ title: t("nginx.proxy_updated") }); },
  });

  const deleteMutation = useMutation({
    mutationFn: () => api.deleteNginxProxy(proxy.id),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["nginx-proxies"] }); toast({ title: t("nginx.proxy_deleted") }); onBack(); },
  });

  const testMutation = useMutation({
    mutationFn: () => api.testNginxProxy(proxy.id),
    onSuccess: (data) => toast({ title: t("nginx.connectivity_test"), description: `Status: ${data.status} | Time: ${data.time}ms` }),
  });

  const isBuiltIn = !!proxy.built_in;

  return (
    <div className="space-y-6">
      <PageBreadcrumb items={[
        { label: t("nav.proxy"), onClick: onBack },
        { label: t("nav.nginx"), onClick: onBack },
        { label: proxy.path },
      ]} />

      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight flex items-center gap-2">
            {proxy.path}
            {isBuiltIn && <Badge variant="secondary" className="text-xs gap-1"><Lock className="h-3 w-3" />{t("nginx.built_in")}</Badge>}
          </h1>
          <p className="text-sm text-muted-foreground font-mono">{proxy.target}</p>
        </div>
        <div className="flex items-center gap-2">
          <StatusBadge status={proxy.status} />
          <Button variant="outline" size="sm" onClick={() => testMutation.mutate()}><Zap className="mr-1 h-3.5 w-3.5" />{t("nginx.connectivity_test")}</Button>
        </div>
      </div>

      {isBuiltIn && (
        <div className="rounded-lg border border-muted bg-muted/30 px-4 py-3 text-sm text-muted-foreground flex items-center gap-2">
          <Lock className="h-4 w-4 shrink-0" />
          {t("nginx.built_in_hint")}
        </div>
      )}

      <Card>
        <CardHeader><CardTitle className="text-base">{t("nginx.edit_proxy")}</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          <div><Label>{t("nginx.proxy_type")}</Label>
            <Select value={proxyType} onValueChange={v => setProxyType(v as any)} disabled={isBuiltIn}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="static">{t("nginx.static_files")}</SelectItem>
                <SelectItem value="http">{t("nginx.http_proxy")}</SelectItem>
                <SelectItem value="tcp">{t("nginx.tcp_proxy")}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div><Label>{t("common.path")}</Label><Input value={path} onChange={e => setPath(e.target.value)} className="font-mono" disabled={isBuiltIn} /></div>
          <div><Label>{proxyType === "static" ? t("nginx.root_dir") : t("common.target")}</Label>
            <Input value={target} onChange={e => setTarget(e.target.value)} className="font-mono" disabled={isBuiltIn} />
          </div>
          <div className="flex gap-2">
            {!isBuiltIn && (
              <>
                <Button onClick={() => updateMutation.mutate()}>{t("common.save_changes")}</Button>
                <ConfirmDeleteButton onConfirm={() => deleteMutation.mutate()} />
              </>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function CreateProxyForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [proxyType, setProxyType] = useState<"static" | "http" | "tcp">("http");
  const [path, setPath] = useState("");
  const [target, setTarget] = useState("");

  const createMutation = useMutation({
    mutationFn: () => api.createNginxProxy({ path, proxy_type: proxyType, target }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["nginx-proxies"] }); toast({ title: t("nginx.proxy_created") }); onClose(); },
  });

  return (
    <div className="space-y-4">
      <div><Label>{t("nginx.proxy_type")}</Label>
        <Select value={proxyType} onValueChange={v => setProxyType(v as any)}>
          <SelectTrigger><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="static">{t("nginx.static_files")}</SelectItem>
            <SelectItem value="http">{t("nginx.http_proxy")}</SelectItem>
            <SelectItem value="tcp">{t("nginx.tcp_proxy")}</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div><Label>{t("common.path")}</Label><Input value={path} onChange={e => setPath(e.target.value)} placeholder="/api" className="font-mono" /></div>
      <div><Label>{proxyType === "static" ? t("nginx.root_dir") : t("common.target")}</Label>
        <Input value={target} onChange={e => setTarget(e.target.value)} placeholder={proxyType === "static" ? "/var/www/html" : proxyType === "http" ? "http://127.0.0.1:8080" : "127.0.0.1:9090"} className="font-mono" />
      </div>
      <Button onClick={() => createMutation.mutate()} className="w-full" disabled={!path || !target}>{t("nginx.create_proxy")}</Button>
    </div>
  );
}
