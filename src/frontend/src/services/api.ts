import type {
  SystemdService, SystemdLog, CrontabTask,
  MiseDependency, MiseTask, NginxProxy,
  CloudflareTunnel, TTYSession, ManagedDirectory, ConfigStatus,
  ConfigChange, ConfigCommit, SettingsConfig,
  ToolStatus, ActivityLog, DashboardStats,
} from "@/types/api";

const API_BASE = "/svcmgr/api";
const USE_MOCK = true; // Toggle to false when backend is ready

async function apiFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  if (!res.ok) throw new Error(`API error: ${res.status}`);
  return res.json();
}

// ─── Mock Data ───────────────────────────────────────────────
const mockServices: SystemdService[] = [
  { name: "nginx.service", status: "running", enabled: true, pid: 1234, memory: "12.4 MB", uptime: "3d 4h", description: "Nginx HTTP Server" },
  { name: "svcmgr.service", status: "running", enabled: true, pid: 5678, memory: "28.1 MB", uptime: "3d 4h", description: "Service Manager" },
  { name: "redis.service", status: "stopped", enabled: false, description: "Redis In-Memory Store" },
  { name: "postgresql.service", status: "running", enabled: true, pid: 9012, memory: "64.2 MB", uptime: "7d 12h", description: "PostgreSQL Database" },
  { name: "backup.service", status: "failed", enabled: true, description: "Daily Backup Service" },
];

const mockCrontabs: CrontabTask[] = [
  { id: "1", expression: "0 2 * * *", command: "/usr/local/bin/backup.sh", enabled: true, description: "Daily backup at 2 AM", last_run: "2026-02-21T02:00:00Z" },
  { id: "2", expression: "*/5 * * * *", command: "/usr/local/bin/health-check.sh", enabled: true, description: "Health check every 5 minutes", last_run: "2026-02-21T10:55:00Z" },
  { id: "3", expression: "0 0 * * 0", command: "/usr/local/bin/cleanup.sh", enabled: false, description: "Weekly cleanup on Sunday" },
];

const mockMiseDeps: MiseDependency[] = [
  { id: "1", name: "node", current_version: "20.11.0", latest_version: "22.4.0", source: "mise", installed_versions: ["18.20.0", "20.11.0"] },
  { id: "2", name: "python", current_version: "3.12.1", latest_version: "3.13.0", source: "mise", installed_versions: ["3.11.0", "3.12.1"] },
  { id: "3", name: "rust", current_version: "1.82.0", latest_version: "1.84.0", source: "mise", installed_versions: ["1.80.0", "1.82.0"] },
];

const mockMiseTasks: MiseTask[] = [
  { id: "1", name: "db:backup", description: "Backup PostgreSQL database", command: "pg_dump -U postgres mydb > backup.sql" },
  { id: "2", name: "logs:clean", description: "Clean old log files", command: "find /var/log -name '*.log' -mtime +30 -delete" },
  { id: "3", name: "health:check", description: "Run health checks", command: "curl -sf http://localhost:8080/health" },
];

const mockNginxProxies: NginxProxy[] = [
  { id: "sys-1", path: "/svcmgr", proxy_type: "http", target: "http://127.0.0.1:8080", status: "active", built_in: true },
  { id: "sys-2", path: "/tty", proxy_type: "http", target: "http://127.0.0.1:7681", status: "active", built_in: true },
  { id: "1", path: "/", proxy_type: "static", target: "/var/www/html", status: "active", root: "/var/www/html" },
  { id: "2", path: "/api", proxy_type: "http", target: "http://127.0.0.1:3000", status: "active" },
  { id: "3", path: "/ws", proxy_type: "tcp", target: "127.0.0.1:9090", status: "inactive", port: 9090 },
];

const mockTunnels: CloudflareTunnel[] = [
  { id: "1", name: "main-tunnel", domain: "app.example.com", service_url: "http://localhost:8080", status: "connected", uptime: "14d 6h" },
  { id: "2", name: "dev-tunnel", domain: "dev.example.com", service_url: "http://localhost:3000", status: "disconnected" },
];

const mockTTYSessions: TTYSession[] = [
  { id: "1", name: "Main Terminal", command: "/bin/bash", url: "http://localhost:7681", status: "running", created_at: "2026-02-20T10:00:00Z" },
  { id: "2", name: "Monitoring", command: "htop", url: "http://localhost:7682", status: "stopped", created_at: "2026-02-19T08:00:00Z" },
];

const mockManagedDirs: ManagedDirectory[] = [
  { id: "1", path: "/etc/svcmgr/mise", label: "mise", git_status: "clean", branch: "main", uncommitted_changes: 0, last_commit_message: "init: mise config", last_commit_time: "2026-02-19T10:00:00Z" },
  { id: "2", path: "/etc/svcmgr/systemd", label: "systemd", git_status: "dirty", branch: "main", uncommitted_changes: 2, last_commit_message: "fix: update backup service", last_commit_time: "2026-02-20T15:30:00Z" },
  { id: "3", path: "/etc/svcmgr/crontab", label: "crontab", git_status: "clean", branch: "main", uncommitted_changes: 0, last_commit_message: "chore: cleanup old jobs", last_commit_time: "2026-02-18T09:00:00Z" },
  { id: "4", path: "/etc/svcmgr/nginx", label: "nginx", git_status: "dirty", branch: "main", uncommitted_changes: 1, last_commit_message: "fix: update nginx config", last_commit_time: "2026-02-20T15:30:00Z" },
  { id: "5", path: "/etc/svcmgr/cloudflare", label: "cloudflare", git_status: "clean", branch: "main", uncommitted_changes: 0, last_commit_message: "feat: add tunnel config", last_commit_time: "2026-02-17T14:00:00Z" },
];

const mockConfigStatus: ConfigStatus = {
  path: "/etc/svcmgr", git_status: "dirty", branch: "main", uncommitted_changes: 3, last_commit: "fix: update nginx config",
};

const mockConfigChanges: ConfigChange[] = [
  { file: "nginx/default.conf", status: "modified" },
  { file: "systemd/backup.service", status: "added" },
  { file: "crontab/old-job", status: "deleted" },
];

const mockConfigCommits: ConfigCommit[] = [
  { hash: "a1b2c3d", message: "fix: update nginx config", author: "admin", timestamp: "2026-02-20T15:30:00Z" },
  { hash: "e4f5g6h", message: "feat: add backup service", author: "admin", timestamp: "2026-02-19T10:00:00Z" },
  { hash: "i7j8k9l", message: "chore: cleanup old configs", author: "admin", timestamp: "2026-02-18T09:00:00Z" },
];

const mockSettings: SettingsConfig = { nginx_port: 80, config_dir: "/etc/svcmgr", auto_commit: true, log_level: "info" };

const mockTools: ToolStatus[] = [
  { name: "systemctl", installed: true, version: "255", path: "/usr/bin/systemctl" },
  { name: "nginx", installed: true, version: "1.24.0", path: "/usr/sbin/nginx" },
  { name: "cloudflared", installed: true, version: "2024.2.1", path: "/usr/local/bin/cloudflared" },
  { name: "mise", installed: true, version: "2024.11.0", path: "/usr/local/bin/mise" },
  { name: "ttyd", installed: false },
];

const mockActivity: ActivityLog[] = [
  { id: "1", timestamp: "2026-02-21T10:55:00Z", type: "systemd", action: "restart", description: "Restarted nginx.service" },
  { id: "2", timestamp: "2026-02-21T10:30:00Z", type: "crontab", action: "run", description: "Executed health-check.sh" },
  { id: "3", timestamp: "2026-02-21T09:00:00Z", type: "config", action: "commit", description: "Committed nginx config changes" },
  { id: "4", timestamp: "2026-02-20T18:00:00Z", type: "nginx", action: "create", description: "Created proxy /api -> localhost:8080" },
  { id: "5", timestamp: "2026-02-20T15:30:00Z", type: "cloudflare", action: "connect", description: "Tunnel main-tunnel connected" },
];

// ─── API Functions ───────────────────────────────────────────
export const api = {
  // Dashboard
  getDashboardStats: (): Promise<DashboardStats> =>
    USE_MOCK ? Promise.resolve({ systemd_running: 3, systemd_total: 5, crontab_tasks: 3, nginx_proxies: 3, cloudflare_connected: 1, cloudflare_total: 2 })
      : apiFetch("/dashboard/stats"),
  getActivityLogs: (): Promise<ActivityLog[]> =>
    USE_MOCK ? Promise.resolve(mockActivity) : apiFetch("/activity"),

  // Systemd
  getServices: (): Promise<SystemdService[]> =>
    USE_MOCK ? Promise.resolve(mockServices) : apiFetch("/systemd/services"),
  getServiceLogs: (name: string): Promise<SystemdLog[]> =>
    USE_MOCK ? Promise.resolve([
      { timestamp: "2026-02-21T10:55:00Z", level: "info", message: `${name} started successfully`, unit: name },
      { timestamp: "2026-02-21T10:54:59Z", level: "info", message: "Listening on port 80", unit: name },
    ]) : apiFetch(`/systemd/services/${name}/logs`),
  controlService: (name: string, action: "start" | "stop" | "restart"): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/systemd/services/${name}/${action}`, { method: "POST" }),
  toggleServiceEnabled: (name: string, enabled: boolean): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/systemd/services/${name}/enable`, { method: "POST", body: JSON.stringify({ enabled }) }),
  createService: (data: Partial<SystemdService>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch("/systemd/services", { method: "POST", body: JSON.stringify(data) }),
  updateService: (name: string, data: Partial<SystemdService>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/systemd/services/${name}`, { method: "PUT", body: JSON.stringify(data) }),
  deleteService: (name: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/systemd/services/${name}`, { method: "DELETE" }),

  // Crontab
  getCrontabs: (): Promise<CrontabTask[]> =>
    USE_MOCK ? Promise.resolve(mockCrontabs) : apiFetch("/crontab/tasks"),
  createCrontab: (data: Partial<CrontabTask>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch("/crontab/tasks", { method: "POST", body: JSON.stringify(data) }),
  toggleCrontab: (id: string, enabled: boolean): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/crontab/tasks/${id}/toggle`, { method: "POST", body: JSON.stringify({ enabled }) }),
  updateCrontab: (id: string, data: Partial<CrontabTask>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/crontab/tasks/${id}`, { method: "PUT", body: JSON.stringify(data) }),
  deleteCrontab: (id: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/crontab/tasks/${id}`, { method: "DELETE" }),

  // Mise / Tool Manager
  getMiseDeps: (): Promise<MiseDependency[]> =>
    USE_MOCK ? Promise.resolve(mockMiseDeps) : apiFetch("/mise/dependencies"),
  getAvailableVersions: (name: string): Promise<string[]> =>
    USE_MOCK ? Promise.resolve(
      name === "node" ? ["22.4.0", "22.0.0", "21.7.0", "20.11.0", "20.0.0", "18.20.0", "18.0.0"] :
      name === "python" ? ["3.13.0", "3.12.1", "3.12.0", "3.11.0", "3.10.0"] :
      name === "rust" ? ["1.84.0", "1.83.0", "1.82.0", "1.81.0", "1.80.0"] : []
    ) : apiFetch(`/mise/dependencies/${name}/versions`),
  uninstallMiseDepVersion: (id: string, version: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/mise/dependencies/${id}/versions/${version}`, { method: "DELETE" }),
  createMiseDep: (data: { name: string; version: string }): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch("/mise/dependencies", { method: "POST", body: JSON.stringify(data) }),
  switchMiseDepVersion: (id: string, version: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/mise/dependencies/${id}/switch`, { method: "POST", body: JSON.stringify({ version }) }),
  deleteMiseDep: (id: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/mise/dependencies/${id}`, { method: "DELETE" }),
  getMiseTasks: (): Promise<MiseTask[]> =>
    USE_MOCK ? Promise.resolve(mockMiseTasks) : apiFetch("/mise/tasks"),
  createMiseTask: (data: Partial<MiseTask>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch("/mise/tasks", { method: "POST", body: JSON.stringify(data) }),
  updateMiseTask: (id: string, data: Partial<MiseTask>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/mise/tasks/${id}`, { method: "PUT", body: JSON.stringify(data) }),
  deleteMiseTask: (id: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/mise/tasks/${id}`, { method: "DELETE" }),
  runMiseTask: (name: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/mise/tasks/${name}/run`, { method: "POST" }),

  // Nginx
  getNginxProxies: (): Promise<NginxProxy[]> =>
    USE_MOCK ? Promise.resolve(mockNginxProxies) : apiFetch("/nginx/proxies"),
  createNginxProxy: (data: Partial<NginxProxy>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch("/nginx/proxies", { method: "POST", body: JSON.stringify(data) }),
  updateNginxProxy: (id: string, data: Partial<NginxProxy>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/nginx/proxies/${id}`, { method: "PUT", body: JSON.stringify(data) }),
  deleteNginxProxy: (id: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/nginx/proxies/${id}`, { method: "DELETE" }),
  testNginxProxy: (id: string): Promise<{ status: number; time: number }> =>
    USE_MOCK ? Promise.resolve({ status: 200, time: 42 }) : apiFetch(`/nginx/proxies/${id}/test`),

  // Cloudflare
  getTunnels: (): Promise<CloudflareTunnel[]> =>
    USE_MOCK ? Promise.resolve(mockTunnels) : apiFetch("/cloudflare/tunnels"),
  createTunnel: (data: Partial<CloudflareTunnel>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch("/cloudflare/tunnels", { method: "POST", body: JSON.stringify(data) }),
  updateTunnel: (id: string, data: Partial<CloudflareTunnel>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/cloudflare/tunnels/${id}`, { method: "PUT", body: JSON.stringify(data) }),
  deleteTunnel: (id: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/cloudflare/tunnels/${id}`, { method: "DELETE" }),

  // TTY
  getTTYSessions: (): Promise<TTYSession[]> =>
    USE_MOCK ? Promise.resolve(mockTTYSessions) : apiFetch("/tty/sessions"),
  createTTYSession: (data: Partial<TTYSession>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch("/tty/sessions", { method: "POST", body: JSON.stringify(data) }),
  startTTYSession: (id: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/tty/sessions/${id}/start`, { method: "POST" }),
  updateTTYSession: (id: string, data: Partial<TTYSession>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/tty/sessions/${id}`, { method: "PUT", body: JSON.stringify(data) }),
  deleteTTYSession: (id: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/tty/sessions/${id}`, { method: "DELETE" }),

  // Config
  getManagedDirs: (): Promise<ManagedDirectory[]> =>
    USE_MOCK ? Promise.resolve(mockManagedDirs) : apiFetch("/config/dirs"),
  addManagedDir: (path: string, label: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch("/config/dirs", { method: "POST", body: JSON.stringify({ path, label }) }),
  removeManagedDir: (id: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/config/dirs/${id}`, { method: "DELETE" }),
  getConfigStatus: (dirId?: string): Promise<ConfigStatus> =>
    USE_MOCK ? Promise.resolve(mockConfigStatus) : apiFetch(`/config/status${dirId ? `?dir=${dirId}` : ""}`),
  getConfigChanges: (dirId?: string): Promise<ConfigChange[]> =>
    USE_MOCK ? Promise.resolve(mockConfigChanges) : apiFetch(`/config/changes${dirId ? `?dir=${dirId}` : ""}`),
  getConfigCommits: (dirId?: string): Promise<ConfigCommit[]> =>
    USE_MOCK ? Promise.resolve(mockConfigCommits) : apiFetch(`/config/commits${dirId ? `?dir=${dirId}` : ""}`),
  commitConfig: (message: string, files: string[], dirId?: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch("/config/commit", { method: "POST", body: JSON.stringify({ message, files, dir_id: dirId }) }),
  rollbackConfig: (hash: string, dirId?: string): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch(`/config/rollback/${hash}`, { method: "POST", body: JSON.stringify({ dir_id: dirId }) }),

  // Settings
  getSettings: (): Promise<SettingsConfig> =>
    USE_MOCK ? Promise.resolve(mockSettings) : apiFetch("/settings"),
  updateSettings: (data: Partial<SettingsConfig>): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch("/settings", { method: "PUT", body: JSON.stringify(data) }),
  getToolStatuses: (): Promise<ToolStatus[]> =>
    USE_MOCK ? Promise.resolve(mockTools) : apiFetch("/settings/tools"),
  resetSystem: (): Promise<void> =>
    USE_MOCK ? Promise.resolve() : apiFetch("/settings/reset", { method: "POST" }),
};
