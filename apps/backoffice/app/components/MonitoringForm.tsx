"use client";

import { useTransition } from "react";

interface MonitoringFormProps {
  libraryId: string;
  monitorEnabled: boolean;
  scanMode: string;
  watcherEnabled: boolean;
}

export function MonitoringForm({ libraryId, monitorEnabled, scanMode, watcherEnabled }: MonitoringFormProps) {
  const [isPending, startTransition] = useTransition();

  const handleSubmit = (formData: FormData) => {
    startTransition(async () => {
      try {
        const response = await fetch(`/api/libraries/${libraryId}/monitoring`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            monitor_enabled: formData.get("monitor_enabled") === "true",
            scan_mode: formData.get("scan_mode"),
            watcher_enabled: formData.get("watcher_enabled") === "true",
          }),
        });
        if (response.ok) {
          window.location.reload();
        }
      } catch (error) {
        console.error("Failed to update monitoring:", error);
      }
    });
  };

  return (
    <form action={handleSubmit} className="monitoring-form-compact">
      <input type="hidden" name="id" value={libraryId} />
      
      <div className="monitor-row">
        <label className={`monitor-checkbox ${isPending ? 'pending' : ''}`}>
          <input
            type="checkbox"
            name="monitor_enabled"
            value="true"
            defaultChecked={monitorEnabled}
            disabled={isPending}
          />
          <span>Auto</span>
        </label>
        
        <label className={`monitor-checkbox watcher ${isPending ? 'pending' : ''} ${watcherEnabled ? 'active' : ''}`}>
          <input
            type="checkbox"
            name="watcher_enabled"
            value="true"
            defaultChecked={watcherEnabled}
            disabled={isPending}
          />
          <span title="Real-time file watcher">⚡</span>
        </label>
        
        <select
          name="scan_mode"
          defaultValue={scanMode}
          disabled={isPending}
          className="scan-mode-select"
        >
          <option value="manual">Manual</option>
          <option value="hourly">Hourly</option>
          <option value="daily">Daily</option>
          <option value="weekly">Weekly</option>
        </select>
        
        <button type="submit" className="save-btn" disabled={isPending}>
          {isPending ? '...' : '✓'}
        </button>
      </div>
    </form>
  );
}
