import { AlertCircle } from "lucide-react";
import type { ComponentProps, ReactNode } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Hint } from "@/components/ui/hint";
import { Skeleton } from "@/components/ui/skeleton";
import { counted, num } from "@/lib/format";
import { type PageQuoteKey, usePageQuote } from "@/lib/quotes";
import { cn } from "@/lib/utils";

function PageQuote({ page }: { page: PageQuoteKey }) {
  const quote = usePageQuote(page);
  return (
    <span>
      {`"${quote.line}"`}{" "}
      <span className="text-muted-foreground/70">{quote.speaker}</span>
    </span>
  );
}

type Subtitle =
  | { quote: PageQuoteKey; description?: never }
  | { description: ReactNode; quote?: never };

export function Page({
  title,
  quote,
  description,
  actions,
  loading = false,
  error = null,
  children,
}: Subtitle & {
  title: string;
  actions?: ReactNode;
  loading?: boolean;
  error?: string | null;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-6 p-6">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div className="flex flex-col gap-1">
          <h1 className="text-primary text-2xl font-bold">{title}</h1>
          <p className="text-muted-foreground text-sm">
            {quote === undefined ? description : <PageQuote page={quote} />}
          </p>
        </div>
        {actions && (
          <div className="flex flex-wrap items-center gap-2">{actions}</div>
        )}
      </header>
      {loading ? (
        <TableSkeleton />
      ) : (
        <>
          {error && <ErrorNote message={error} />}
          {children}
        </>
      )}
    </div>
  );
}

export function Section({
  id,
  title,
  description,
  action,
  className,
  controls,
  children,
}: {
  id?: string;
  title?: string;
  description?: ReactNode;
  action?: ReactNode;
  className?: string;
  controls?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Card id={id} className={cn("gap-4", className)}>
      {(title || description || action || controls) && (
        <CardHeader
          className={cn(
            "flex flex-wrap justify-between gap-3",
            controls ? "items-start" : "items-center",
          )}
        >
          <div className="flex min-w-0 flex-1 basis-64 flex-col gap-1">
            {title && <CardTitle>{title}</CardTitle>}
            {description && (
              <p className="text-muted-foreground text-sm">{description}</p>
            )}
            {controls}
          </div>
          {action}
        </CardHeader>
      )}
      <CardContent>{children}</CardContent>
    </Card>
  );
}

export function Surface({ className, ...props }: ComponentProps<"div">) {
  return <Card className={cn("gap-2 border p-4", className)} {...props} />;
}

export function Stat({
  label,
  value,
  hint,
}: {
  label: string;
  value: ReactNode;
  hint?: ReactNode;
}) {
  return (
    <Surface className="gap-1 px-4 py-3">
      <Hint as="span">{label}</Hint>
      <span className="text-foreground text-xl font-bold">{value}</span>
      {hint && <Hint as="span">{hint}</Hint>}
    </Surface>
  );
}

export function CardGrid({
  className,
  children,
}: {
  className?: string;
  children: ReactNode;
}) {
  return (
    <div
      className={cn(
        "grid gap-3 @sm:grid-cols-2 @5xl:grid-cols-3 @7xl:grid-cols-4",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function TableSkeleton({ rows = 8 }: { rows?: number }) {
  const placeholders = Array.from({ length: rows }, (_, index) => ({
    id: `placeholder-${index}`,
    opacity: 1 - index * 0.07,
  }));
  return (
    <div className="flex flex-col gap-3">
      <Skeleton className="h-9 w-72" />
      <Surface>
        {placeholders.map((placeholder) => (
          <Skeleton
            key={placeholder.id}
            className="h-6 w-full"
            style={{ opacity: placeholder.opacity }}
          />
        ))}
      </Surface>
    </div>
  );
}

export function ResultCount({
  shown,
  total,
  noun,
  plural,
}: {
  shown: number;
  total: number | undefined;
  noun: string;
  plural?: string;
}) {
  return (
    <span className="text-muted-foreground text-sm">
      {num(shown)} of {counted(total, noun, plural)}
    </span>
  );
}

export function FramedList({
  header,
  children,
  className,
}: {
  header: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "bg-background overflow-hidden rounded-xl border",
        className,
      )}
    >
      <div className="flex items-center gap-2 border-b px-3 py-2">{header}</div>
      <ul className="divide-y">{children}</ul>
    </div>
  );
}

export function EmptyNote({ children }: { children: ReactNode }) {
  return <p className="text-muted-foreground text-sm">{children}</p>;
}

export function EmptyPanel({ children }: { children: ReactNode }) {
  return (
    <p className="text-muted-foreground rounded-xl border p-8 text-center text-sm">
      {children}
    </p>
  );
}

export function ErrorNote({ message }: { message: string }) {
  return (
    <div className="text-destructive flex items-center gap-2 text-sm">
      <AlertCircle className="size-4 shrink-0" />
      <span className="min-w-0 wrap-anywhere">{message}</span>
    </div>
  );
}
