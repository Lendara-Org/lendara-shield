"use client";

import { useCallback, useEffect, useState } from "react";

import { ConnectGate } from "@/components/connect-gate";
import { message, useSession } from "@/components/session";
import { Button, Card, Field, Notice, TxList, inputClass } from "@/components/ui/primitives";
import { formatAmount, parseAmount } from "@/lib/amount";
import type { PoolResult, ShieldWallet } from "@/lib/pool/types";
import { decodeRecipient, encodeRecipient } from "@/lib/recipient";

export function WalletView() {
  return <ConnectGate requireAllowed>{(wallet) => <Wallet wallet={wallet} />}</ConnectGate>;
}

function Wallet({ wallet }: { wallet: ShieldWallet }) {
  const { info } = useSession();
  const asset = info?.pool.asset.code ?? "USDC";
  const [balance, setBalance] = useState<bigint | null>(null);
  const [syncing, setSyncing] = useState(true);
  const [balanceError, setBalanceError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setSyncing(true);
    setBalanceError(null);
    try {
      setBalance(await wallet.balance());
    } catch (err) {
      setBalanceError(message(err));
    } finally {
      setSyncing(false);
    }
  }, [wallet]);

  // First load: state is only set from the promise's callbacks.
  useEffect(() => {
    let active = true;
    wallet
      .balance()
      .then(
        (value) => active && setBalance(value),
        (err: unknown) => active && setBalanceError(message(err)),
      )
      .finally(() => active && setSyncing(false));
    return () => {
      active = false;
    };
  }, [wallet]);

  return (
    <div className="space-y-6">
      <Card title="Private balance">
        <div className="flex items-end justify-between gap-4">
          <p className="font-mono text-4xl" aria-live="polite">
            {balance === null ? "—" : formatAmount(balance)} <span className="text-base text-gray">{asset}</span>
          </p>
          <Button variant="quiet" onClick={() => void refresh()} busy={syncing}>
            Sync
          </Button>
        </div>
        <p className="mt-2 text-xs text-gray">Only you (and the auditor&apos;s view key) can see this balance.</p>
        {balanceError && (
          <div className="mt-3">
            <Notice tone="error">{balanceError}</Notice>
          </div>
        )}
      </Card>

      <ReceiveCard wallet={wallet} />

      <div className="grid gap-6 sm:grid-cols-2">
        <SpendForm
          title="Send privately"
          action="Send"
          asset={asset}
          withRecipient
          note="ext_amount = 0: amount and recipient stay hidden. You sign it, so your account shows as the sender (a relayer removes that later)."
          run={(amount, to) => (to ? wallet.transfer(to, amount) : Promise.reject(new Error("Missing recipient")))}
          onDone={refresh}
        />
        <SpendForm
          title="Withdraw to my wallet"
          action="Withdraw"
          asset={asset}
          note="ext_amount < 0: the amount and your address become public."
          run={(amount) => wallet.withdraw(amount)}
          onDone={refresh}
        />
      </div>
    </div>
  );
}

function ReceiveCard({ wallet }: { wallet: ShieldWallet }) {
  const [address, setAddress] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function reveal() {
    setError(null);
    try {
      setAddress(encodeRecipient(await wallet.keys()));
    } catch (err) {
      setError(message(err));
    }
  }

  async function copy() {
    if (!address) return;
    await navigator.clipboard.writeText(address);
    setCopied(true);
  }

  return (
    <Card title="Receive privately">
      <p className="mb-3 text-sm text-gray">
        Share your private address off-chain. It holds only public keys and is not linked to your account on-chain.
      </p>
      {address ? (
        <div className="flex items-start gap-2">
          <code className="block flex-1 break-all rounded-md bg-paper p-3 font-mono text-xs">{address}</code>
          <Button variant="quiet" onClick={() => void copy()}>
            {copied ? "Copied" : "Copy"}
          </Button>
        </div>
      ) : (
        <Button variant="quiet" onClick={() => void reveal()}>
          Show my private address
        </Button>
      )}
      {error && <Notice tone="error">{error}</Notice>}
    </Card>
  );
}

function SpendForm({
  title,
  action,
  asset,
  note,
  withRecipient = false,
  run,
  onDone,
}: {
  title: string;
  action: string;
  asset: string;
  note: string;
  withRecipient?: boolean;
  run: (amount: bigint, to: ReturnType<typeof decodeRecipient>) => Promise<PoolResult>;
  onDone: () => Promise<void>;
}) {
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<PoolResult | null>(null);

  const to = withRecipient ? decodeRecipient(recipient) : null;
  const stroops = parseAmount(amount);
  const ready = stroops !== null && (!withRecipient || to !== null);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!ready || stroops === null) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(await run(stroops, to));
      setAmount("");
      await onDone();
    } catch (err) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card title={title}>
      <form onSubmit={(e) => void submit(e)} className="space-y-4">
        {withRecipient && (
          <Field label="Recipient's private address" hint="shield:0x…:0x…">
            {(control) => (
              <input
                {...control}
                className={inputClass}
                value={recipient}
                onChange={(e) => setRecipient(e.target.value)}
                aria-invalid={recipient !== "" && to === null}
              />
            )}
          </Field>
        )}
        <Field label={`Amount (${asset})`} hint={note}>
          {(control) => (
            <input
              {...control}
              className={inputClass}
              inputMode="decimal"
              value={amount}
              onChange={(e) => setAmount(e.target.value.trim())}
            />
          )}
        </Field>
        <Button type="submit" busy={busy} disabled={!ready}>
          {action}
        </Button>
        {error && <Notice tone="error">{error}</Notice>}
        {result && (
          <Notice tone="ok">
            <p className="mb-1">Done:</p>
            <TxList hashes={result.hashes} />
          </Notice>
        )}
      </form>
    </Card>
  );
}
