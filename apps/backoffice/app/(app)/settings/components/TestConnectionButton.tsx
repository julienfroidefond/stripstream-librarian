"use client";

import { useState } from "react";
import { Button, Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";

interface TestConnectionButtonProps {
  endpoint: string;
  disabled?: boolean;
}

export function TestConnectionButton({ endpoint, disabled }: TestConnectionButtonProps) {
  const { t } = useTranslation();
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);

  async function handleTestConnection() {
    setIsTesting(true);
    setTestResult(null);
    try {
      const resp = await fetch(endpoint);
      const data = await resp.json();
      if (data.error) {
        setTestResult({ success: false, message: data.error });
      } else {
        setTestResult(data);
      }
    } catch {
      setTestResult({ success: false, message: "Failed to connect" });
    } finally {
      setIsTesting(false);
    }
  }

  return (
    <>
      <Button onClick={handleTestConnection} disabled={isTesting || disabled}>
        {isTesting ? (
          <>
            <Icon name="spinner" size="sm" className="animate-spin -ml-1 mr-2" />
            {t("settings.testing")}
          </>
        ) : (
          <>
            <Icon name="refresh" size="sm" className="mr-2" />
            {t("settings.testConnection")}
          </>
        )}
      </Button>
      {testResult && (
        <span className={`text-sm font-medium ${testResult.success ? "text-success" : "text-destructive"}`}>
          {testResult.message}
        </span>
      )}
    </>
  );
}
