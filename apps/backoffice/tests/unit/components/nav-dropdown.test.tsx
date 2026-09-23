import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { NavDropdown, type NavDropdownItem } from "@/app/components/NavDropdown";

const items: NavDropdownItem[] = [
  { href: "/genres", label: "Genres", icon: "tag", color: "text-pink-500" },
  { href: "/metadata", label: "Métadonnées", icon: "document", color: "text-sky-500" },
];

function renderDropdown() {
  render(<NavDropdown label="Données" icon="database" items={items} />);
  return screen.getByRole("button", { name: "Données" });
}

describe("NavDropdown", () => {
  it("renders the trigger closed by default", () => {
    const trigger = renderDropdown();

    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("opens the menu and renders one link per item", () => {
    const trigger = renderDropdown();

    fireEvent.click(trigger);

    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("menu")).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Genres" })).toHaveAttribute("href", "/genres");
    expect(screen.getByRole("menuitem", { name: "Métadonnées" })).toHaveAttribute(
      "href",
      "/metadata"
    );
  });

  it("closes the menu when clicking outside", () => {
    const trigger = renderDropdown();

    fireEvent.click(trigger);
    expect(screen.getByRole("menu")).toBeInTheDocument();

    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes the menu on Escape", () => {
    const trigger = renderDropdown();

    fireEvent.click(trigger);
    expect(screen.getByRole("menu")).toBeInTheDocument();

    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes the menu after selecting an item", () => {
    const trigger = renderDropdown();

    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole("menuitem", { name: "Genres" }));

    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});
