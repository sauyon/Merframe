import { Progress } from "@/components/ui/progress";
import { gradeClasses } from "@/lib/rivens";
import { cn } from "@/lib/utils";
import type { AttributeGrade } from "@/types";

export function RivenRollBar({
  attribute,
  className,
}: {
  attribute: AttributeGrade;
  className?: string;
}) {
  return (
    <div className={cn("relative", className)}>
      <Progress
        value={attribute.percentile * 100}
        className="h-1"
        indicatorClassName={gradeClasses(attribute.grade).fill}
        aria-hidden="true"
      />
      <span className="bg-foreground/60 absolute -inset-y-0.5 left-1/2 w-px -translate-x-1/2" />
    </div>
  );
}
