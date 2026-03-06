"use client";

import { useTransition } from "react";

interface MonitoringFormProps {
  libraryId: string;
  monitorEnabled: boolean;
  scanMode: string;
}

export function MonitoringForm({ libraryId, monitorEnabled, scanMode }: MonitoringFormProps) {
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
    <form action={handleSubmit} className="monitoring-form">
      <input type="hidden" name="id" value={libraryId} />
      <label className={`monitor-toggle ${isPending ? 'pending' : ''}`}>
        <input
          type="checkbox"
          name="monitor_enabled"
          value="true"
          defaultChecked={monitorEnabled}
          disabled={isPending}
        />
        Auto
      </label>
      <select
        name="scan_mode"
        defaultValue={scanMode}
        disabled={isPending}
      >
        <option value="manual">Manual only</option>
        <option value="hourly">Hourly</option>
        <option value="daily">Daily</option>
        <option value="weekly">Weekly</option>
      </select>
      <button type="submit" className="update-btn" disabled={isPending}>
        {isPending ? '...' : 'Update'}
      </button>
    </form>
  );
}
