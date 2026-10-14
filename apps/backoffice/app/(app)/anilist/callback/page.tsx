"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";

import { useTranslation } from "@/lib/i18n/context";
import type { TranslationKey } from "@/lib/i18n/fr";

type CallbackMessage = { key: TranslationKey; params?: Record<string, string | number> };

/**
 * The callback outcome is stored as an i18n key (not a rendered string) so the
 * effect can stay independent of `t`, which is recreated on every render.
 */
type CallbackState =
  | { status: "loading" }
  | { status: "success"; message: CallbackMessage }
  | { status: "error"; message: CallbackMessage };

export default function AnilistCallbackPage() {
  const router = useRouter();
  const { t } = useTranslation();
  const [state, setState] = useState<CallbackState>({ status: "loading" });

  useEffect(() => {
    async function handleCallback() {
      const hash = window.location.hash.slice(1); // remove leading #
      const params = new URLSearchParams(hash);
      const accessToken = params.get("access_token");

      if (!accessToken) {
        setState({ status: "error", message: { key: "settings.anilistCallbackNoToken" } });
        return;
      }

      try {
        // Read existing settings to preserve client_id
        const existingResp = await fetch("/api/settings/anilist").catch(() => null);
        const existing = existingResp?.ok ? await existingResp.json().catch(() => ({})) : {};

        const save = (extra: Record<string, unknown>) =>
          fetch("/api/settings/anilist", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ value: { ...existing, access_token: accessToken, ...extra } }),
          });

        const saveResp = await save({});
        if (!saveResp.ok) {
          setState({ status: "error", message: { key: "settings.anilistCallbackSaveError" } });
          return;
        }

        // Auto-fetch user info to populate user_id
        const statusResp = await fetch("/api/anilist/status");
        if (statusResp.ok) {
          const data = await statusResp.json();
          if (data.user_id) {
            await save({ user_id: data.user_id });
          }
          setState({
            status: "success",
            message: {
              key: "settings.anilistCallbackConnected",
              params: { username: String(data.username) },
            },
          });
        } else {
          setState({ status: "success", message: { key: "settings.anilistCallbackTokenSaved" } });
        }

        setTimeout(() => router.push("/settings?tab=anilist"), 2000);
      } catch {
        setState({ status: "error", message: { key: "common.unknownError" } });
      }
    }

    handleCallback();
  }, [router]);

  return (
    <div className="min-h-screen flex items-center justify-center bg-background">
      <div className="text-center space-y-4 p-8">
        {state.status === "loading" && (
          <>
            <div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin mx-auto" />
            <p className="text-muted-foreground">{t("settings.anilistCallbackConnecting")}</p>
          </>
        )}
        {state.status === "success" && (
          <>
            <div className="w-12 h-12 rounded-full bg-success/15 flex items-center justify-center mx-auto">
              <svg className="w-6 h-6 text-success" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <p className="text-success font-medium">{t(state.message.key, state.message.params)}</p>
            <p className="text-sm text-muted-foreground">{t("settings.anilistCallbackRedirecting")}</p>
          </>
        )}
        {state.status === "error" && (
          <>
            <div className="w-12 h-12 rounded-full bg-destructive/15 flex items-center justify-center mx-auto">
              <svg className="w-6 h-6 text-destructive" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </div>
            <p className="text-destructive font-medium">{t(state.message.key, state.message.params)}</p>
            <a href="/settings" className="text-sm text-primary hover:underline">
              {t("settings.anilistCallbackBackToSettings")}
            </a>
          </>
        )}
      </div>
    </div>
  );
}
