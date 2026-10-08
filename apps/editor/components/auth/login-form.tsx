"use client";

import { useTranslations } from "next-intl";
import { useState, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import { GitHubMark } from "@/components/ui/github-mark";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { supabaseBrowser } from "@/lib/supabase/client";

type State =
  | { kind: "idle" }
  | { kind: "sending" }
  | { kind: "sent"; email: string }
  | { kind: "error"; message: string };

export function LoginForm({ next }: { next: string }) {
  const t = useTranslations("auth");
  const [email, setEmail] = useState("");
  const [state, setState] = useState<State>({ kind: "idle" });

  const redirectTo = () =>
    `${window.location.origin}/auth/callback?next=${encodeURIComponent(next)}`;

  async function signInWithGitHub() {
    const { error } = await supabaseBrowser().auth.signInWithOAuth({
      provider: "github",
      options: { redirectTo: redirectTo() },
    });
    if (error) setState({ kind: "error", message: error.message });
  }

  async function sendMagicLink(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setState({ kind: "sending" });
    const address = email.trim();
    const { error } = await supabaseBrowser().auth.signInWithOtp({
      email: address,
      options: { emailRedirectTo: redirectTo() },
    });
    setState(
      error
        ? { kind: "error", message: error.message }
        : { kind: "sent", email: address },
    );
  }

  return (
    <div className="flex flex-col gap-6">
      <Button variant="outline" onClick={signInWithGitHub} className="w-full">
        <GitHubMark />
        {t("github")}
      </Button>
      <div className="flex items-center gap-3 text-xs uppercase text-muted-foreground">
        <span className="h-px flex-1 bg-border" />
        {t("or")}
        <span className="h-px flex-1 bg-border" />
      </div>
      <form onSubmit={sendMagicLink} className="flex flex-col gap-3">
        <Label htmlFor="email">{t("email")}</Label>
        <Input
          id="email"
          type="email"
          name="email"
          autoComplete="email"
          required
          placeholder={t("emailPlaceholder")}
          value={email}
          onChange={(event) => setEmail(event.target.value)}
        />
        <Button type="submit" disabled={state.kind === "sending"}>
          {state.kind === "sending" ? t("sending") : t("magicLink")}
        </Button>
      </form>
      <p role="status" aria-live="polite" className="min-h-5 text-sm">
        {state.kind === "sent" ? t("sent", { email: state.email }) : null}
        {state.kind === "error" ? (
          <span className="text-destructive">
            {t("failed", { message: state.message })}
          </span>
        ) : null}
      </p>
    </div>
  );
}
