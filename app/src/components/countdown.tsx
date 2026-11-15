import { cn } from "@/lib/utils";
import { countdownTone, timeLeft } from "@/lib/world";

export function Countdown({
  seconds,
  className,
}: {
  seconds: number;
  className?: string;
}) {
  return (
    <span
      className={cn(
        "font-mono font-semibold",
        countdownTone(seconds),
        className,
      )}
    >
      {timeLeft(seconds)}
    </span>
  );
}
