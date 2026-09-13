import { Check, ChevronDown, Search } from "lucide-react";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { cn } from "../lib/cn";

export type ComboBoxOption = {
  value: string;
  label: string;
  description?: string;
  recommended?: boolean;
};

export function ComboBox({
  label,
  value,
  options,
  onValueChange,
  placeholder = "Choose an option",
  emptyText = "No matching options",
  disabled = false,
}: {
  label: string;
  value: string;
  options: ComboBoxOption[];
  onValueChange: (value: string) => void;
  placeholder?: string;
  emptyText?: string;
  disabled?: boolean;
}) {
  const inputId = useId();
  const listboxId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [highlighted, setHighlighted] = useState(0);
  const selected = options.find((option) => option.value === value);
  const filtered = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) return options;
    return options.filter((option) =>
      `${option.label} ${option.description ?? ""}`
        .toLocaleLowerCase()
        .includes(normalized),
    );
  }, [options, query]);

  useEffect(() => {
    const closeOnOutsidePress = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", closeOnOutsidePress);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePress);
  }, []);

  const openList = () => {
    if (disabled) return;
    setQuery("");
    setHighlighted(Math.max(0, options.findIndex((option) => option.value === value)));
    setOpen(true);
  };

  const select = (option: ComboBoxOption) => {
    onValueChange(option.value);
    setQuery("");
    setOpen(false);
  };

  return (
    <div ref={rootRef} className="relative">
      <label htmlFor={inputId} className="block text-xs font-bold text-app-text">
        {label}
      </label>
      <div
        className={cn(
          "relative mt-2 flex h-10 items-center rounded-control border border-app-separator bg-app-bg focus-within:border-app-accent",
          disabled && "opacity-55",
        )}
      >
        <Search
          className="pointer-events-none ml-3 shrink-0 text-app-muted"
          size={15}
          aria-hidden="true"
        />
        <input
          id={inputId}
          role="combobox"
          aria-autocomplete="list"
          aria-expanded={open}
          aria-controls={listboxId}
          aria-activedescendant={
            open && filtered[highlighted]
              ? `${listboxId}-${filtered[highlighted].value}`
              : undefined
          }
          autoComplete="off"
          disabled={disabled}
          value={open ? query : (selected?.label ?? value)}
          placeholder={placeholder}
          className="h-full min-w-0 flex-1 border-0 bg-transparent px-2 text-[13px] text-app-text outline-none placeholder:text-app-muted"
          onFocus={openList}
          onChange={(event) => {
            setQuery(event.target.value);
            setHighlighted(0);
            setOpen(true);
          }}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") {
              event.preventDefault();
              if (!open) openList();
              else setHighlighted((current) => Math.min(current + 1, filtered.length - 1));
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              setHighlighted((current) => Math.max(0, current - 1));
            } else if (event.key === "Enter" && open && filtered[highlighted]) {
              event.preventDefault();
              select(filtered[highlighted]);
            } else if (event.key === "Escape") {
              event.preventDefault();
              setOpen(false);
              setQuery("");
            } else if (event.key === "Tab") {
              setOpen(false);
            }
          }}
        />
        <button
          type="button"
          tabIndex={-1}
          disabled={disabled}
          className="mr-1 inline-flex size-8 items-center justify-center rounded-compact border-0 bg-transparent text-app-muted hover:bg-app-hover hover:text-app-text"
          aria-label={open ? `Close ${label}` : `Open ${label}`}
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => (open ? setOpen(false) : openList())}
        >
          <ChevronDown
            size={16}
            className={cn("transition-transform duration-150", open && "rotate-180")}
            aria-hidden="true"
          />
        </button>
      </div>

      {open ? (
        <div
          id={listboxId}
          role="listbox"
          aria-label={label}
          className="absolute z-40 mt-1 max-h-64 w-full overflow-y-auto rounded-control border border-app-separator bg-app-raised p-1 shadow-[0_14px_34px_rgb(0_0_0/.38)] [scrollbar-color:var(--slate-separator)_transparent]"
        >
          {filtered.length === 0 ? (
            <p className="m-0 px-3 py-4 text-center text-[11px] text-app-muted">
              {emptyText}
            </p>
          ) : (
            filtered.map((option, index) => (
              <button
                type="button"
                role="option"
                id={`${listboxId}-${option.value}`}
                aria-selected={option.value === value}
                key={option.value}
                className={cn(
                  "grid min-h-10 w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-compact border-0 bg-transparent px-3 py-2 text-left text-app-text",
                  index === highlighted && "bg-app-hover",
                )}
                onMouseEnter={() => setHighlighted(index)}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => select(option)}
              >
                <span className="min-w-0">
                  <span className="flex items-center gap-2">
                    <strong className="overflow-hidden text-xs font-bold text-ellipsis whitespace-nowrap">
                      {option.label}
                    </strong>
                    {option.recommended ? (
                      <small className="rounded-full bg-app-accent/12 px-2 py-0.5 text-[9px] font-bold tracking-[.03em] text-app-accent uppercase">
                        Recommended
                      </small>
                    ) : null}
                  </span>
                  {option.description ? (
                    <small className="mt-0.5 block overflow-hidden text-[10px] text-app-muted text-ellipsis whitespace-nowrap">
                      {option.description}
                    </small>
                  ) : null}
                </span>
                {option.value === value ? (
                  <Check size={15} className="text-app-accent" aria-hidden="true" />
                ) : null}
              </button>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}
