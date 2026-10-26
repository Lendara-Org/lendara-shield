"use client";

import type { KycScenario, ShieldUser } from "@lendara/shield-api-types";
import Link from "next/link";
import { useEffect, useState } from "react";

import { ConnectGate } from "@/components/connect-gate";
import { useI18n } from "@/components/i18n";
import { message, useSession } from "@/components/session";
import { Button, Card, Field, Notice, TxLink, inputClass } from "@/components/ui/primitives";
import { api } from "@/lib/api";
import { INTENT_POLL_MS, ONBOARDING_RETRY_AFTER_MS } from "@/lib/config";
import { usePoll } from "@/lib/hooks/use-poll";
import { onboardingMessage } from "@/lib/onboarding";
import type { ShieldWallet } from "@/lib/pool/types";

const SCENARIOS: KycScenario[] = ["approved", "rejected", "blocked"];

const TONE: Record<ShieldUser["asp_status"], "ok" | "warn" | "error" | "info"> = {
  pending: "info",
  allowed: "ok",
  not_allowed: "error",
  blocked: "error",
};

export function OnboardFlow() {
  return <ConnectGate>{(wallet) => <Onboarding wallet={wallet} />}</ConnectGate>;
}

function Onboarding({ wallet }: { wallet: ShieldWallet }) {
  const { user, setUser } = useSession();
  const { t, href } = useI18n();
  const [scenario, setScenario] = useState<KycScenario>("approved");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  usePoll(user?.asp_status === "pending", INTENT_POLL_MS, async () => {
    setUser(await api.user(wallet.address));
  });

  // Still pending after a while: the ASP write may never have been sent.
  const [stuck, setStuck] = useState(false);
  const pending = user?.asp_status === "pending";
  useEffect(() => {
    if (!pending) return;
    const timer = setTimeout(() => setStuck(true), ONBOARDING_RETRY_AFTER_MS);
    return () => {
      clearTimeout(timer);
      setStuck(false);
    };
  }, [pending]);

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
    return (
      <Card title={t.onboard.statusTitle}>
        <dl className="mb-4 grid grid-cols-[8rem_1fr] gap-y-2 text-sm">
          <dt className="text-gray">{t.onboard.kyc}</dt>
          <dd className="capitalize">{t.onboard.kycStatus[user.kyc_status]}</dd>
          <dt className="text-gray">{t.onboard.allowList}</dt>
          <dd>{t.aspStatus[user.asp_status]}</dd>
          {user.last_asp_tx_hash && (
            <>
              <dt className="text-gray">{t.onboard.aspUpdate}</dt>
              <dd>
                <TxLink hash={user.last_asp_tx_hash} />
              </dd>
            </>
          )}
        </dl>
        <Notice tone={TONE[user.asp_status]}>{t.onboard.statusText[user.asp_status]}</Notice>
        {pending && stuck && (
          <div className="mt-4 space-y-3">
            <p className="text-sm text-gray">{t.onboard.stillPending}</p>
            <Button onClick={() => void register()} busy={busy}>
              {t.onboard.retry}
            </Button>
            {error && <Notice tone="error">{error}</Notice>}
          </div>
        )}
        {user.asp_status === "allowed" && (
          <Link href={href("/deposit")} className="mt-4 inline-block text-sm font-medium underline">
            {t.onboard.continue}
          </Link>
        )}
      </Card>
    );
  }

  return (
    <Card title={t.onboard.registerTitle}>
      <ol className="mb-5 list-decimal space-y-1 pl-5 text-sm text-gray">
        {t.onboard.steps.map((step) => (
          <li key={step}>{step}</li>
        ))}
      </ol>
      <div className="space-y-4">
        <Field label={t.onboard.scenario} hint={t.onboard.scenarioHint}>
          {(control) => (
            <select
              {...control}
              className={inputClass}
              value={scenario}
              onChange={(e) => setScenario(SCENARIOS.find((s) => s === e.target.value) ?? "approved")}
            >
              {SCENARIOS.map((s) => (
                <option key={s} value={s}>
                  {t.onboard.scenarios[s]}
                </option>
              ))}
            </select>
          )}
        </Field>
        <Button onClick={() => void register()} busy={busy}>
          {t.onboard.register}
        </Button>
        {error && <Notice tone="error">{error}</Notice>}
      </div>
    </Card>
  );
}
