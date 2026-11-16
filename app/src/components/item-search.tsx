import { useMemo, useState } from "react";
import { ItemImage } from "@/components/item-image";
import { SearchInput } from "@/components/search-input";
import {
  Command,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";

interface Searchable {
  name: string;
  image_name: string | null;
}

export function ItemSearch<T extends Searchable>({
  items,
  placeholder,
  onPick,
}: {
  items: readonly T[];
  placeholder: string;
  onPick: (item: T) => void;
}) {
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);

  const matches = useMemo(() => {
    const search = query.trim().toLowerCase();
    if (!search) {
      return [];
    }
    return items
      .filter((item) => item.name.toLowerCase().includes(search))
      .sort(
        (a, b) =>
          Number(b.name.toLowerCase() === search) -
          Number(a.name.toLowerCase() === search),
      )
      .slice(0, 20);
  }, [items, query]);

  const listed = open && matches.length > 0;
  return (
    <Command
      shouldFilter={false}
      vimBindings={false}
      label={placeholder}
      className="relative"
    >
      <CommandInput value={query}>
        <SearchInput
          value={query}
          onValueChange={(next) => {
            setQuery(next);
            setOpen(true);
          }}
          onFocus={() => setOpen(query.trim().length > 0)}
          onBlur={() => setOpen(false)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              setOpen(false);
            }

            if (e.key === "Home" || e.key === "End") {
              e.stopPropagation();
            }
          }}
          aria-expanded={listed}
          placeholder={placeholder}
          className="w-full"
        />
      </CommandInput>
      {listed && (
        <CommandList
          onMouseDown={(e) => e.preventDefault()}
          className="bg-popover absolute top-full z-20 mt-1 w-full rounded-md border shadow-md"
        >
          {matches.map((item) => (
            <CommandItem
              key={item.name}
              value={item.name}
              onSelect={() => {
                setQuery("");
                setOpen(false);
                onPick(item);
              }}
            >
              <ItemImage
                imageName={item.image_name}
                size={24}
                alt={item.name}
              />
              <span className="truncate">{item.name}</span>
            </CommandItem>
          ))}
        </CommandList>
      )}
    </Command>
  );
}
