import { countdown } from "@/lib/format";

export const DAY_MS = 86_400_000;

export function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

export function countdownTone(seconds: number): string {
  return seconds < 5 * 60 ? "text-warning" : "text-accent";
}

export function timeLeft(seconds: number): string {
  return seconds <= 0 ? "0s" : countdown(seconds);
}

export function splitNode(nodeName: string | null, nodeId: string) {
  if (!nodeName) {
    return { node: nodeId, planet: "" };
  }
  const open = nodeName.lastIndexOf("(");
  if (open < 0) {
    return { node: nodeName, planet: "" };
  }
  return {
    node: nodeName.slice(0, open).trim(),
    planet: nodeName
      .slice(open + 1)
      .replace(/\)$/, "")
      .trim(),
  };
}

export function nodeLabel(nodeName: string | null, nodeId: string): string {
  const { node, planet } = splitNode(nodeName, nodeId);
  return planet ? `${node}, ${planet}` : node;
}

export function elapsedShare(starts: string, ends: string, now: number) {
  const from = new Date(starts).getTime();
  const to = new Date(ends).getTime();
  if (to <= from) {
    return 100;
  }
  return Math.min(100, Math.max(0, ((now - from) / (to - from)) * 100));
}
