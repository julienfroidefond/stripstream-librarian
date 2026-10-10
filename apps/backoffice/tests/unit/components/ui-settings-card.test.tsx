import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { FormInput, FormLabel, SettingsCard, SettingsField } from "@/app/components/ui";

describe("FormLabel", () => {
  it("defaults to the foreground style", () => {
    render(<FormLabel>Name</FormLabel>);
    const label = screen.getByText("Name");
    expect(label.className).toContain("text-foreground");
  });

  it("uses the muted settings style when variant='settings'", () => {
    render(<FormLabel variant="settings">Name</FormLabel>);
    const label = screen.getByText("Name");
    expect(label.className).toContain("text-muted-foreground");
    expect(label.className).toContain("mb-1");
    expect(label.className).toContain("block");
  });
});

describe("SettingsCard", () => {
  it("renders an icon, title, description and spaced content", () => {
    const { container } = render(
      <SettingsCard icon="settings" title="Section" description="Details">
        <span>body</span>
      </SettingsCard>
    );

    expect(screen.getByText("Section")).toBeInTheDocument();
    expect(screen.getByText("Details")).toBeInTheDocument();
    expect(screen.getByText("body")).toBeInTheDocument();
    // Icon renders an <svg> next to the title.
    expect(container.querySelector("svg")).not.toBeNull();
    const content = screen.getByText("body").parentElement;
    expect(content?.className).toContain("space-y-4");
  });

  it("omits the description and accepts a custom content className", () => {
    const { container } = render(
      <SettingsCard title="Section" contentClassName="space-y-6">
        <span>body</span>
      </SettingsCard>
    );

    expect(container.querySelector("svg")).toBeNull();
    expect(screen.getByText("body").parentElement?.className).toContain("space-y-6");
  });
});

describe("SettingsField", () => {
  it("renders a settings label, the control and helper text", () => {
    render(
      <SettingsField label="Bot token" help="Where to find it">
        <FormInput placeholder="token" />
      </SettingsField>
    );

    const label = screen.getByText("Bot token");
    expect(label.className).toContain("text-muted-foreground");
    expect(screen.getByPlaceholderText("token")).toBeInTheDocument();
    expect(screen.getByText("Where to find it")).toBeInTheDocument();
  });

  it("associates the label with a control id", () => {
    render(
      <SettingsField label="Token" labelHtmlFor="token-input">
        <FormInput id="token-input" placeholder="token" />
      </SettingsField>
    );

    expect(screen.getByLabelText("Token")).toBe(screen.getByPlaceholderText("token"));
  });
});
