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

type CommandResultLike = {
  stderr: string;
  stdout: string;
};

export function commandFailureMessage(
  result: CommandResultLike,
  fallback: string,
): string {
  if (result.stderr.trim()) {
    return result.stderr;
  }
  if (result.stdout.trim()) {
    return result.stdout;
  }
  return fallback;
}
