import { describe, expect, it } from "vitest";
import type { PlatformContext } from "../types";
import {
  bootstrapPlatformAttributes,
  detectFallbackPlatform,
} from "./platform";

function createMatchMediaStub(matches: Record<string, boolean>) {
  return (query: string): MediaQueryList =>
    ({
      matches: Boolean(matches[query]),
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList;
}

describe("platform bootstrap", () => {
  it("applies backend platform context to document dataset", async () => {
    const root = document.createElement("html");
    const context: PlatformContext = {
      os: "linux",
      linux_desktop: "GNOME",
    };

    await bootstrapPlatformAttributes(
      () => Promise.resolve(context),
      root,
      { platform: "Linux x86_64", userAgent: "Linux" },
      createMatchMediaStub({}),
    );

    expect(root.dataset.platform).toBe("linux");
    expect(root.dataset.linuxDesktop).toBe("GNOME");
  });

  it("falls back to navigator detection when backend call fails", async () => {
    const root = document.createElement("html");

    await bootstrapPlatformAttributes(
      async () => Promise.reject(new Error("boom")),
      root,
      { platform: "MacIntel", userAgent: "Macintosh" },
      createMatchMediaStub({}),
    );

    expect(root.dataset.platform).toBe("macos");
    expect(root.dataset.linuxDesktop).toBeUndefined();
  });

  it("sets motion and contrast data attributes from media queries", async () => {
    const root = document.createElement("html");
    const matchMedia = createMatchMediaStub({
      "(prefers-reduced-motion: reduce)": true,
      "(forced-colors: active)": true,
    });

    await bootstrapPlatformAttributes(
      () => Promise.resolve({ os: "windows", linux_desktop: null }),
      root,
      { platform: "Win32", userAgent: "Windows NT 10.0" },
      matchMedia,
    );

    expect(root.dataset.motion).toBe("reduced");
    expect(root.dataset.contrast).toBe("more");
  });
});

describe("detectFallbackPlatform", () => {
  it("returns unknown for unsupported values", () => {
    expect(
      detectFallbackPlatform({ platform: "Plan9", userAgent: "foo" }),
    ).toBe("unknown");
  });
});
