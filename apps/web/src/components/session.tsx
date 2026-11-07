"use client";

/**
 * The connected user, shared by every page: Shield's public info, the
 * wallet (Freighter + pool SDK) and the user's Shield record. The wallet
 * connector is injected so tests run without Freighter or WASM.
 */
import type { Info, ShieldUser } from "@lendara/shield-api-types";
import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";

import { api } from "@/lib/api";
import { sppConnector } from "@/lib/pool/sdk";
import type { ShieldWallet, WalletConnector } from "@/lib/pool/types";

export type ConnectorFactory = (networkPassphrase: string) => WalletConnector;

interface Session {
  info: Info | null;
  /** Set when `/v1/info` failed (usually: the API is not running). */
  infoError: string | null;
  wallet: ShieldWallet | null;
  /** `undefined` until looked up, `null` if never onboarded. */
  user: ShieldUser | null | undefined;
  connecting: boolean;
  connectError: string | null;
  connect(): Promise<void>;
  setUser(user: ShieldUser | null): void;
}

const SessionContext = createContext<Session | null>(null);

export function SessionProvider({
  children,
  connectorFactory = sppConnector,
}: {
  children: React.ReactNode;
  connectorFactory?: ConnectorFactory;
}) {
  const [info, setInfo] = useState<Info | null>(null);
  const [infoError, setInfoError] = useState<string | null>(null);
  const [wallet, setWallet] = useState<ShieldWallet | null>(null);
  const [user, setUser] = useState<ShieldUser | null | undefined>(undefined);
  const [connecting, setConnecting] = useState(false);
  const [connectError, setConnectError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    api.info().then(
      (value) => active && setInfo(value),
      (err: unknown) => active && setInfoError(message(err)),
    );
    return () => {
      active = false;
    };
  }, []);

  const connect = useCallback(async () => {
    if (!info) return;
    setConnecting(true);
    setConnectError(null);
    try {
      const connected = await connectorFactory(info.network_passphrase).connect();
      setWallet(connected);
      setUser(await api.user(connected.address));
    } catch (err) {
      setConnectError(message(err));
    } finally {
      setConnecting(false);
    }
  }, [info, connectorFactory]);

  const value = useMemo<Session>(
    () => ({ info, infoError, wallet, user, connecting, connectError, connect, setUser }),
    [info, infoError, wallet, user, connecting, connectError, connect],
  );
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession(): Session {
  const session = useContext(SessionContext);
  if (!session) throw new Error("useSession must be used inside <SessionProvider>");
  return session;
}

/** A readable message for any thrown value. */
export function message(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
