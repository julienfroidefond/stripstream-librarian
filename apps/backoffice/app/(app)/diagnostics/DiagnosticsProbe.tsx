"use client";

import { useState } from "react";
import { Badge, Button, Icon, toast } from "@/app/components/ui";
import type { DiagnosticsIntegration, DiagnosticsProbe } from "@/lib/api";
import { formatDateTime, formatNumber } from "@/lib/format";
import { useTranslation } from "@/lib/i18n/context";

/**
 * Readable name of a probed integration.
 *
 * `id` is either a fixed key (`qbittorrent`, `alldebrid`, `prowlarr`, `telegram`)
 * or `ddl_<provider>` for a Direct Download provider.
 */
function integrationLabel(id: string): string {
  if (id.startsWith("ddl_")) {
    const provider = id.slice("ddl_".length);
    return `DDL · ${provider.charAt(0).toUpperCase()}${provider.slice(1)}`;
  }

  switch (id) {
    case "qbittorrent":
      return "qBittorrent";
    case "alldebrid":
      return "AllDebrid";
    case "prowlarr":
      return "Prowlarr";
    case "telegram":
      return "Telegram";
    default:
      return id;
  }
}

function IntegrationBadge({ integration }: { integration: DiagnosticsIntegration }) {
  const { t } = useTranslation();

  if (!integration.configured) {
    return <Badge variant="muted">{t("diagnostics.probeNotConfigured")}</Badge>;
  }

  return (
    <Badge variant={integration.reachable ? "success" : "error"}>
      {integration.reachable ? t("diagnostics.probeOk") : t("diagnostics.probeKo")}
    </Badge>
  );
}

/**
 * On-demand probe: external integrations and missing book files.
 *
 * Nothing runs on page load — the operator triggers it explicitly, because it
 * contacts third-party services and stats every book file.
 */
export function DiagnosticsProbePanel() {
  const { t, locale } = useTranslation();
  const [probe, setProbe] = useState<DiagnosticsProbe | null>(null);
  const [isRunning, setIsRunning] = useState(false);

  async function runProbe() {
    setIsRunning(true);
    try {
      const response = await fetch("/api/diagnostics/probe");
      if (!response.ok) {
        throw new Error(`probe failed (${response.status})`);
      }
      setProbe((await response.json()) as DiagnosticsProbe);
    } catch {
      toast(t("diagnostics.probeError"), "error");
    } finally {
      setIsRunning(false);
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-3">
        <Button variant="outline" disabled={isRunning} onClick={runProbe}>
          <Icon name={isRunning ? "spinner" : "search"} size="sm" className={`mr-2 ${isRunning ? "animate-spin" : ""}`} />
          {isRunning ? t("diagnostics.probing") : t("diagnostics.runProbe")}
        </Button>
        {probe && (
          <span className="text-xs text-muted-foreground">
            {t("diagnostics.probeSampledAt")} {formatDateTime(probe.sampled_at, locale)}
          </span>
        )}
      </div>

      {!probe && <p className="text-sm text-muted-foreground">{t("diagnostics.probeNotRun")}</p>}

      {probe && (
        <>
          <div>
            <h3 className="mb-2 text-sm font-semibold text-foreground">
              {t("diagnostics.probeIntegrations")}
            </h3>
            {probe.integrations.length === 0 ? (
              <p className="text-sm text-muted-foreground">{t("diagnostics.probeNotConfigured")}</p>
            ) : (
              <ul className="space-y-2">
                {probe.integrations.map((integration) => (
                  <li
                    key={integration.id}
                    className="flex flex-wrap items-center justify-between gap-2 rounded-md bg-muted/40 px-3 py-2"
                  >
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="text-sm font-medium text-foreground">
                        {integrationLabel(integration.id)}
                      </span>
                      <IntegrationBadge integration={integration} />
                      {integration.detail && (
                        <span className="text-xs text-muted-foreground">{integration.detail}</span>
                      )}
                    </div>
                    <span className="font-mono text-xs text-muted-foreground">
                      {integration.latency_ms} ms
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div>
            <h3 className="mb-2 text-sm font-semibold text-foreground">
              {t("diagnostics.probeFiles")}
            </h3>
            <div className="flex flex-wrap gap-6 text-sm">
              <span className="text-muted-foreground">
                {t("diagnostics.probeFilesChecked")}{" "}
                <span className="font-medium text-foreground">
                  {formatNumber(probe.missing_files.checked, locale)}
                </span>
              </span>
              <span className="text-muted-foreground">
                {t("diagnostics.probeFilesMissing")}{" "}
                <span
                  className={`font-medium ${probe.missing_files.missing > 0 ? "text-destructive" : "text-foreground"}`}
                >
                  {formatNumber(probe.missing_files.missing, locale)}
                </span>
              </span>
            </div>

            {probe.missing_files.missing === 0 ? (
              <p className="mt-2 text-sm text-success">{t("diagnostics.probeNoMissing")}</p>
            ) : (
              <div className="mt-2">
                <p className="mb-1 text-xs uppercase tracking-wide text-muted-foreground">
                  {t("diagnostics.probeSamples")}
                </p>
                <ul className="space-y-1">
                  {probe.missing_files.samples.map((sample) => (
                    <li key={sample} className="break-all font-mono text-xs text-destructive">
                      {sample}
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
