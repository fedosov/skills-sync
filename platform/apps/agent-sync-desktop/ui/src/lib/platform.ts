import { getPlatformContext } from "../tauriApi";
import type { PlatformContext, PlatformOs } from "../types";

type NavigatorLike = Pick<Navigator, "platform" | "userAgent">;
type MatchMediaFn = (query: string) => MediaQueryList;

function normalizePlatform(value: string | null | undefined): PlatformOs {
  switch ((value ?? "").toLowerCase()) {
    case "macos":
      return "macos";
    case "windows":
      return "windows";
    case "linux":
      return "linux";
    default:
      return "unknown";
  }
}

function normalizeLinuxDesktop(
  value: string | null | undefined,
): string | null {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}

function readMediaQuery(matchMedia: MatchMediaFn, query: string): boolean {
  try {
    return Boolean(matchMedia(query).matches);
  } catch {
    return false;
  }
}

export function detectFallbackPlatform(nav: NavigatorLike): PlatformOs {
  const platform = nav.platform.toLowerCase();
  const userAgent = nav.userAgent.toLowerCase();

  if (platform.includes("mac") || userAgent.includes("mac")) {
    return "macos";
  }
  if (platform.includes("win") || userAgent.includes("windows")) {
    return "windows";
  }
  if (
    platform.includes("linux") ||
    userAgent.includes("linux") ||
    userAgent.includes("x11")
  ) {
    return "linux";
  }
  return "unknown";
}

export function applyPlatformDataset(
  context: PlatformContext,
  root: HTMLElement,
): void {
  root.dataset.platform = normalizePlatform(context.os);
  const linuxDesktop = normalizeLinuxDesktop(context.linux_desktop);
  if (root.dataset.platform === "linux" && linuxDesktop) {
    root.dataset.linuxDesktop = linuxDesktop;
  } else {
    delete root.dataset.linuxDesktop;
  }
}

export function applyAccessibilityDataset(
  root: HTMLElement,
  matchMedia: MatchMediaFn,
): void {
  root.dataset.motion = readMediaQuery(
    matchMedia,
    "(prefers-reduced-motion: reduce)",
  )
    ? "reduced"
    : "normal";

  root.dataset.contrast =
    readMediaQuery(matchMedia, "(forced-colors: active)") ||
    readMediaQuery(matchMedia, "(prefers-contrast: more)")
      ? "more"
      : "normal";
}

function safeDefaultMatchMedia(query: string): MediaQueryList {
  if (typeof window.matchMedia !== "function") {
    return {
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    } as MediaQueryList;
  }
  return window.matchMedia(query);
}

export async function bootstrapPlatformAttributes(
  fetchContext: () => Promise<PlatformContext> = getPlatformContext,
  root: HTMLElement = document.documentElement,
  nav: NavigatorLike = navigator,
  matchMedia: MatchMediaFn = safeDefaultMatchMedia,
): Promise<PlatformContext> {
  let context: PlatformContext;
  try {
    const backend = await fetchContext();
    context = {
      os: normalizePlatform(backend.os),
      linux_desktop: normalizeLinuxDesktop(backend.linux_desktop),
    };
  } catch {
    context = {
      os: detectFallbackPlatform(nav),
      linux_desktop: null,
    };
  }

  applyPlatformDataset(context, root);
  applyAccessibilityDataset(root, matchMedia);
  return context;
}
