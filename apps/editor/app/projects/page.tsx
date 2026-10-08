import { LogOut } from "lucide-react";
import { getTranslations } from "next-intl/server";

import { LanguageSwitcher } from "@/components/language-switcher";
import { ProjectsView } from "@/components/projects/projects-view";
import { Button } from "@/components/ui/button";
import { supabaseServer } from "@/lib/supabase/server";

export default async function ProjectsPage() {
  const t = await getTranslations();
  const supabase = await supabaseServer();
  const { data } = await supabase.auth.getClaims();
  const email =
    typeof data?.claims?.email === "string" ? data.claims.email : null;
  return (
    <div className="min-h-dvh">
      <header className="border-b border-border">
        <div className="mx-auto flex max-w-5xl flex-wrap items-center justify-between gap-3 px-4 py-3">
          <span className="font-semibold">{t("common.appName")}</span>
          <div className="flex flex-wrap items-center gap-2">
            {email ? (
              <span className="hidden text-sm text-muted-foreground sm:inline">
                {email}
              </span>
            ) : null}
            <LanguageSwitcher />
            <form action="/auth/signout" method="post">
              <Button type="submit" variant="ghost" size="sm">
                <LogOut aria-hidden="true" />
                {t("auth.signOut")}
              </Button>
            </form>
          </div>
        </div>
      </header>
      <main className="mx-auto max-w-5xl px-4 py-8">
        <ProjectsView />
      </main>
    </div>
  );
}
