import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  Badge,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  CoverFan,
  GlassCard,
  Icon,
  JobTypeBadge,
  NavIcon,
  PageIcon,
  ProgressBadge,
  SimpleCard,
  StatusBadge,
  Tooltip,
} from "@/app/components/ui";

describe("Badge", () => {
  it("renders its children with the default variant", () => {
    render(<Badge>New</Badge>);
    const badge = screen.getByText("New");
    expect(badge.className).toContain("bg-primary/90");
  });

  it("applies a variant and merges extra classes", () => {
    render(
      <Badge variant="outline" className="extra">
        Out
      </Badge>
    );
    const badge = screen.getByText("Out");
    expect(badge.className).toContain("text-foreground");
    expect(badge).toHaveClass("extra");
  });
});

describe("StatusBadge", () => {
  it.each([
    ["running", "badge-in-progress"],
    ["success", "badge-completed"],
    ["failed", "bg-destructive/90"],
    ["cancelled", "bg-muted/60"],
    ["pending", "bg-warning/90"],
  ])("maps status %s to its variant", (status, expected) => {
    render(<StatusBadge status={status} />);
    expect(screen.getByText(status).className).toContain(expected);
  });

  it("translates the special statuses", () => {
    render(<StatusBadge status="extracting_pages" />);
    expect(screen.getByText("statusBadge.extracting_pages")).toBeInTheDocument();
  });

  it("falls back to the raw label for unknown statuses", () => {
    render(<StatusBadge status="weird" />);
    expect(screen.getByText("weird")).toBeInTheDocument();
  });
});

describe("JobTypeBadge", () => {
  it("translates a known job type and styles it", () => {
    render(<JobTypeBadge type="metadata_batch" />);
    const badge = screen.getByText("jobType.metadata_batch");
    expect(badge.className).toContain("bg-teal-500/80");
  });

  it("falls back for an unknown type", () => {
    render(<JobTypeBadge type="mystery" />);
    expect(screen.getByText("mystery").className).toContain("bg-muted/60");
  });
});

describe("ProgressBadge", () => {
  it("shows the percentage and picks a variant per range", () => {
    const { rerender } = render(<ProgressBadge progress={0} />);
    expect(screen.getByText("0%").className).toContain("badge-unread");

    rerender(<ProgressBadge progress={42} />);
    expect(screen.getByText("42%").className).toContain("badge-in-progress");

    rerender(<ProgressBadge progress={100} />);
    expect(screen.getByText("100%").className).toContain("badge-completed");
  });
});

describe("Card family", () => {
  it("keeps the hover classes by default and drops them on request", () => {
    const { container, rerender } = render(<Card>body</Card>);
    expect(container.firstElementChild?.className).toContain("hover:shadow-md");

    rerender(<Card hover={false}>body</Card>);
    expect(container.firstElementChild?.className).not.toContain("hover:shadow-md");
  });

  it("renders the header, title, description, content and footer", () => {
    render(
      <Card>
        <CardHeader>
          <CardTitle>Title</CardTitle>
          <CardDescription>Description</CardDescription>
        </CardHeader>
        <CardContent>Content</CardContent>
        <CardFooter>Footer</CardFooter>
      </Card>
    );

    expect(screen.getByRole("heading", { name: "Title" })).toBeInTheDocument();
    expect(screen.getByText("Description")).toBeInTheDocument();
    expect(screen.getByText("Content")).toBeInTheDocument();
    expect(screen.getByText("Footer")).toBeInTheDocument();
  });

  it("renders a glass card", () => {
    const { container } = render(<GlassCard>glass</GlassCard>);
    expect(container.firstElementChild?.className).toContain("glass-card");
    expect(screen.getByText("glass")).toBeInTheDocument();
  });

  it("builds a simple card with optional title, description and footer", () => {
    render(
      <SimpleCard title="S" description="D" footer={<span>F</span>}>
        body
      </SimpleCard>
    );

    expect(screen.getByRole("heading", { name: "S" })).toBeInTheDocument();
    expect(screen.getByText("D")).toBeInTheDocument();
    expect(screen.getByText("F")).toBeInTheDocument();
  });

  it("omits the header when neither title nor description is set", () => {
    const { container } = render(<SimpleCard>only body</SimpleCard>);
    expect(container.querySelector("h3")).toBeNull();
    expect(screen.getByText("only body")).toBeInTheDocument();
  });
});

describe("Tooltip", () => {
  it("renders the trigger and the hidden label", () => {
    render(<Tooltip label="More info">trigger</Tooltip>);
    expect(screen.getByText("trigger")).toBeInTheDocument();
    expect(screen.getByText("More info")).toBeInTheDocument();
  });
});

describe("Icon", () => {
  it("renders an svg with the size class", () => {
    const { container } = render(<Icon name="books" size="lg" />);
    const svg = container.querySelector("svg");
    expect(svg?.className.baseVal).toContain("w-6");
  });

  it("exposes the page and nav aliases", () => {
    const { container } = render(
      <>
        <PageIcon name="books" />
        <NavIcon name="books" />
      </>
    );
    const svgs = container.querySelectorAll("svg");
    expect(svgs).toHaveLength(2);
    expect(svgs[0].className.baseVal).toContain("w-8");
    expect(svgs[1].className.baseVal).toContain("w-4");
  });
});

describe("CoverFan", () => {
  it("renders the background and every cover with a rotation", () => {
    const { container } = render(
      <CoverFan
        background={<span>bg</span>}
        covers={[
          <span key="a">A</span>,
          <span key="b">B</span>,
        ]}
      />
    );

    expect(screen.getByText("bg")).toBeInTheDocument();
    expect(screen.getByText("A")).toBeInTheDocument();
    expect(screen.getByText("B")).toBeInTheDocument();

    const covers = container.querySelectorAll(".absolute.h-36");
    expect(covers).toHaveLength(2);
    expect((covers[0] as HTMLElement).style.transform).toContain("rotate(-6deg)");
    expect((covers[1] as HTMLElement).style.transform).toContain("rotate(6deg)");
  });
});
