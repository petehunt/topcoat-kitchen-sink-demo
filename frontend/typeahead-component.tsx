import React, {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import useSWR from "swr";

export type ProductSuggestion = {
  name: string;
  sku: string;
  specification: string;
  href: string;
};

export type TypeaheadProps = {
  initial_value: string;
  suggestions_url: string;
};

declare global {
  interface Window {
    htmx: {
      process(element: Element): void;
    };
  }
}

const fetcher = async (url: string): Promise<ProductSuggestion[]> => {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`suggestions request failed: ${response.status}`);
  return response.json();
};

export function Typeahead({ initial_value, suggestions_url }: TypeaheadProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const listId = `catalog-typeahead-${useId().replaceAll(":", "")}`;
  const [query, setQuery] = useState(initial_value);
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const [style, setStyle] = useState<React.CSSProperties>({});
  const { data: products = [] } = useSWR<ProductSuggestion[]>(
    suggestions_url,
    fetcher,
    { revalidateOnMount: false },
  );

  const matches = useMemo(() => {
    const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (!terms.length) return [];
    return products
      .filter((product) => {
        const searchable = `${product.name} ${product.sku} ${product.specification}`.toLowerCase();
        return terms.every((term) => searchable.includes(term));
      })
      .slice(0, 8);
  }, [products, query]);

  const position = useCallback(() => {
    const input = inputRef.current;
    if (!input) return;
    const bounds = input.getBoundingClientRect();
    setStyle({
      left: bounds.left,
      top: bounds.bottom + 4,
      width: bounds.width,
    });
  }, []);

  useEffect(() => {
    if (!open) return;
    position();
    window.addEventListener("resize", position);
    window.addEventListener("scroll", position, true);
    return () => {
      window.removeEventListener("resize", position);
      window.removeEventListener("scroll", position, true);
    };
  }, [open, position]);

  useEffect(() => {
    setActiveIndex(-1);
    setOpen(matches.length > 0 && query.trim().length > 0);
  }, [matches.length, query]);

  useEffect(() => {
    if (!open) return;
    const list = document.getElementById(listId);
    if (list) window.htmx.process(list);
  }, [listId, matches, open]);

  useEffect(() => {
    if (activeIndex < 0) return;
    document
      .getElementById(`${listId}-${activeIndex}`)
      ?.dispatchEvent(new MouseEvent("mouseover", { bubbles: true }));
  }, [activeIndex, listId]);

  const select = (next: number) => {
    if (!matches.length) return;
    setActiveIndex((next + matches.length) % matches.length);
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      select(activeIndex + 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      select(activeIndex - 1);
    } else if (event.key === "Enter" && activeIndex >= 0) {
      event.preventDefault();
      document.getElementById(`${listId}-${activeIndex}`)?.click();
    } else if (event.key === "Escape") {
      setOpen(false);
    }
  };

  const list = open
    ? createPortal(
        <div id={listId} className="topcoat-typeahead" role="listbox" style={style}>
          {matches.map((product, index) => (
            <a
              id={`${listId}-${index}`}
              className="topcoat-typeahead__item"
              href={product.href}
              data-preload="mouseover"
              role="option"
              aria-selected={index === activeIndex}
              key={product.sku}
              onMouseDown={(event) => event.preventDefault()}
            >
              <span className="topcoat-typeahead__heading">
                <span>{product.name}</span>
                <span className="topcoat-typeahead__sku">{product.sku}</span>
              </span>
              <span className="topcoat-typeahead__specification">
                {product.specification}
              </span>
            </a>
          ))}
        </div>,
        document.body,
      )
    : null;

  return (
    <>
      <input
        ref={inputRef}
        id="catalog-search"
        name="q"
        value={query}
        autoComplete="off"
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={activeIndex >= 0 ? `${listId}-${activeIndex}` : undefined}
        placeholder="Search by product, material, or specification"
        className="min-w-0 flex-1 px-4 py-2.5 text-sm outline-none placeholder:text-slate-400"
        onChange={(event) => setQuery(event.target.value)}
        onFocus={() => setOpen(matches.length > 0)}
        onBlur={() => window.setTimeout(() => setOpen(false), 100)}
        onKeyDown={handleKeyDown}
      />
      {list}
    </>
  );
}
