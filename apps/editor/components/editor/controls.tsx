"use client";

// Contrôles compacts des panneaux de l'éditeur. Les champs de saisie gardent un brouillon local
// et ne valident qu'à la sortie du champ ou sur Entrée : une frappe n'est pas une commande.

import { ChevronDown } from "lucide-react";
import {
  useEffect,
  useId,
  useRef,
  useState,
  type ComponentProps,
  type ReactNode,
} from "react";

import { cn } from "@/lib/utils";

const FIELD =
  "h-8 w-full min-w-0 rounded-md border border-border bg-background px-2 text-base outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50 sm:text-sm";

/** Champ texte : `onCommit` reçoit la valeur à la sortie du champ ou sur Entrée, si elle a changé. */
export function CommitInput({
  value,
  onCommit,
  className,
  ...props
}: Omit<ComponentProps<"input">, "value" | "onChange" | "defaultValue"> & {
  value: string;
  onCommit: (value: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  // La valeur du document l'emporte dès qu'elle change (undo, autre panneau, commande refusée).
  useEffect(() => setDraft(value), [value]);
  const commit = () => {
    if (draft !== value) onCommit(draft);
  };
  return (
    <input
      {...props}
      className={cn(FIELD, className)}
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={() => {
        commit();
        // Une valeur refusée par le moteur ne reste pas affichée.
        setDraft((current) => (current === value ? current : value));
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          commit();
        } else if (event.key === "Escape") {
          event.stopPropagation();
          setDraft(value);
        }
      }}
    />
  );
}

/** Zone de texte : même règle de validation que `CommitInput` (⌘/Ctrl + Entrée valide). */
export function CommitTextarea({
  value,
  onCommit,
  className,
  ...props
}: Omit<ComponentProps<"textarea">, "value" | "onChange" | "defaultValue"> & {
  value: string;
  onCommit: (value: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const commit = () => {
    if (draft !== value) onCommit(draft);
  };
  return (
    <textarea
      rows={3}
      {...props}
      className={cn(FIELD, "h-auto py-1.5 leading-snug", className)}
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={() => {
        commit();
        setDraft((current) => (current === value ? current : value));
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
          event.preventDefault();
          commit();
        } else if (event.key === "Escape") {
          event.stopPropagation();
          setDraft(value);
        }
      }}
    />
  );
}

/** Nombre entier borné ; une saisie invalide est abandonnée. */
export function CommitNumber({
  value,
  onCommit,
  min,
  max,
  ...props
}: Omit<ComponentProps<"input">, "value" | "onChange" | "type"> & {
  value: number;
  onCommit: (value: number) => void;
  min: number;
  max: number;
}) {
  return (
    <CommitInput
      {...props}
      inputMode="numeric"
      value={String(value)}
      onCommit={(text) => {
        const parsed = Number(text.trim());
        if (text.trim() !== "" && Number.isInteger(parsed)) {
          onCommit(Math.min(max, Math.max(min, parsed)));
        }
      }}
    />
  );
}

export function Select({
  className,
  children,
  ...props
}: ComponentProps<"select">) {
  return (
    <select {...props} className={cn(FIELD, "pr-6", className)}>
      {children}
    </select>
  );
}

/** Champ à suggestions : liste proposée, saisie libre validée par le moteur. */
export function Combo({
  value,
  options,
  onCommit,
  ...props
}: Omit<ComponentProps<"input">, "value" | "onChange" | "list"> & {
  value: string;
  options: readonly string[];
  onCommit: (value: string) => void;
}) {
  const list = useId();
  return (
    <>
      <CommitInput
        {...props}
        list={list}
        value={value}
        onCommit={onCommit}
        autoComplete="off"
        spellCheck={false}
      />
      <datalist id={list}>
        {options.map((option) => (
          <option key={option} value={option} />
        ))}
      </datalist>
    </>
  );
}

/**
 * Sélecteur de couleur natif : `onPick` suit le curseur, `onDone` marque la fin du choix. Le
 * panneau ouvre un geste à la première valeur, pour que le choix soit une seule entrée d'undo.
 */
export function ColorSwatch({
  value,
  label,
  onPick,
  onDone,
  disabled,
}: {
  /** `#rrggbb` ou `#rrggbbaa` ; le sélecteur natif ignore l'alpha. */
  value: string;
  label: string;
  onPick: (hex: string) => void;
  onDone: () => void;
  disabled?: boolean | undefined;
}) {
  const ref = useRef<HTMLInputElement>(null);
  const done = useRef(onDone);
  done.current = onDone;
  useEffect(() => {
    const input = ref.current;
    if (!input) return;
    // `change` natif : émis une fois, à la fermeture du sélecteur (React le confond avec `input`).
    const onChange = () => done.current();
    input.addEventListener("change", onChange);
    return () => input.removeEventListener("change", onChange);
  }, []);
  return (
    <input
      ref={ref}
      type="color"
      aria-label={label}
      title={label}
      disabled={disabled}
      className="size-8 shrink-0 cursor-pointer rounded-md border border-border bg-background p-0.5 disabled:opacity-50"
      value={value.slice(0, 7)}
      onInput={(event) => onPick(event.currentTarget.value)}
      onChange={() => undefined}
    />
  );
}

/** Section repliable d'un panneau. */
export function Section({
  title,
  children,
  defaultOpen = true,
  aside,
}: {
  title: string;
  children: ReactNode;
  defaultOpen?: boolean;
  aside?: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const body = useId();
  return (
    <section className="border-b border-border">
      <h3 className="flex items-center">
        <button
          type="button"
          aria-expanded={open}
          aria-controls={body}
          className="flex min-h-9 flex-1 items-center gap-1.5 px-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
          onClick={() => setOpen((current) => !current)}
        >
          <ChevronDown
            aria-hidden="true"
            className={cn(
              "size-3.5 transition-transform",
              !open && "-rotate-90",
            )}
          />
          {title}
        </button>
        {aside}
      </h3>
      {open ? (
        <div id={body} className="flex flex-col gap-2 px-3 pb-3">
          {children}
        </div>
      ) : null}
    </section>
  );
}

/** Libellé au-dessus d'un contrôle. */
export function Field({
  label,
  children,
  className,
}: {
  label: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <label className={cn("flex min-w-0 flex-col gap-1 text-xs", className)}>
      <span className="text-muted-foreground">{label}</span>
      {children}
    </label>
  );
}

export function Checkbox({
  label,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean | undefined;
}) {
  return (
    <label className="flex min-h-8 items-center gap-2 text-sm">
      <input
        type="checkbox"
        className="size-4 accent-primary"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      {label}
    </label>
  );
}
