import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

function NumberInput({
  id,
  value,
  unit,
  disabled,
  className,
  onChange,
}: {
  id: string;
  value: number;
  unit?: string;
  disabled?: boolean;
  className?: string;
  onChange: (value: number) => void;
}) {
  return (
    <>
      <Input
        id={id}
        type="number"
        min={0}
        disabled={disabled}
        value={value}
        onChange={(event) => onChange(Math.max(0, Number(event.target.value)))}
        className={cn("h-8 w-20 text-right", className)}
      />
      {unit && <span className="text-sm">{unit}</span>}
    </>
  );
}

export { NumberInput };
