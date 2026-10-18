import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { FormInput, FormLabel, SettingsCard, SettingsColumn, SettingsColumns, SettingsField, SettingsSection } from "@/app/components/ui";

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

describe("SettingsColumns", () => {
  it("lays its children out in a fixed two-column grid from xl up", () => {
    const { container } = render(
      <SettingsColumns>
        <SettingsColumn>
          <span>left</span>
        </SettingsColumn>
        <SettingsColumn>
          <span>right</span>
        </SettingsColumn>
      </SettingsColumns>
    );

    const grid = container.firstElementChild as HTMLElement;
    expect(grid.className).toContain("grid");
    expect(grid.className).toContain("xl:grid-cols-2");
    expect(grid.className).toContain("items-start");
    expect(grid.className).toContain("gap-5");
    expect(grid.children).toHaveLength(2);
    expect((grid.children[0] as HTMLElement).className).toContain("space-y-5");
    expect((grid.children[1] as HTMLElement).className).toContain("space-y-5");
  });

  it("keeps each column's own top-to-bottom order", () => {
    const { container } = render(
      <SettingsColumns>
        <SettingsColumn>
          <span>first</span>
          <span>second</span>
        </SettingsColumn>
        <SettingsColumn>
          <span>third</span>
        </SettingsColumn>
      </SettingsColumns>
    );

    const grid = container.firstElementChild as HTMLElement;
    const columnTexts = (column: Element) =>
      Array.from(column.children).map((child) => child.textContent);

    expect(columnTexts(grid.children[0])).toEqual(["first", "second"]);
    expect(columnTexts(grid.children[1])).toEqual(["third"]);
  });

  it("accepts extra class names on the grid and on a column", () => {
    const { container } = render(
      <SettingsColumns className="mt-5">
        <SettingsColumn className="order-first">
          <span>left</span>
        </SettingsColumn>
        <SettingsColumn>
          <span>right</span>
        </SettingsColumn>
      </SettingsColumns>
    );

    const grid = container.firstElementChild as HTMLElement;
    expect(grid.className).toContain("mt-5");
    expect((grid.children[0] as HTMLElement).className).toContain("order-first");
  });
});

describe("SettingsSection", () => {
  it("renders an eyebrow heading above its panels", () => {
    const { container } = render(
      <SettingsSection title="Provider">
        <span>panel</span>
      </SettingsSection>
    );

    const section = container.firstElementChild as HTMLElement;
    const heading = screen.getByRole("heading", { name: "Provider" });
    expect(heading.tagName).toBe("H2");
    expect(heading.className).toContain("uppercase");
    expect(section.contains(heading)).toBe(true);
    // The heading shares its row with a hairline rule that closes the separation.
    expect(heading.parentElement?.className).toContain("flex");
    expect(heading.parentElement?.querySelector("span[aria-hidden]")).not.toBeNull();
    // The panels keep the same vertical rhythm as the rest of the settings page.
    expect(screen.getByText("panel").parentElement?.className).toContain("space-y-5");
  });

  it("keeps several panels in their given order and accepts extra class names", () => {
    const { container } = render(
      <SettingsSection title="Clients" className="mt-4">
        <span>choice</span>
        <span>credentials</span>
      </SettingsSection>
    );

    const section = container.firstElementChild as HTMLElement;
    expect(section.className).toContain("mt-4");
    const panels = screen.getByText("choice").parentElement as HTMLElement;
    expect(Array.from(panels.children).map((child) => child.textContent)).toEqual([
      "choice",
      "credentials",
    ]);
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
