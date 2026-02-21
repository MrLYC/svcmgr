import { Toaster } from "@/components/ui/toaster";
import { Toaster as Sonner } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { I18nProvider } from "@/i18n";
import { AppLayout } from "@/components/AppLayout";
import Dashboard from "@/pages/Dashboard";
import SystemdServices from "@/pages/SystemdServices";
import CrontabTasks from "@/pages/CrontabTasks";
import MiseTasks from "@/pages/MiseTasks";
import NginxProxies from "@/pages/NginxProxies";
import CloudflareTunnels from "@/pages/CloudflareTunnels";
import TTYSessions from "@/pages/TTYSessions";
import ConfigManagement from "@/pages/ConfigManagement";
import SettingsPage from "@/pages/SettingsPage";
import NotFound from "@/pages/NotFound";

const queryClient = new QueryClient();

const App = () => (
  <QueryClientProvider client={queryClient}>
    <I18nProvider>
      <TooltipProvider>
        <Toaster />
        <Sonner />
        <BrowserRouter>
          <Routes>
            <Route element={<AppLayout />}>
              <Route path="/" element={<Dashboard />} />
              <Route path="/services/systemd" element={<SystemdServices />} />
              <Route path="/services/crontab" element={<CrontabTasks />} />
              <Route path="/services/mise" element={<MiseTasks />} />
              <Route path="/proxy/nginx" element={<NginxProxies />} />
              <Route path="/proxy/cloudflare" element={<CloudflareTunnels />} />
              <Route path="/tty" element={<TTYSessions />} />
              <Route path="/config" element={<ConfigManagement />} />
              <Route path="/settings" element={<SettingsPage />} />
            </Route>
            <Route path="*" element={<NotFound />} />
          </Routes>
        </BrowserRouter>
      </TooltipProvider>
    </I18nProvider>
  </QueryClientProvider>
);

export default App;
