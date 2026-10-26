import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";

import { API_BASE_URL } from "@/lib/config";

import { info } from "./fixtures";

export const url = (path: string) => `${API_BASE_URL}${path}`;

/** By default the API answers `/v1/info`; each test adds what it needs. */
export const server = setupServer(http.get(url("/v1/info"), () => HttpResponse.json(info)));

export const apiError = (status: number, error: string, message: string) =>
  HttpResponse.json({ error, message }, { status });
