import {
  LayoutDashboard, Server, Clock, Package, Globe, CloudCog,
  Terminal, FolderGit2, Settings, ChevronDown,
} from "lucide-react";
import { useLocation } from "react-router-dom";
import { NavLink } from "@/components/NavLink";
import { useI18n } from "@/i18n";
import {
  Sidebar, SidebarContent, SidebarGroup, SidebarGroupContent,
  SidebarMenu, SidebarMenuButton, SidebarMenuItem,
  SidebarMenuSub, SidebarMenuSubItem, SidebarMenuSubButton,
} from "@/components/ui/sidebar";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";

export function AppSidebar() {
  const { t } = useI18n();

  const navItems = [
    { title: t("nav.dashboard"), url: "/", icon: LayoutDashboard },
  ];

  const serviceItems = [
    { title: t("nav.systemd"), url: "/services/systemd", icon: Server },
    { title: t("nav.crontab"), url: "/services/crontab", icon: Clock },
    { title: t("nav.mise"), url: "/services/mise", icon: Package },
  ];

  const proxyItems = [
    { title: t("nav.nginx"), url: "/proxy/nginx", icon: Globe },
    { title: t("nav.cloudflare"), url: "/proxy/cloudflare", icon: CloudCog },
  ];

  const otherItems = [
    { title: t("nav.tty"), url: "/tty", icon: Terminal },
    { title: t("nav.config"), url: "/config", icon: FolderGit2 },
    { title: t("nav.settings"), url: "/settings", icon: Settings },
  ];

  return (
    <Sidebar className="border-r-0 bg-sidebar">
      <div className="flex h-14 items-center gap-2 border-b border-sidebar-border px-4">
        <Server className="h-5 w-5 text-sidebar-primary" />
        <span className="text-base font-bold text-sidebar-accent-foreground tracking-tight">svcmgr</span>
      </div>
      <SidebarContent className="px-2 py-3">
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {navItems.map(item => (
                <SidebarMenuItem key={item.url}>
                  <SidebarMenuButton asChild>
                    <NavLink to={item.url} end className="flex items-center gap-2.5 rounded-md px-3 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground transition-colors" activeClassName="bg-sidebar-accent text-sidebar-primary font-medium">
                      <item.icon className="h-4 w-4" /><span>{item.title}</span>
                    </NavLink>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
              <NavGroup label={t("nav.services")} items={serviceItems} defaultOpen />
              <NavGroup label={t("nav.proxy")} items={proxyItems} />
              {otherItems.map(item => (
                <SidebarMenuItem key={item.url}>
                  <SidebarMenuButton asChild>
                    <NavLink to={item.url} className="flex items-center gap-2.5 rounded-md px-3 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground transition-colors" activeClassName="bg-sidebar-accent text-sidebar-primary font-medium">
                      <item.icon className="h-4 w-4" /><span>{item.title}</span>
                    </NavLink>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  );
}

function NavGroup({ label, items, defaultOpen }: { label: string; items: { title: string; url: string; icon: any }[]; defaultOpen?: boolean }) {
  const location = useLocation();
  const isActive = items.some(i => location.pathname.startsWith(i.url));

  return (
    <Collapsible defaultOpen={defaultOpen || isActive}>
      <SidebarMenuItem>
        <CollapsibleTrigger asChild>
          <SidebarMenuButton className="w-full justify-between text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground">
            <span className="text-xs font-semibold uppercase tracking-wider opacity-60">{label}</span>
            <ChevronDown className="h-3.5 w-3.5 transition-transform duration-200 [[data-state=open]>&]:rotate-180" />
          </SidebarMenuButton>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <SidebarMenuSub>
            {items.map(item => (
              <SidebarMenuSubItem key={item.url}>
                <SidebarMenuSubButton asChild>
                  <NavLink to={item.url} className="flex items-center gap-2 rounded-md px-3 py-1.5 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground transition-colors" activeClassName="bg-sidebar-accent text-sidebar-primary font-medium">
                    <item.icon className="h-4 w-4" /><span>{item.title}</span>
                  </NavLink>
                </SidebarMenuSubButton>
              </SidebarMenuSubItem>
            ))}
          </SidebarMenuSub>
        </CollapsibleContent>
      </SidebarMenuItem>
    </Collapsible>
  );
}
