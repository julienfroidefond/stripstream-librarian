"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";

export default function AnilistCallbackPage() {
  const router = useRouter();
  const [status, setStatus] = useState<"loading" | "success" | "error">("loading");
  const [message, setMessage] = useState("");

  useEffect(() => {
    async function handleCallback() {
      const hash = window.location.hash.slice(1); // remove leading #
      const params = new URLSearchParams(hash);
      const accessToken = params.get("access_token");

      if (!accessToken) {
        setStatus("error");
        setMessage("Aucun token trouvé dans l'URL de callback.");
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
        if (!saveResp.ok) throw new Error("Impossible de sauvegarder le token");

        // Auto-fetch user info to populate user_id
        const statusResp = await fetch("/api/anilist/status");
        if (statusResp.ok) {
          const data = await statusResp.json();
          if (data.user_id) {
            await save({ user_id: data.user_id });
          }
          setMessage(`Connecté en tant que ${data.username}`);
        } else {
          setMessage("Token sauvegardé.");
        }

        setStatus("success");
        setTimeout(() => router.push("/settings?tab=anilist"), 2000);
      } catch (e) {
        setStatus("error");
        setMessage(e instanceof Error ? e.message : "Erreur inconnue");
      }
    }

    handleCallback();
  }, [router]);

  return (
    <div className="min-h-screen flex items-center justify-center bg-background">
      <div className="text-center space-y-4 p-8">
        {status === "loading" && (
          <>
            <div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin mx-auto" />
            <p className="text-muted-foreground">Connexion AniList en cours…</p>
          </>
        )}
        {status === "success" && (
          <>
            <div className="w-12 h-12 rounded-full bg-success/15 flex items-center justify-center mx-auto">
              <svg className="w-6 h-6 text-success" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <p className="text-success font-medium">{message}</p>
            <p className="text-sm text-muted-foreground">Redirection vers les paramètres…</p>
          </>
        )}
        {status === "error" && (
          <>
            <div className="w-12 h-12 rounded-full bg-destructive/15 flex items-center justify-center mx-auto">
              <svg className="w-6 h-6 text-destructive" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </div>
            <p className="text-destructive font-medium">{message}</p>
            <a href="/settings" className="text-sm text-primary hover:underline">
              Retour aux paramètres
            </a>
          </>
        )}
      </div>
    </div>
  );
}
