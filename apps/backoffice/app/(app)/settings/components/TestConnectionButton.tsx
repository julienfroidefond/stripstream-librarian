"use client";

import { useState } from "react";
import { Button, Icon } from "@/app/components/ui";
import { browserApi, type TestConnectionTarget } from "@/lib/api/browser";
import { ApiError } from "@/lib/api/errors";
import { useTranslation } from "@/lib/i18n/context";

interface TestConnectionButtonProps {
  target: TestConnectionTarget;
  disabled?: boolean;
}

export function TestConnectionButton({ target, disabled }: TestConnectionButtonProps) {
  const { t } = useTranslation();
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);

  async function handleTestConnection() {
    setIsTesting(true);
    setTestResult(null);
    try {
      const data = await browserApi.settings.testConnection(target);
      if (data.error) {
        setTestResult({ success: false, message: data.error });
      } else {
        setTestResult(data);
      }
    } catch (error) {
      setTestResult({
        success: false,
        message: error instanceof ApiError ? error.detail ?? "Failed to connect" : "Failed to connect",
      });
    } finally {
      setIsTesting(false);
    }
  }

  return (
    <div className="flex flex-wrap items-center gap-3">
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
    </div>
  );
}
