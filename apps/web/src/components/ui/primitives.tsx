import { useId } from "react";

import { shorten, txUrl } from "@/lib/explorer";

type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & { busy?: boolean; variant?: "primary" | "quiet" };

export function Button({ busy = false, variant = "primary", className = "", children, disabled, ...props }: ButtonProps) {
  const look =
    variant === "primary"
      ? "bg-ink text-paper hover:bg-ink/85"
      : "border border-line bg-surface text-ink hover:border-ink/40";
  return (
    <button
      {...props}
      disabled={disabled === true || busy}
      aria-busy={busy}
      className={`inline-flex items-center justify-center gap-2 rounded-md px-4 py-2.5 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${look} ${className}`}
    >
      {busy && <span aria-hidden className="size-3 animate-spin rounded-full border-2 border-current border-r-transparent" />}
      {children}
    </button>
  );
}

export function Card({ title, children, className = "" }: { title?: string; children: React.ReactNode; className?: string }) {
  return (
    <section className={`animate-step-in rounded-lg border border-line bg-surface p-6 ${className}`}>
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
    <div className="space-y-1.5 text-sm">
      <label htmlFor={id} className="block font-medium">
        {label}
      </label>
      {children(hint ? { id, "aria-describedby": hintId } : { id })}
      {hint && (
        <p id={hintId} className="text-xs text-gray">
          {hint}
        </p>
      )}
    </div>
  );
}

export const inputClass =
  "w-full rounded-md border border-line bg-paper px-3 py-2 font-mono text-sm outline-none focus:border-ink/50";

const TONES = {
  info: "border-line bg-paper text-ink",
  warn: "border-warn/30 bg-warn/5 text-warn",
  error: "border-error/30 bg-error/5 text-error",
  ok: "border-ok/30 bg-ok/5 text-ok",
} as const;

export function Notice({ tone = "info", children }: { tone?: keyof typeof TONES; children: React.ReactNode }) {
  return (
    <div role={tone === "error" ? "alert" : "status"} className={`rounded-md border px-3 py-2 text-sm ${TONES[tone]}`}>
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
