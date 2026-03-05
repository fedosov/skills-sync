export function compactPath(path: string | null | undefined): string {
  if (!path) {
    return "-";
  }
  const segments = path.split("/").filter(Boolean);
  if (segments.length <= 3) {
    return path;
  }
  return `/${segments[0]}/.../${segments[segments.length - 1]}`;
}

export function formatUnixTime(value: number | null): string {
  if (value == null || Number.isNaN(value)) return "-";
  const date = new Date(value * 1000);
  if (Number.isNaN(date.getTime())) return "-";
  return date.toLocaleString();
}

export function formatIsoTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}
