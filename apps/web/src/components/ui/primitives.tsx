import { useId } from "react";

import { shorten, txUrl } from "@/lib/explorer";

type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & { busy?: boolean; variant?: "primary" | "quiet" };

export function Button({ busy = false, variant = "primary", className = "", children, disabled, ...props }: ButtonProps) {
  const look =
    variant === "primary"
      ? "bg-action-primary text-text-inverse enabled:hover:bg-action-primary-hover enabled:active:bg-action-primary-pressed"
      : "border border-border-control bg-action-secondary text-ink enabled:hover:bg-action-secondary-hover enabled:active:bg-action-secondary-pressed";
  return (
    <button
      {...props}
      disabled={disabled === true || busy}
      aria-busy={busy}
      className={`inline-flex items-center justify-center gap-2 min-h-12 rounded-control px-5 py-3 text-base font-semibold transition-colors disabled:cursor-not-allowed disabled:border-transparent disabled:bg-action-disabled disabled:text-action-disabled-text ${look} ${className}`}
    >
      {busy && <span aria-hidden className="size-3 animate-spin rounded-full border-2 border-current border-r-transparent" />}
      {children}
    </button>
  );
}

export function Card({ title, children, className = "" }: { title?: string; children: React.ReactNode; className?: string }) {
  return (
    <section className={`animate-step-in rounded-2xl border border-line bg-surface p-5 sm:p-6 ${className}`}>
      {title && <h2 className="mb-4 text-base font-semibold">{title}</h2>}
      {children}
    </section>
  );
}

/** Props that tie a control to its label and hint. */
export interface FieldControl {
  id: string;
  "aria-describedby"?: string;
}

export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: (control: FieldControl) => React.ReactNode;
}) {
  const id = useId();
  const hintId = `${id}-hint`;
  return (
    <div className="space-y-2 text-sm">
      <label htmlFor={id} className="block font-medium">
        {label}
      </label>
      {children(hint ? { id, "aria-describedby": hintId } : { id })}
      {hint && (
        <p id={hintId} className="text-sm leading-[22px] text-gray">
          {hint}
        </p>
      )}
    </div>
  );
}

export const inputClass =
  "min-h-12 w-full rounded-control border border-border-control bg-surface px-3 py-2.5 font-sans text-base focus:border-ink aria-invalid:border-error disabled:bg-action-disabled disabled:text-action-disabled-text read-only:bg-paper";

const TONES = {
  info: "border-info-accent bg-surface text-info",
  warn: "border-warning-accent bg-surface text-warning",
  error: "border-error-accent bg-surface text-error",
  ok: "border-success-accent bg-surface text-success",
} as const;

export function Notice({ tone = "info", children }: { tone?: keyof typeof TONES; children: React.ReactNode }) {
  return (
    <div role={tone === "error" ? "alert" : "status"} className={`rounded-xl border border-l-4 px-4 py-3 text-sm leading-relaxed ${TONES[tone]}`}>
      {children}
    </div>
  );
}

export function TxLink({ hash }: { hash: string }) {
  return (
    <a
      href={txUrl(hash)}
      target="_blank"
      rel="noreferrer"
      title={hash}
      className="font-mono text-sm underline decoration-line underline-offset-4 hover:decoration-ink"
    >
      {shorten(hash)} ↗
    </a>
  );
}

export function TxList({ hashes }: { hashes: string[] }) {
  return (
    <ul className="space-y-1">
      {hashes.map((hash) => (
        <li key={hash}>
          <TxLink hash={hash} />
        </li>
      ))}
    </ul>
  );
}
