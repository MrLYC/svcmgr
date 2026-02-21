import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/services/api";
import { useI18n } from "@/i18n";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { StatusDot } from "@/components/StatusBadge";
import { Plus, ChevronRight } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBadge } from "@/components/StatusBadge";
import { toast } from "@/hooks/use-toast";
import { PageBreadcrumb } from "@/components/PageBreadcrumb";
import { ConfirmDeleteButton } from "@/components/ConfirmDeleteButton";
import type { CloudflareTunnel } from "@/types/api";

export default function CloudflareTunnels() {
  const [selectedTunnel, setSelectedTunnel] = useState<CloudflareTunnel | null>(null);

  if (selectedTunnel) {
    return <TunnelDetailView tunnel={selectedTunnel} onBack={() => setSelectedTunnel(null)} />;
  }
  return <TunnelListView onSelect={setSelectedTunnel} />;
}

function TunnelListView({ onSelect }: { onSelect: (t: CloudflareTunnel) => void }) {
  const { t } = useI18n();
  const [createOpen, setCreateOpen] = useState(false);
  const { data: tunnels, isLoading } = useQuery({ queryKey: ["cf-tunnels"], queryFn: api.getTunnels });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">{t("cf.title")}</h1>
        <Dialog open={createOpen} onOpenChange={setCreateOpen}>
          <DialogTrigger asChild><Button size="sm"><Plus className="mr-1 h-3.5 w-3.5" />{t("cf.create_tunnel")}</Button></DialogTrigger>
          <DialogContent>
            <DialogHeader><DialogTitle>{t("cf.create_tunnel")}</DialogTitle></DialogHeader>
            <CreateTunnelForm onClose={() => setCreateOpen(false)} />
          </DialogContent>
        </Dialog>
      </div>

      <div className="rounded-lg border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("common.status")}</TableHead>
              <TableHead>{t("common.name")}</TableHead>
              <TableHead>{t("common.domain")}</TableHead>
              <TableHead>{t("cf.service")}</TableHead>
              <TableHead>{t("common.uptime")}</TableHead>
              <TableHead className="text-right">{t("common.actions")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow><TableCell colSpan={6} className="text-center text-muted-foreground">{t("common.loading")}</TableCell></TableRow>
            ) : tunnels?.map(tun => (
              <TableRow key={tun.id} className="cursor-pointer hover:bg-muted/50" onClick={() => onSelect(tun)}>
                <TableCell><StatusDot status={tun.status} /></TableCell>
                <TableCell className="font-medium">{tun.name}</TableCell>
                <TableCell className="font-mono text-sm">{tun.domain}</TableCell>
                <TableCell className="font-mono text-xs">{tun.service_url}</TableCell>
                <TableCell className="text-sm text-muted-foreground">{tun.uptime ?? "—"}</TableCell>
                <TableCell className="text-right"><ChevronRight className="h-4 w-4 text-muted-foreground inline-block" /></TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

function TunnelDetailView({ tunnel, onBack }: { tunnel: CloudflareTunnel; onBack: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [name, setName] = useState(tunnel.name);
  const [domain, setDomain] = useState(tunnel.domain);
  const [serviceUrl, setServiceUrl] = useState(tunnel.service_url);

  const updateMutation = useMutation({
    mutationFn: () => api.updateTunnel(tunnel.id, { name, domain, service_url: serviceUrl }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["cf-tunnels"] }); toast({ title: t("cf.tunnel_updated") }); },
  });

  const deleteMutation = useMutation({
    mutationFn: () => api.deleteTunnel(tunnel.id),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["cf-tunnels"] }); toast({ title: t("cf.tunnel_deleted") }); onBack(); },
  });

  return (
    <div className="space-y-6">
      <PageBreadcrumb items={[
        { label: t("nav.proxy"), onClick: onBack },
        { label: t("nav.cloudflare"), onClick: onBack },
        { label: tunnel.name },
      ]} />

      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{tunnel.name}</h1>
          <p className="text-sm text-muted-foreground font-mono">{tunnel.domain}</p>
        </div>
        <div className="flex items-center gap-2">
          <StatusBadge status={tunnel.status} />
          {tunnel.uptime && <span className="text-sm text-muted-foreground">{t("common.uptime")}: {tunnel.uptime}</span>}
        </div>
      </div>

      <Card>
        <CardHeader><CardTitle className="text-base">{t("cf.edit_tunnel")}</CardTitle></CardHeader>
        <CardContent className="space-y-4">
          <div><Label>{t("cf.tunnel_name")}</Label><Input value={name} onChange={e => setName(e.target.value)} /></div>
          <div><Label>{t("common.domain")}</Label><Input value={domain} onChange={e => setDomain(e.target.value)} className="font-mono" /></div>
          <div><Label>{t("cf.service_url")}</Label><Input value={serviceUrl} onChange={e => setServiceUrl(e.target.value)} className="font-mono" /></div>
          <div className="flex gap-2">
            <Button onClick={() => updateMutation.mutate()}>{t("common.save_changes")}</Button>
            <ConfirmDeleteButton onConfirm={() => deleteMutation.mutate()} />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function CreateTunnelForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [domain, setDomain] = useState("");
  const [serviceUrl, setServiceUrl] = useState("");

  const createMutation = useMutation({
    mutationFn: () => api.createTunnel({ name, domain, service_url: serviceUrl }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["cf-tunnels"] }); toast({ title: t("cf.tunnel_created") }); onClose(); },
  });

  return (
    <div className="space-y-4">
      <div><Label>{t("cf.tunnel_name")}</Label><Input value={name} onChange={e => setName(e.target.value)} placeholder="my-tunnel" /></div>
      <div><Label>{t("common.domain")}</Label><Input value={domain} onChange={e => setDomain(e.target.value)} placeholder="app.example.com" className="font-mono" /></div>
      <div><Label>{t("cf.service_url")}</Label><Input value={serviceUrl} onChange={e => setServiceUrl(e.target.value)} placeholder="http://localhost:8080" className="font-mono" /></div>
      <Button onClick={() => createMutation.mutate()} className="w-full" disabled={!name || !domain || !serviceUrl}>{t("cf.create_tunnel")}</Button>
    </div>
  );
}
