"use client";

import { MeshGradient } from "@paper-design/shaders-react";
import { useEffect, useState, useSyncExternalStore } from "react";
import type { Messages } from "@/content/messages";

import { Isotipo } from "../brand/isotipo";

type VisualCopy = Messages["landing"]["visual"];

/** Interactive privacy preview with fixture data; never a connected payment form. */
export function HeroPaymentVisual({ copy }: { copy: VisualCopy }) {
  const [authorized, setAuthorized] = useState(false);
  const [manual, setManual] = useState(false);
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    const preference = window.matchMedia("(prefers-reduced-motion: reduce)");
    let timer: ReturnType<typeof setInterval> | undefined;
    const sync = () => {
      clearInterval(timer);
      if (!preference.matches && !manual && !paused) {
        timer = setInterval(() => setAuthorized((value) => !value), 4000);
      }
    };
    sync();
    preference.addEventListener("change", sync);
    return () => {
      clearInterval(timer);
      preference.removeEventListener("change", sync);
    };
  }, [manual, paused]);

  function selectView(value: boolean) {
    setManual(true);
    setAuthorized(value);
  }

  return (
    <div className="payment-hero-visual" data-authorized={authorized} onMouseEnter={() => setPaused(true)} onMouseLeave={() => setPaused(false)} onFocusCapture={() => setManual(true)}>
      <PaperHeroMesh />
      <div className="payment-illustration">
        <div className="payment-illustration-brand">
          <span className="inline-flex items-center gap-2 font-display text-xl font-semibold">
            <Isotipo className="h-6 w-auto text-crimson" /> Lendara
          </span>
        </div>
        <p className="payment-illustration-title">{copy.payment}</p>
        <div className="payment-view-labels" role="group" aria-label={copy.viewLabel}>
          <button type="button" aria-pressed={!authorized} onClick={() => selectView(false)}>{copy.public}</button>
          <button type="button" aria-pressed={authorized} onClick={() => selectView(true)}>{copy.authorized}</button>
        </div>
        <dl className="payment-details">
          <div><dt>{copy.from}</dt><dd>{copy.pool}</dd></div>
          <div><dt>{copy.to}</dt><dd><span className="payment-sensitive" aria-hidden={!authorized}>{copy.recipient}</span><span className="sr-only">{!authorized ? copy.private : ""}</span></dd></div>
          <div><dt>{copy.amount}</dt><dd><span className="payment-sensitive" aria-hidden={!authorized}>{copy.paymentAmount}</span><span className="sr-only">{!authorized ? copy.private : ""}</span></dd></div>
          <div><dt>{copy.asset}</dt><dd>USDC</dd></div>
        </dl>
        <div className="payment-privacy-note">
          <LockIcon /> <span>{authorized ? copy.readOnly : copy.private}</span>
        </div>
      </div>
    </div>
  );
}

/** The two entry paths share the same visual language, with distinct light palettes. */
export function EntryPaymentVisual({ copy, kind }: { copy: VisualCopy; kind: "ramp" | "direct" }) {
  const ramp = kind === "ramp";
  return (
    <div className={`entry-payment-visual entry-payment-visual--${kind}`} aria-hidden="true">
      <div className="entry-payment-window">
        <div className="entry-window-bar">
          <span className="entry-window-dots"><i /><i /><i /></span>
        </div>
        <div className="entry-window-body">
          <span className="entry-start">{copy.start}</span>
          <p className="entry-currency">{ramp ? copy.fiat : copy.wallet}</p>
          {ramp ? (
            <div className="entry-currency-chips"><span>CRC</span><span>ARS</span><span>CLP</span></div>
          ) : (
            <div className="entry-wallet-address"><WalletIcon /><span>G…<span className="entry-address-dots">••••</span></span><span>USDC</span></div>
          )}
          <div className="entry-route">
            <span>{ramp ? copy.ramp : copy.yourWallet}</span>
            <span className="entry-route-arrow">→</span>
            <span>{copy.privatePool}</span>
          </div>
          <div className={`entry-visibility ${ramp ? "entry-visibility--private" : ""}`}>
            {ramp && <LockIcon />}<span>{ramp ? copy.rampNote : copy.walletNote}</span>
          </div>
        </div>
      </div>
    </div>
  );
}

function LockIcon() {
  return <svg aria-hidden="true" focusable="false" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><rect x="5" y="10" width="14" height="11" rx="3" /><path d="M8 10V7a4 4 0 0 1 8 0v3M12 14v3" /></svg>;
}

function WalletIcon() {
  return <svg aria-hidden="true" focusable="false" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="5" width="18" height="15" rx="3" /><path d="M3 8h18M17 13h4M17 16h4" /></svg>;
}


function subscribeToMotionPreference(onChange: () => void) {
  const preference = window.matchMedia("(prefers-reduced-motion: reduce)");
  preference.addEventListener("change", onChange);
  return () => preference.removeEventListener("change", onChange);
}

function motionIsAllowed() {
  return !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** Original Paper shader parameters; a stationary frame honors reduced motion. */
function PaperHeroMesh() {
  const animate = useSyncExternalStore(subscribeToMotionPreference, motionIsAllowed, () => false);
  return (
    <div className="payment-mesh" data-testid="payment-mesh" data-speed={animate ? "1" : "0"} aria-hidden="true">
      <MeshGradient
        speed={animate ? 1 : 0}
        scale={1}
        distortion={0.8}
        swirl={0.1}
        frame={5927957.59999519}
        colors={["#FBE3ED", "#FF8A3D", "#E22366", "#F7F5F2", "#E22366"]}
        style={{ height: "100%", width: "100%", position: "absolute", inset: 0 }}
      />
    </div>
  );
}
