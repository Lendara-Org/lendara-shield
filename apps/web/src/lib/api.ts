/**
 * Typed client of the Shield API. Types come from `openapi.yaml` through
 * `@lendara/shield-api-types`; nothing here is written by hand.
 */
import type {
  ApiError,
  CreateIntentRequest,
  Info,
  Intent,
  OnboardingRequest,
  ShieldUser,
} from "@lendara/shield-api-types";

import { API_BASE_URL } from "./config";

/** A non-2xx answer, with the API's stable error code. */
export class ApiRequestError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "ApiRequestError";
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_BASE_URL}${path}`, {
      ...init,
      headers: { accept: "application/json", ...(init?.body ? { "content-type": "application/json" } : {}) },
    });
  } catch {
    throw new ApiRequestError(0, "network", "Shield's API is unreachable. Is it running?");
  }
  const body: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    const error = isApiError(body) ? body : { error: "unexpected", message: `HTTP ${response.status}` };
    throw new ApiRequestError(response.status, error.error, error.message);
  }
  // The API is validated against the same contract these types come from.
  return body as T;
}

function isApiError(body: unknown): body is ApiError {
  return (
    typeof body === "object" &&
    body !== null &&
    typeof (body as Record<string, unknown>).error === "string" &&
    typeof (body as Record<string, unknown>).message === "string"
  );
}

const post = (body: unknown): RequestInit => ({ method: "POST", body: JSON.stringify(body) });

export const api = {
  info: () => request<Info>("/v1/info"),
  onboard: (body: OnboardingRequest) => request<ShieldUser>("/v1/onboarding", post(body)),
  /** `null` when the account never onboarded. */
  user: async (account: string): Promise<ShieldUser | null> => {
    try {
      return await request<ShieldUser>(`/v1/users/${encodeURIComponent(account)}`);
    } catch (err) {
      if (err instanceof ApiRequestError && err.status === 404) return null;
      throw err;
    }
  },
  createIntent: (body: CreateIntentRequest) => request<Intent>("/v1/intents", post(body)),
  intent: (id: string) => request<Intent>(`/v1/intents/${encodeURIComponent(id)}`),
};
