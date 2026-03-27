"use client";

import { useState, useMemo } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, FormField, FormInput, FormSelect, Icon } from "@/app/components/ui";
import { StatusMappingDto } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";

export function StatusMappingsCard({ initialStatusMappings, initialSeriesStatuses, initialProviderStatuses }: { initialStatusMappings: Record<string, unknown>[]; initialSeriesStatuses: string[]; initialProviderStatuses: string[] }) {
  const { t } = useTranslation();
  const [mappings, setMappings] = useState<StatusMappingDto[]>(initialStatusMappings as unknown as StatusMappingDto[]);
  const [targetStatuses, setTargetStatuses] = useState<string[]>(initialSeriesStatuses);
  const [providerStatuses] = useState<string[]>(initialProviderStatuses);
  const [newTargetName, setNewTargetName] = useState("");

  // Group mappings by target status (only those with a non-null mapped_status)
  const grouped = useMemo(() => {
    const map = new Map<string, StatusMappingDto[]>();
    for (const m of mappings) {
      if (m.mapped_status) {
        const list = map.get(m.mapped_status) || [];
        list.push(m);
        map.set(m.mapped_status, list);
      }
    }
    return map;
  }, [mappings]);

  // Unmapped = mappings with null mapped_status + provider statuses not in status_mappings at all
  const knownProviderStatuses = useMemo(
    () => new Set(mappings.map((m) => m.provider_status)),
    [mappings],
  );
  const unmappedMappings = useMemo(
    () => mappings.filter((m) => !m.mapped_status),
    [mappings],
  );
  const newProviderStatuses = useMemo(
    () => providerStatuses.filter((ps) => !knownProviderStatuses.has(ps)),
    [providerStatuses, knownProviderStatuses],
  );

  // All possible targets = existing statuses from DB + custom ones added locally
  const [customTargets, setCustomTargets] = useState<string[]>([]);
  const allTargets = useMemo(() => {
    const set = new Set([...targetStatuses, ...customTargets]);
    return [...set].sort();
  }, [targetStatuses, customTargets]);

  async function handleAssign(providerStatus: string, targetStatus: string) {
    if (!providerStatus || !targetStatus) return;
    try {
      const res = await fetch("/api/settings/status-mappings", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ provider_status: providerStatus, mapped_status: targetStatus }),
      });
      if (res.ok) {
        const created: StatusMappingDto = await res.json();
        setMappings((prev) => [...prev.filter((m) => m.provider_status !== created.provider_status), created]);
      }
    } catch {
      // ignore
    }
  }

  async function handleUnmap(id: string) {
    try {
      const res = await fetch(`/api/settings/status-mappings/${id}`, { method: "DELETE" });
      if (res.ok) {
        const updated: StatusMappingDto = await res.json();
        setMappings((prev) => prev.map((m) => (m.id === id ? updated : m)));
      }
    } catch {
      // ignore
    }
  }

  function handleCreateTarget() {
    const name = newTargetName.trim().toLowerCase();
    if (!name || allTargets.includes(name)) return;
    setCustomTargets((prev) => [...prev, name]);
    setNewTargetName("");
  }

  function statusLabel(status: string) {
    const key = `seriesStatus.${status}` as Parameters<typeof t>[0];
    const translated = t(key);
    return translated !== key ? translated : status;
  }

  return (
    <Card className="mb-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Icon name="settings" size="md" />
          {t("settings.statusMappings")}
        </CardTitle>
        <CardDescription>{t("settings.statusMappingsDesc")}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-4">
          {/* Create new target status */}
          <div className="flex gap-2 items-center">
            <FormInput
              placeholder={t("settings.newTargetPlaceholder")}
              value={newTargetName}
              onChange={(e) => setNewTargetName(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") handleCreateTarget(); }}
              className="max-w-[250px]"
            />
            <Button
              onClick={handleCreateTarget}
              disabled={!newTargetName.trim() || allTargets.includes(newTargetName.trim().toLowerCase())}
            >
              <Icon name="plus" size="sm" />
              {t("settings.createTargetStatus")}
            </Button>
          </div>

          {/* Grouped by target status */}
          {allTargets.map((target) => {
            const items = grouped.get(target) || [];
            return (
              <div key={target} className="border border-border/50 rounded-lg p-3">
                <div className="flex items-center gap-2 mb-2">
                  <span className="text-sm font-medium text-foreground">
                    {statusLabel(target)}
                  </span>
                  <span className="text-xs text-muted-foreground font-mono">({target})</span>
                </div>
                <div className="flex flex-wrap gap-2">
                  {items.map((m) => (
                    <span
                      key={m.id}
                      className="inline-flex items-center gap-1 px-2 py-1 rounded-md bg-muted/50 text-sm font-mono"
                    >
                      {m.provider_status}
                      <button
                        type="button"
                        onClick={() => handleUnmap(m.id)}
                        className="ml-1 text-muted-foreground hover:text-destructive transition-colors"
                        title={t("common.delete")}
                      >
                        <Icon name="x" size="sm" />
                      </button>
                    </span>
                  ))}
                  {items.length === 0 && (
                    <span className="text-xs text-muted-foreground italic">{t("settings.noMappings")}</span>
                  )}
                </div>
              </div>
            );
          })}

          {/* Unmapped provider statuses (null mapped_status + brand new from providers) */}
          {(unmappedMappings.length > 0 || newProviderStatuses.length > 0) && (
            <div className="border-t border-border/50 pt-4">
              <h4 className="text-sm font-medium text-foreground mb-3">{t("settings.unmappedSection")}</h4>
              <div className="space-y-2">
                {unmappedMappings.map((m) => (
                  <div key={m.id} className="flex items-center gap-2">
                    <span className="text-sm font-mono bg-muted/50 px-2 py-1 rounded-md min-w-[120px]">{m.provider_status}</span>
                    <Icon name="chevronRight" size="sm" />
                    <FormSelect
                      className="w-auto"
                      value=""
                      onChange={(e) => { if (e.target.value) handleAssign(m.provider_status, e.target.value); }}
                    >
                      <option value="">{t("settings.selectTargetStatus")}</option>
                      {allTargets.map((s) => (
                        <option key={s} value={s}>{statusLabel(s)}</option>
                      ))}
                    </FormSelect>
                  </div>
                ))}
                {newProviderStatuses.map((ps) => (
                  <div key={ps} className="flex items-center gap-2">
                    <span className="text-sm font-mono bg-muted/50 px-2 py-1 rounded-md min-w-[120px]">{ps}</span>
                    <Icon name="chevronRight" size="sm" />
                    <FormSelect
                      className="w-auto"
                      value=""
                      onChange={(e) => { if (e.target.value) handleAssign(ps, e.target.value); }}
                    >
                      <option value="">{t("settings.selectTargetStatus")}</option>
                      {allTargets.map((s) => (
                        <option key={s} value={s}>{statusLabel(s)}</option>
                      ))}
                    </FormSelect>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
