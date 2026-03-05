import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "Unknown error";
}

export function pickPreferred<T>(
  items: T[],
  preferred: string | null | undefined,
  previous: string | null | undefined,
  getKey: (item: T) => string,
): string | null {
  if (preferred && items.some((item) => getKey(item) === preferred)) {
    return preferred;
  }
  if (previous && items.some((item) => getKey(item) === previous)) {
    return previous;
  }
  const first = items[0];
  return first ? getKey(first) : null;
}
