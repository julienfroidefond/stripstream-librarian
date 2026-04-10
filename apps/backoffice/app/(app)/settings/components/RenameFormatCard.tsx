"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, FormField, FormInput } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";

const AVAILABLE_VARS = [
  { name: "series_name", example: "Dragon Ball" },
  { name: "volume", example: "1" },
  { name: "volume_padded", example: "01" },
  { name: "title", example: "Son Goku et ses amis" },
  { name: "authors", example: "Akira Toriyama" },
  { name: "publish_date", example: "2003-03-04" },
  { name: "isbn", example: "978-2723434546" },
];

function applyExampleTemplate(template: string): string {
  let result = template;
  for (const v of AVAILABLE_VARS) {
    result = result.replaceAll(`{${v.name}}`, v.example);
  }
  // Clean up dangling separators for missing vars
  result = result.replace(/\s*-\s*\{[^}]+\}/g, "");
  result = result.replace(/\{[^}]+\}\s*-\s*/g, "");
  result = result.replace(/\{[^}]+\}/g, "");
  return result.trim() + ".cbz";
}

export function RenameFormatCard({
  handleUpdateSetting,
  initialRenameFormat,
  initialRenameFormatHs,
}: {
  handleUpdateSetting: (key: string, value: unknown) => Promise<void>;
  initialRenameFormat: string | null;
  initialRenameFormatHs: string | null;
}) {
  const { t } = useTranslation();
  const [format, setFormat] = useState(initialRenameFormat || "{series_name} - T{volume_padded} - {title}");
  const [formatHs, setFormatHs] = useState(initialRenameFormatHs || "{series_name} - HS {volume_padded}");

  const preview = applyExampleTemplate(format);
  const previewHs = applyExampleTemplate(formatHs);

  return (
    <Card className="mb-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {t("rename.settingTitle")}
        </CardTitle>
        <CardDescription>{t("rename.settingDesc")}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-4">
          <FormField>
            <label className="text-sm font-medium text-muted-foreground mb-1 block">
              {t("rename.template")}
            </label>
            <FormInput
              value={format}
              onChange={(e) => setFormat(e.target.value)}
              onBlur={() => handleUpdateSetting("rename_format", format)}
              placeholder="{series_name} - T{volume_padded} - {title}"
            />
          </FormField>

          <div className="p-3 bg-muted/30 rounded-lg">
            <p className="text-xs font-medium text-muted-foreground mb-2">{t("rename.examplePreview")}</p>
            <p className="text-sm font-mono text-foreground">{preview}</p>
          </div>

          <FormField>
            <label className="text-sm font-medium text-muted-foreground mb-1 block">
              {t("rename.templateHs")}
            </label>
            <FormInput
              value={formatHs}
              onChange={(e) => setFormatHs(e.target.value)}
              onBlur={() => handleUpdateSetting("rename_format_hs", formatHs)}
              placeholder="{series_name} - HS {volume_padded}"
            />
          </FormField>

          <div className="p-3 bg-muted/30 rounded-lg">
            <p className="text-xs font-medium text-muted-foreground mb-2">{t("rename.examplePreviewHs")}</p>
            <p className="text-sm font-mono text-foreground">{previewHs}</p>
          </div>

          <div>
            <p className="text-xs font-medium text-muted-foreground mb-2">{t("rename.availableVars")}</p>
            <div className="flex flex-wrap gap-2">
              {AVAILABLE_VARS.map((v) => (
                <code
                  key={v.name}
                  className="text-xs px-2 py-1 bg-muted rounded-md cursor-pointer hover:bg-muted/80 transition-colors"
                  onClick={() => {
                    setFormat((prev) => prev + `{${v.name}}`);
                  }}
                  title={v.example}
                >
                  {`{${v.name}}`}
                </code>
              ))}
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
