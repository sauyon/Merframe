import { RotateCcw } from "lucide-react";
import { BrandBar } from "@/components/brand-bar";
import Merframe from "@/components/icons/merframe";
import { ErrorNote } from "@/components/page";
import { Button } from "@/components/ui/button";
import { Hint } from "@/components/ui/hint";
import { usePageQuote } from "@/lib/quotes";

export function StartupScreen({
  error,
  onRetry,
}: {
  error: string | null;
  onRetry: () => void;
}) {
  const quote = usePageQuote("startup");

  return (
    <div
      className="bg-background flex flex-col h-full"
      role={error ? "alert" : "status"}
      aria-live={error ? "assertive" : "polite"}
    >
      <BrandBar />

      <main className="flex h-full items-center justify-center gap-2 p-8">
        <div className="flex flex-col gap-2 w-full max-w-lg">
          <div className="flex items-center gap-5 animate-pulse">
            <Merframe className="text-primary size-12" />
            <div className="pt-1">
              <h1 className="text-primary mt-1 text-2xl font-bold">
                {error ? "Merframe could not start" : "Waking ship systems"}
              </h1>
            </div>
          </div>

          {error && (
            <div className="mt-8 flex flex-col gap-4">
              <ErrorNote message={error} />
              <Button variant="outline" onClick={onRetry}>
                <RotateCcw />
                Try Again
              </Button>
            </div>
          )}

          <div className="flex flex-col gap-2">
            <span className="text-sm">{quote.line}</span>
            <Hint as="span">{quote.speaker}</Hint>
          </div>
        </div>
      </main>
    </div>
  );
}
