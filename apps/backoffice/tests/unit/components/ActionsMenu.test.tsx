import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  ActionsMenu,
  ActionsMenuItem,
  ActionsMenuSection,
} from "@/app/components/ui/ActionsMenu";

const RECT = {
  bottom: 500,
  right: 900,
  left: 100,
  top: 460,
  width: 800,
  height: 40,
  x: 100,
  y: 460,
  toJSON: () => ({}),
} as DOMRect;

function renderMenu(props: Partial<Parameters<typeof ActionsMenu>[0]> = {}) {
  return render(
    <ActionsMenu {...props}>
      <ActionsMenuSection label="Actions">
        <ActionsMenuItem onClick={vi.fn()}>First</ActionsMenuItem>
        <ActionsMenuItem keepOpen onClick={vi.fn()}>
          Second
        </ActionsMenuItem>
      </ActionsMenuSection>
    </ActionsMenu>
  );
}

function menu(): HTMLElement {
  return document.querySelector('[role="menu"]') as HTMLElement;
}

describe("ActionsMenu", () => {
  it("is collapsed by default and expands on click", () => {
    renderMenu();
    const trigger = screen.getByRole("button", { name: "Plus" });
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(menu().getAttribute("aria-hidden")).toBe("true");

    fireEvent.click(trigger);

    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(menu().getAttribute("aria-hidden")).toBe("false");
  });

  it("closes when clicking a regular item", () => {
    renderMenu();
    fireEvent.click(screen.getByRole("button", { name: "Plus" }));

    fireEvent.click(screen.getByRole("menuitem", { name: "First" }));

    expect(menu().getAttribute("aria-hidden")).toBe("true");
  });

  it("keeps the menu open for keepOpen items", () => {
    renderMenu();
    fireEvent.click(screen.getByRole("button", { name: "Plus" }));

    fireEvent.click(screen.getByRole("menuitem", { name: "Second" }));

    expect(menu().getAttribute("aria-hidden")).toBe("false");
  });

  it("calls the item onClick handler", () => {
    const onClick = vi.fn();
    render(
      <ActionsMenu>
        <ActionsMenuItem onClick={onClick}>Run</ActionsMenuItem>
      </ActionsMenu>
    );
    fireEvent.click(screen.getByRole("button", { name: "Plus" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Run" }));

    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("closes on Escape and on an outside click", () => {
    renderMenu();
    const trigger = screen.getByRole("button", { name: "Plus" });

    fireEvent.click(trigger);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(menu().getAttribute("aria-hidden")).toBe("true");

    fireEvent.click(trigger);
    fireEvent.mouseDown(document.body);
    expect(menu().getAttribute("aria-hidden")).toBe("true");
  });

  it("anchors the popin to the button and clamps its height", () => {
    renderMenu();
    const trigger = screen.getByRole("button", { name: "Plus" });
    vi.spyOn(trigger, "getBoundingClientRect").mockReturnValue(RECT);

    fireEvent.click(trigger);

    const popin = menu();
    expect(popin.style.position).toBe("fixed");
    expect(popin.style.top).toBe("508px");
    expect(popin.style.maxHeight).toBe("248px");
    expect(popin.style.right).toBe("124px");
    expect(popin.style.overflowY).toBe("auto");
    expect(popin.style.minWidth).toBe("240px");
  });

  it("aligns the popin to the left when requested", () => {
    renderMenu({ align: "left" });
    const trigger = screen.getByRole("button", { name: "Plus" });
    vi.spyOn(trigger, "getBoundingClientRect").mockReturnValue(RECT);

    fireEvent.click(trigger);

    expect(menu().style.left).toBe("100px");
  });
});
