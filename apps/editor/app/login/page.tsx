import { getTranslations } from "next-intl/server";

import { LoginForm } from "@/components/auth/login-form";
import { LanguageSwitcher } from "@/components/language-switcher";
import { safeNext } from "@/lib/redirect";

export default async function LoginPage({
  searchParams,
}: {
  searchParams: Promise<{ next?: string; error?: string }>;
}) {
  const { next, error } = await searchParams;
  const t = await getTranslations("auth");
  return (
    <main className="flex min-h-dvh flex-col items-center justify-center gap-8 px-4 py-12">
      <div className="w-full max-w-sm rounded-xl border border-border p-6 shadow-sm sm:p-8">
        <h1 className="text-2xl font-semibold tracking-tight">{t("title")}</h1>
        <p className="mt-2 text-sm text-muted-foreground">{t("subtitle")}</p>
        {error ? (
          <p role="alert" className="mt-4 text-sm text-destructive">
            {t("callbackFailed")}
          </p>
        ) : null}
        <div className="mt-6">
          <LoginForm next={safeNext(next)} />
        </div>
      </div>
      <LanguageSwitcher />
    </main>
  );
}
