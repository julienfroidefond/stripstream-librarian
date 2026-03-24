"use client";

import Image from "next/image";
import { useSearchParams } from "next/navigation";
import { useState, Suspense } from "react";

function LoginForm() {
  const searchParams = useSearchParams();
  const from = searchParams.get("from") || "/";

  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    setLoading(true);

    try {
      const res = await fetch("/api/auth/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username, password }),
      });

      if (res.ok) {
        window.location.href = from;
      } else {
        const data = await res.json().catch(() => ({}));
        setError(data.error || "Identifiants invalides");
      }
    } catch {
      setError("Erreur réseau");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="relative min-h-screen flex flex-col items-center justify-center px-4 py-16 overflow-hidden">

      {/* Background logo */}
      <Image
        src="/logo.png"
        alt=""
        fill
        className="object-cover opacity-20"
        priority
        aria-hidden
      />

      {/* Hero */}
      <div className="relative flex flex-col items-center mb-10">
        <h1 className="text-4xl font-bold tracking-tight text-foreground">
          StripStream{" "}
          <span className="text-primary font-light">: Librarian</span>
        </h1>
        <p className="text-sm text-muted-foreground mt-1.5 tracking-wide uppercase font-medium">
          Administration
        </p>
      </div>

      {/* Form card */}
      <div
        className="relative w-full max-w-sm rounded-2xl border border-white/20 backdrop-blur-sm p-8"
        style={{ boxShadow: "0 24px 48px -12px rgb(0 0 0 / 0.18)" }}
      >
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="username" className="block text-sm font-medium text-foreground mb-1.5">
              Identifiant
            </label>
            <input
              id="username"
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoComplete="username"
              autoFocus
              required
              disabled={loading}
              placeholder="admin"
              className="
                flex w-full h-11 px-4
                rounded-xl border border-input bg-background/60
                text-sm text-foreground
                placeholder:text-muted-foreground/40
                focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:border-ring
                disabled:opacity-50
                transition-all duration-200
              "
            />
          </div>

          <div>
            <label htmlFor="password" className="block text-sm font-medium text-foreground mb-1.5">
              Mot de passe
            </label>
            <input
              id="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="current-password"
              required
              disabled={loading}
              placeholder="••••••••"
              className="
                flex w-full h-11 px-4
                rounded-xl border border-input bg-background/60
                text-sm text-foreground
                placeholder:text-muted-foreground/40
                focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:border-ring
                disabled:opacity-50
                transition-all duration-200
              "
            />
          </div>

          {error && (
            <div className="flex items-center gap-2 px-3 py-2.5 rounded-xl bg-destructive/10 border border-destructive/20 text-sm text-destructive">
              <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="shrink-0">
                <circle cx="12" cy="12" r="10" /><line x1="12" y1="8" x2="12" y2="12" /><line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
              {error}
            </div>
          )}

          <button
            type="submit"
            disabled={loading}
            className="
              w-full h-11 mt-2
              inline-flex items-center justify-center gap-2
              rounded-xl font-medium text-sm
              bg-primary text-primary-foreground
              hover:bg-primary/90
              transition-all duration-200 ease-out
              focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2
              disabled:pointer-events-none disabled:opacity-50
              active:scale-[0.98]
            "
            style={{ boxShadow: "0 4px 16px -4px hsl(198 78% 37% / 0.5)" }}
          >
            {loading ? (
              <>
                <svg className="animate-spin" xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M21 12a9 9 0 1 1-6.219-8.56" />
                </svg>
                Connexion…
              </>
            ) : "Se connecter"}
          </button>
        </form>
      </div>
    </div>
  );
}

export default function LoginPage() {
  return (
    <Suspense>
      <LoginForm />
    </Suspense>
  );
}
