import { ToggleGroup } from "radix-ui";

const CELLS = [
  "top_left",
  "top_centre",
  "top_right",
  "middle_left",
  "centre",
  "middle_right",
  "bottom_left",
  "bottom_centre",
  "bottom_right",
];

function PositionPicker<P extends string>({
  id,
  label,
  value,
  options,
  disabled = false,
  onChange,
}: {
  id: string;
  label: string;
  value: P;
  options: { value: P; label: string }[];
  disabled?: boolean;
  onChange: (value: P) => void;
}) {
  return (
    <ToggleGroup.Root
      id={id}
      type="single"
      aria-label={label}
      value={value}
      disabled={disabled}
      onValueChange={(next) => next && onChange(next as P)}
      className="grid w-fit shrink-0 grid-cols-3 rounded-md border p-0.5"
    >
      {CELLS.map((cell) => {
        const option = options.find((candidate) => candidate.value === cell);
        return (
          <ToggleGroup.Item
            key={cell}
            value={cell}
            disabled={!option}
            aria-label={option?.label ?? cell}
            title={option?.label}
            className="group hover:bg-accent/30 focus-visible:ring-ring/50 inline-flex size-6 items-center justify-center rounded-sm transition-colors outline-none focus-visible:ring-[3px] disabled:pointer-events-none disabled:opacity-50"
          >
            <span className="bg-muted-foreground group-data-[state=on]:bg-primary size-1 rounded-full transition-all group-data-[state=on]:size-3 group-data-[state=on]:rounded-xs" />
          </ToggleGroup.Item>
        );
      })}
    </ToggleGroup.Root>
  );
}

export { PositionPicker };
