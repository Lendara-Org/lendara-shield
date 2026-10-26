import type { Messages } from "@/content/messages";

import { Isotipo } from "../brand/isotipo";

type VisualCopy = Messages["landing"]["visual"];

/** Decorative product illustration; never a connected payment form. */
export function HeroPaymentVisual({ copy }: { copy: VisualCopy }) {
  return (
    <div className="payment-hero-visual" aria-hidden="true">
      <div className="payment-mesh" data-testid="payment-mesh" />
      <div className="payment-illustration">
        <div className="payment-illustration-brand">
          <span className="inline-flex items-center gap-2 font-display text-xl font-semibold">
            <Isotipo className="h-6 w-auto text-crimson" /> Lendara
          </span>
          <span className="payment-example">{copy.example}</span>
        </div>
        <p className="payment-illustration-title">{copy.payment}</p>
        <div className="payment-view-labels">
          <span className="payment-view-active">{copy.public}</span>
          <span>{copy.authorized}</span>
        </div>
        <dl className="payment-details">
          <div><dt>{copy.from}</dt><dd>{copy.pool}</dd></div>
          <div><dt>{copy.to}</dt><dd><span className="payment-hidden">{copy.hidden}</span></dd></div>
          <div><dt>{copy.amount}</dt><dd><span className="payment-hidden">{copy.hidden}</span></dd></div>
          <div><dt>{copy.asset}</dt><dd>USDC</dd></div>
        </dl>
        <div className="payment-privacy-note">
          <LockIcon /> <span>{copy.private}</span>
        </div>
        <p className="payment-readonly">{copy.readOnly}</p>
      </div>
      <span className="payment-testnet">{copy.testnet}</span>
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
          <span>{copy.example}</span>
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
  return <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><rect x="5" y="10" width="14" height="11" rx="3" /><path d="M8 10V7a4 4 0 0 1 8 0v3M12 14v3" /></svg>;
}

function WalletIcon() {
  return <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="5" width="18" height="15" rx="3" /><path d="M3 8h18M17 13h4M17 16h4" /></svg>;
}
