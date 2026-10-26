import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../test/render";

import { Avatar, AVATAR_HUES, avatarHue, initialsOf } from "./Avatar";

describe("Avatar", () => {
  it("picks a stable hue from one of the eight identity hues", () => {
    const names = [
      "Sofia Pessoa",
      "Rui Matos",
      "Marta Duarte",
      "Cloudhost",
      "Inês Carvalho",
      "Bankly",
      "Joana Reis",
      "Hugo Alves",
    ];
    const hues = names.map((name) => avatarHue(name));

    expect(hues.every((hue) => hue >= 1 && hue <= AVATAR_HUES)).toBe(true);
    expect(avatarHue("  sofia pessoa ")).toBe(avatarHue("Sofia Pessoa"));
    expect(new Set(hues).size).toBeGreaterThan(3);
  });

  it("derives initials from a name", () => {
    expect(initialsOf("Sofia Pessoa · Brightloop")).toBe("SP");
    expect(initialsOf("Duarte & Pessoa")).toBe("DP");
    expect(initialsOf("Cloudhost")).toBe("CL");
    expect(initialsOf("Ângela")).toBe("ÂN");
  });

  it("is decorative unless labelled; services are squircles", () => {
    const { container } = renderUi(
      <>
        <Avatar name="Cloudhost" kind="service" />
        <Avatar name="Rui Matos" initials="RM" label="Rui Matos" size="lg" />
      </>,
    );

    expect(container.querySelector("[data-kind='service']")).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByRole("img", { name: "Rui Matos" })).toHaveTextContent("RM");
  });
});
