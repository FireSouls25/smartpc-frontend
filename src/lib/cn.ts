// Minimal classnames join (no dependency). Honest and tiny.
export function cn(
  ...parts: Array<string | false | null | undefined>
): string {
  return parts.filter(Boolean).join(" ");
}
