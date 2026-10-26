import type { ReactNode } from "react";
import { CopyCode } from "./copy-code";

// Building blocks for the docs pages. Labels for the copy button come from
// the page language through CodeBlock's props.

export function H2({ id, children }: { id: string; children: ReactNode }) {
  return (
    <h2 id={id} className="group mt-12 scroll-mt-20 text-xl font-semibold text-ink first:mt-0">
      <a href={`#${id}`} className="no-underline">
        {children}
        <span aria-hidden className="ml-2 text-gray opacity-0 transition group-hover:opacity-100">
          #
        </span>
      </a>
    </h2>
  );
}

export function H3({ children }: { children: ReactNode }) {
  return <h3 className="mt-8 text-base font-semibold text-ink">{children}</h3>;
}

export function P({ children }: { children: ReactNode }) {
  return <p className="mt-4 leading-relaxed text-ink/85">{children}</p>;
}

export function C({ children }: { children: ReactNode }) {
  return <code className="rounded bg-paper px-1.5 py-0.5 font-mono text-[0.85em] text-ink">{children}</code>;
}

export function A({ href, children }: { href: string; children: ReactNode }) {
  const external = href.startsWith("http");
  return (
    <a href={href} className="text-ink underline underline-offset-2" {...(external && { target: "_blank", rel: "noreferrer" })}>
      {children}
    </a>
  );
}

export function List({ children, ordered = false }: { children: ReactNode; ordered?: boolean }) {
  const Tag = ordered ? "ol" : "ul";
  return (
    <Tag className={`mt-4 flex flex-col gap-2 pl-5 leading-relaxed text-ink/85 ${ordered ? "list-decimal" : "list-disc"}`}>
      {children}
    </Tag>
  );
}

export function Callout({ title, children, tone = "info" }: { title: string; children: ReactNode; tone?: "info" | "warning" }) {
  return (
    <aside
      className={`mt-6 rounded-lg border-l-2 bg-paper px-4 py-3 text-sm leading-relaxed ${
        tone === "warning" ? "border-wine" : "border-ink"
      }`}
    >
      <p className="font-semibold text-ink">{title}</p>
      <div className="mt-1 text-ink/85">{children}</div>
    </aside>
  );
}

export function CodeBlock({ code, title, copy }: { code: string; title?: string; copy: [label: string, copied: string] }) {
  return (
    <figure className="mt-5 overflow-hidden rounded-lg border border-white/10 bg-ink shadow-lg">
      <figcaption className="flex items-center justify-between border-b border-white/10 px-4 py-1.5">
        <span className="font-mono text-xs text-paper/60">{title ?? ""}</span>
        <CopyCode code={code} label={copy[0]} copied={copy[1]} />
      </figcaption>
      <pre className="overflow-x-auto p-4 font-mono text-[13px] leading-relaxed text-paper">
        <code>{code}</code>
      </pre>
    </figure>
  );
}

export function Table({ head, rows }: { head: string[]; rows: ReactNode[][] }) {
  return (
    <div className="mt-5 overflow-x-auto rounded-lg border border-line">
      <table className="w-full text-left text-sm [&_td:first-child_code]:whitespace-nowrap">
        <thead className="bg-paper text-xs text-gray">
          <tr>
            {head.map((h) => (
              <th key={h} className="px-4 py-2 font-medium">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {rows.map((row, i) => (
            <tr key={i}>
              {row.map((cell, j) => (
                <td key={j} className="px-4 py-2.5 align-top text-ink/85">
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
