"use client";

import type { KycScenario, ShieldUser } from "@lendara/shield-api-types";
import Link from "next/link";
import { useState } from "react";

import { ConnectGate } from "@/components/connect-gate";
import { message, useSession } from "@/components/session";
import { Button, Card, Field, Notice, TxLink, inputClass } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { INTENT_POLL_MS } from "@/lib/config";
import { usePoll } from "@/lib/hooks/use-poll";
import { onboardingMessage } from "@/lib/onboarding";
import type { ShieldWallet } from "@/lib/pool/types";

const SCENARIOS: { value: KycScenario; label: string }[] = [
  { value: "approved", label: "Approved: your key joins the allow-list" },
  { value: "rejected", label: "Rejected: your key never enters the pool" },
  { value: "blocked", label: "Blocked: approved, then frozen by the ASP" },
];

const STATUS_TEXT: Record<ShieldUser["asp_status"], { tone: "ok" | "warn" | "error" | "info"; text: string }> = {
  pending: { tone: "info", text: "Updating the pool's allow-list on-chain…" },
  allowed: { tone: "ok", text: "You are in the allow-list. You can deposit and transact privately." },
  not_allowed: { tone: "error", text: "KYC rejected: your key is not in the allow-list, so the pool refuses your proofs." },
  blocked: { tone: "error", text: "Your key is in the block-list: it is frozen until the ASP unblocks it." },
};

export function OnboardFlow() {
  return <ConnectGate>{(wallet) => <Onboarding wallet={wallet} />}</ConnectGate>;
}

function Onboarding({ wallet }: { wallet: ShieldWallet }) {
  const { user, setUser } = useSession();
  const [scenario, setScenario] = useState<KycScenario>("approved");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  usePoll(user?.asp_status === "pending", INTENT_POLL_MS, async () => {
    setUser(await api.user(wallet.address));
  });

  async function register() {
    setBusy(true);
    setError(null);
    try {
      const keys = await wallet.keys();
      // Proves the account is ours: nobody can attach keys to someone else's.
      const signature = await wallet.signMessage(onboardingMessage(wallet.address, keys));
      setUser(
        await api.onboard({
          stellar_account: wallet.address,
          note_public_key: keys.notePublicKey.toLowerCase(),
          encryption_public_key: keys.encryptionPublicKey.toLowerCase(),
          asp_leaf: keys.aspLeaf.toLowerCase(),
          signature,
          scenario,
        }),
      );
    } catch (err) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  }

  if (user) {
    const status = STATUS_TEXT[user.asp_status];
    return (
      <Card title="Your Shield status">
        <dl className="mb-4 grid grid-cols-[8rem_1fr] gap-y-2 text-sm">
          <dt className="text-gray">KYC</dt>
          <dd className="capitalize">{user.kyc_status}</dd>
          <dt className="text-gray">Allow-list</dt>
          <dd>{user.asp_status.replace("_", " ")}</dd>
          {user.last_asp_tx_hash && (
            <>
              <dt className="text-gray">ASP update</dt>
              <dd>
                <TxLink hash={user.last_asp_tx_hash} />
              </dd>
            </>
          )}
        </dl>
        <Notice tone={status.tone}>{status.text}</Notice>
        {user.asp_status === "allowed" && (
          <Link href="/deposit" className="mt-4 inline-block text-sm font-medium underline">
            Continue to deposit →
          </Link>
        )}
      </Card>
    );
  }

  return (
    <Card title="Register with Shield">
      <ol className="mb-5 list-decimal space-y-1 pl-5 text-sm text-gray">
        <li>Freighter signs a message to derive your pool keys, here in the browser.</li>
        <li>Freighter signs once more to prove this account is yours.</li>
        <li>Shield receives only your public keys and your allow-list leaf.</li>
        <li>After the (simulated) KYC, Shield, as the pool&apos;s ASP, adds your leaf on-chain.</li>
      </ol>
      <div className="space-y-4">
        <Field label="KYC scenario" hint="Sandbox only: choose the outcome of the simulated KYC.">
          {(control) => (
            <select
              {...control}
              className={inputClass}
              value={scenario}
              onChange={(e) => setScenario(SCENARIOS.find((s) => s.value === e.target.value)?.value ?? "approved")}
            >
              {SCENARIOS.map((s) => (
                <option key={s.value} value={s.value}>
                  {s.label}
                </option>
              ))}
            </select>
          )}
        </Field>
        <Button onClick={() => void register()} busy={busy}>
          Derive keys and register
        </Button>
        {error && <Notice tone="error">{error}</Notice>}
      </div>
    </Card>
  );
}
