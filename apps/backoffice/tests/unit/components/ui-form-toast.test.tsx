import { act, fireEvent, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  Button,
  FormDescription,
  FormError,
  FormField,
  FormInput,
  FormLabel,
  FormRow,
  FormSection,
  FormSelect,
  IconButton,
  Input,
  SearchInput,
  Select,
  StatBox,
  Textarea,
  Toaster,
  toast,
} from "@/app/components/ui";

describe("Button", () => {
  it("applies variant and size classes", () => {
    render(
      <Button variant="danger" size="lg">
        Delete
      </Button>
    );
    const button = screen.getByRole("button", { name: "Delete" });
    expect(button.className).toContain("bg-destructive");
    expect(button.className).toContain("h-11");
  });

  it("fires onClick and respects disabled", async () => {
    const onClick = vi.fn();
    const user = userEvent.setup();

    const { rerender } = render(<Button onClick={onClick}>Go</Button>);
    await user.click(screen.getByRole("button", { name: "Go" }));
    expect(onClick).toHaveBeenCalledTimes(1);

    rerender(
      <Button onClick={onClick} disabled>
        Go
      </Button>
    );
    await user.click(screen.getByRole("button", { name: "Go" }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});

describe("IconButton", () => {
  it("renders a title and the requested size", () => {
    render(
      <IconButton title="Close" size="sm">
        x
      </IconButton>
    );
    const button = screen.getByTitle("Close");
    expect(button.className).toContain("h-8");
    expect(button.className).toContain("rounded-md");
  });
});

describe("StatBox", () => {
  it("renders the value, label and icon with the chosen variant", () => {
    const { container } = render(
      <StatBox value="12" label="Books" variant="success" icon={<span>ico</span>} />
    );

    expect(screen.getByText("12")).toBeInTheDocument();
    expect(screen.getByText("Books")).toBeInTheDocument();
    expect(screen.getByText("ico")).toBeInTheDocument();
    expect(container.firstElementChild?.className).toContain("bg-success/10");
  });

  it("defaults to the neutral variant without an icon", () => {
    const { container } = render(<StatBox value="0" label="None" />);
    expect(container.firstElementChild?.className).toContain("bg-muted/50");
    expect(container.querySelector(".text-xl")).toBeNull();
  });
});

describe("Input family", () => {
  it("renders a labelled input and surfaces errors", () => {
    render(<Input label="Name" error="Required" defaultValue="Ann" />);
    expect(screen.getByText("Name")).toBeInTheDocument();
    expect(screen.getByRole("textbox")).toHaveValue("Ann");
    expect(screen.getByText("Required")).toBeInTheDocument();
  });

  it("forwards the ref to the input", () => {
    const ref = { current: null as HTMLInputElement | null };
    render(<Input ref={ref} />);
    expect(ref.current).toBeInstanceOf(HTMLInputElement);
  });

  it("renders a select with its options", () => {
    render(
      <Select label="Mode" error="Pick one">
        <option value="a">A</option>
      </Select>
    );
    expect(screen.getByText("Mode")).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "A" })).toBeInTheDocument();
    expect(screen.getByText("Pick one")).toBeInTheDocument();
  });

  it("renders a textarea with a label", () => {
    render(<Textarea label="Notes" placeholder="Write" />);
    expect(screen.getByText("Notes")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Write")).toBeInstanceOf(HTMLTextAreaElement);
  });

  it("renders a search input with an icon", () => {
    const { container } = render(<SearchInput icon={<span>ico</span>} placeholder="Search" />);
    expect(screen.getByText("ico")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Search")).toBeInTheDocument();
    expect(container.querySelector(".pl-10")).toBeInTheDocument();
  });
});

describe("Form family", () => {
  it("marks a required label with an asterisk", () => {
    render(<FormLabel required>Email</FormLabel>);
    expect(screen.getByText("Email")).toBeInTheDocument();
    expect(screen.getByText("*")).toBeInTheDocument();
  });

  it("renders a field, row and section with titles", () => {
    render(
      <FormSection title="Profile" description="Your details">
        <FormField>
          <FormLabel>Name</FormLabel>
        </FormField>
        <FormRow>
          <FormInput placeholder="first" />
          <FormSelect error="bad">
            <option value="a">A</option>
          </FormSelect>
        </FormRow>
      </FormSection>
    );

    expect(screen.getByRole("heading", { name: "Profile" })).toBeInTheDocument();
    expect(screen.getByText("Your details")).toBeInTheDocument();
    expect(screen.getByText("Name")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("first")).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "A" })).toBeInTheDocument();
    expect(screen.getByPlaceholderText("first").className).toContain("border-input");
  });

  it("applies the error styling and renders the error and description", () => {
    render(
      <>
        <FormInput error="invalid" placeholder="name" />
        <FormError>invalid</FormError>
        <FormDescription>help text</FormDescription>
      </>
    );

    expect(screen.getByPlaceholderText("name").className).toContain("border-destructive");
    expect(screen.getByText("help text")).toBeInTheDocument();
  });
});

describe("Toast", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    act(() => {
      vi.runOnlyPendingTimers();
    });
    vi.useRealTimers();
  });

  it("renders nothing until a toast is queued", () => {
    render(<Toaster />);
    expect(screen.queryByText("Saved")).toBeNull();

    act(() => {
      toast("Saved");
    });
    expect(screen.getByText("Saved")).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(screen.queryByText("Saved")).toBeNull();
  });

  it("uses the variant styling and can be dismissed manually", () => {
    render(<Toaster />);
    act(() => {
      toast("Oops", "error");
    });

    const message = screen.getByText("Oops");
    expect(message.parentElement?.className).toContain("border-destructive/50");

    fireEvent.click(screen.getByRole("button"));
    expect(screen.queryByText("Oops")).toBeNull();
  });

  it("renders every distinct variant", () => {
    render(<Toaster />);
    act(() => {
      toast("ok", "success");
      toast("info", "info");
    });

    expect(screen.getByText("ok").parentElement?.className).toContain("border-success/50");
    expect(screen.getByText("info").parentElement?.className).toContain("border-primary/50");
  });
});
