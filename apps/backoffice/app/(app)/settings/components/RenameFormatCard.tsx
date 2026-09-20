"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, FormField, FormInput } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { TranslationKey } from "@/lib/i18n";

const AVAILABLE_VARS = [
  { name: "series_name", example: "Dragon Ball" },
  { name: "volume", example: "1" },
  { name: "volume_padded", example: "01" },
  { name: "title", example: "Son Goku et ses amis" },
  { name: "authors", example: "Akira Toriyama" },
  { name: "publish_date", example: "2003-03-04" },
  { name: "isbn", example: "978-2723434546" },
];

type TemplateKey = "regular" | "hs" | "int" | "oneshot";

const TEMPLATES: {
  key: TemplateKey;
  settingKey: string;
  labelKey: TranslationKey;
  previewKey: TranslationKey;
  defaultValue: string;
}[] = [
  {
    key: "regular",
    settingKey: "rename_format",
    labelKey: "rename.template",
    previewKey: "rename.examplePreview",
    defaultValue: "{series_name} - T{volume_padded} - {title}",
  },
  {
    key: "hs",
    settingKey: "rename_format_hs",
    labelKey: "rename.templateHs",
    previewKey: "rename.examplePreviewHs",
    defaultValue: "{series_name} - HS {volume_padded}",
  },
  {
    key: "int",
    settingKey: "rename_format_int",
    labelKey: "rename.templateInt",
    previewKey: "rename.examplePreviewInt",
    defaultValue: "{series_name} - INT {volume_padded}",
  },
  {
    key: "oneshot",
    settingKey: "rename_format_oneshot",
    labelKey: "rename.templateOneshot",
    previewKey: "rename.examplePreviewOneshot",
    defaultValue: "{series_name}",
  },
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
  initialRenameFormatInt,
  initialRenameFormatOneshot,
}: {
  handleUpdateSetting: (key: string, value: unknown) => Promise<void>;
  initialRenameFormat: string | null;
  initialRenameFormatHs: string | null;
  initialRenameFormatInt?: string | null;
  initialRenameFormatOneshot?: string | null;
}) {
  const { t } = useTranslation();
  const initials: Record<TemplateKey, string | null | undefined> = {
    regular: initialRenameFormat,
    hs: initialRenameFormatHs,
    int: initialRenameFormatInt,
    oneshot: initialRenameFormatOneshot,
  };
  const [values, setValues] = useState<Record<TemplateKey, string>>(() => {
    const initial = {} as Record<TemplateKey, string>;
    for (const tpl of TEMPLATES) {
      initial[tpl.key] = initials[tpl.key] || tpl.defaultValue;
    }
    return initial;
  });
  const [activeField, setActiveField] = useState<TemplateKey>("regular");

  const appendVar = (name: string) => {
    const key = activeField;
    setValues((prev) => ({ ...prev, [key]: prev[key] + `{${name}}` }));
  };

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
          {TEMPLATES.map((tpl) => (
            <div key={tpl.key} className="space-y-3">
              <FormField>
                <label
                  htmlFor={`rename-format-${tpl.key}`}
                  className="text-sm font-medium text-muted-foreground mb-1 block"
                >
                  {t(tpl.labelKey)}
                </label>
                <FormInput
                  id={`rename-format-${tpl.key}`}
                  value={values[tpl.key]}
                  onFocus={() => setActiveField(tpl.key)}
                  onChange={(e) =>
                    setValues((prev) => ({ ...prev, [tpl.key]: e.target.value }))
                  }
                  onBlur={() => handleUpdateSetting(tpl.settingKey, values[tpl.key])}
                  placeholder={tpl.defaultValue}
                />
              </FormField>

              <div className="p-3 bg-muted/30 rounded-lg">
                <p className="text-xs font-medium text-muted-foreground mb-2">{t(tpl.previewKey)}</p>
                <p className="text-sm font-mono text-foreground">{applyExampleTemplate(values[tpl.key])}</p>
              </div>
            </div>
          ))}

          <div>
            <p className="text-xs font-medium text-muted-foreground mb-2">{t("rename.availableVars")}</p>
            <div className="flex flex-wrap gap-2">
              {AVAILABLE_VARS.map((v) => (
                <code
                  key={v.name}
                  className="text-xs px-2 py-1 bg-muted rounded-md cursor-pointer hover:bg-muted/80 transition-colors"
                  onClick={() => appendVar(v.name)}
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
