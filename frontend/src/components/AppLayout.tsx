import { Outlet } from "react-router-dom";
import { SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar";
import { AppSidebar } from "@/components/AppSidebar";
import { useI18n } from "@/i18n";
import { Button } from "@/components/ui/button";
import { Languages } from "lucide-react";

export function AppLayout() {
  const { locale, setLocale } = useI18n();

  return (
    <SidebarProvider>
      <div className="flex min-h-screen w-full">
        <AppSidebar />
        <div className="flex flex-1 flex-col">
          <header className="flex h-14 items-center justify-between border-b px-4">
            <SidebarTrigger className="text-muted-foreground hover:text-foreground" />
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setLocale(locale === "zh" ? "en" : "zh")}
              className="gap-1.5 text-muted-foreground"
            >
              <Languages className="h-4 w-4" />
              {locale === "zh" ? "EN" : "中文"}
            </Button>
          </header>
          <main className="flex-1 overflow-auto p-6">
            <Outlet />
          </main>
        </div>
      </div>
    </SidebarProvider>
  );
}
