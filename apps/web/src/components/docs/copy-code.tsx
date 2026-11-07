"use client";

import { useState } from "react";

export function CopyCode({ code, label, copied }: { code: string; label: string; copied: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      type="button"
      onClick={() =>
        void navigator.clipboard.writeText(code).then(() => {
          setDone(true);
          setTimeout(() => setDone(false), 1500);
        })
      }
      className="h-7 rounded-md border border-white/20 px-2.5 text-xs text-paper transition hover:border-white/50"
    >
      {done ? copied : label}
    </button>
  );
}
