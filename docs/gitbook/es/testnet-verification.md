# Verificación en testnet (2026-10-28)

Revisiones de solo lectura del despliegue en vivo (`lendara-shield.duckdns.org`), del anchor del sandbox de Lendara y de los contratos del pool en la testnet de Stellar. No se envió ninguna transacción.

Ledger de referencia: `5437355` (2026-10-28T18:00:15Z).

## 1. Circuito de punta a punta

### Despliegue en vivo

| Revisión | Resultado |
|---|---|
| `GET /health` | `200` `{"status":"ok","database":"ok"}` |
| `GET /v1/info` | `200`, rampa y directo habilitados, anchor `lendara-monorepo-fixh.vercel.app` |
| `/en`, `/es`, `/en/docs`, `/en/onboard`, `/en/deposit`, `/en/wallet`, `/en/audit` | `200` |
| `/api-reference.html`, `/spp/deployments.json` | `200` |
| `POST /v1/intents` con `{}` | `400 invalid_request` |
| `POST /v1/onboarding` con `{}` | `400 invalid_request` |
| `GET /v1/audit/transactions` sin token | `401 unauthorized` |

### Anchor (sandbox de Lendara)

| Revisión | Resultado |
|---|---|
| `stellar.toml` | passphrase de testnet, `WEB_AUTH_ENDPOINT` y `TRANSFER_SERVER_SEP0024` en `lendaraweb-production.up.railway.app`, emisor del USDC `GBVIQ7QV…H47B5` |
| SEP-24 `/info` | depósito de USDC habilitado, mín. 1, máx. 10000 |
| Challenge SEP-10 | `200` |

### On-chain

- Las 8 transacciones listadas en *Proof on testnet* del README del repositorio (también listadas en [Arquitectura técnica](technical-architecture.md), sección 9) son exitosas en Horizon.
- La intake (`GC5VXVUOU6OHZVZ3KZNFK5CJNOEDQUMOOZI32VHYHYV64GBHVCPTEHPQ`) completó 5 intents de rampa el 2026-10-26, cada uno como pago del anchor → depósito en el pool → transferencia privada, con unos 20 s de diferencia. Ninguna transacción fallida.
- El saldo de USDC de la intake es `0`: no quedan fondos en custodia.
- Última actividad de la intake: 2026-10-26T15:39:22Z.

### Tests locales

| Suite | Resultado |
|---|---|
| `make test-js` | 110/110 aprobados (18 archivos) |
| `make test-pool-cli` | 7/7 aprobados |
| `make test-api` | No se corrió: no hay Docker/Postgres en la máquina |

## 2. Configuración y estado del activo

**Criterio de aprobación:** la web, el API, el anchor y los contratos usan la misma configuración esperada. **Resultado: aprobado.**

| Elemento | Web (`/spp/deployments.json` + bundle) | API (`/v1/info`) | Anchor | On-chain | OK |
|---|---|---|---|---|---|
| Red | testnet, `Test SDF Network ; September 2015`, RPC `soroban-testnet.stellar.org` | igual | igual | — | ✅ |
| Emisor del USDC | `GBVIQ7QVTTJ4NLFTYYFPEKWYJWAMKDAOFYSR52ZGWJFSGTSVOO5H47B5` | igual | igual, también en `ACCOUNTS` | — | ✅ |
| Token (SAC) | `CAE6WLIMEMDSRSZZW7H3O7DJS2DRDV5NTR2FHE53TKGJMJOHWGKSR4QX` | igual | — | `stellar contract id asset` da el mismo id; `name()` = `USDC:GBVI…` | ✅ |
| Pool | `CDD66GL3HT5JLRIB5BHAV36XFSEAGRXMGRJJ4VK747ODBXEJ77YAC7OF` | igual | — | `Token` y `Verifier` coinciden; `DepositsPaused = false` | ✅ |
| Allow-list del ASP | `CCETAQARCUMNMCLJKQM6UNY2QJDOJZYGI54MXA3FMPJY4VJDMN2L26GB` | igual | — | `ASPMembership` del pool | ✅ |
| Block-list del ASP | `CCWU7V5HXV6NKPJ3UEZMTGFWK3V35YO42AQTPSXPN2GSDK3RXLICNLKT` | igual | — | `ASPNonMembership` del pool | ✅ |
| Política / GVK | `allowlist` + `blocklist`, `traceable`, `gvkAuthorityPubKey` | igual | — | `PolicyFlags = 3`, `GvkMode = 2`, `AdminViewKey` es igual a la llave publicada | ✅ |
| Dominio del KDF | `Lendara Shield` | igual | — | igual | ✅ |
| Admin | operador `GCCWXZJTF7XMEISC2CXLO6SWETJHE4YBBNVXSMRYOWH7RUOEN2YGGVTE` | — | — | `get_admin` del pool y de ambos contratos del ASP | ✅ |
| Trustline | — | — | — | la intake tiene una trustline de USDC autorizada; el emisor tiene `auth_required = false` | ✅ |
| Modo rampa | ofrecido | `enabled: true` | depósito SEP-24 habilitado, SEP-10 funciona | — | ✅ |
| Modo directo | ofrecido | `enabled: true` | n/a | — | ✅ |

El bundle de la web apunta a `https://lendara-shield.duckdns.org` (API) y a `https://soroban-testnet.stellar.org` (RPC); no hay URLs de `localhost`.

## 3. Trustline con una cuenta nueva

**Criterio de aprobación:** la trustline aparece en el estado on-chain de la cuenta y el sistema detecta correctamente si existe.

| Paso | Resultado |
|---|---|
| Cuenta nueva (friendbot) | `GDMSOZPN7R4HKQOTDKA52HSB4AAMNNPJIEOFPMN43VPYZN6GQOJ3KOW7`: 10 000 XLM, 0 subentradas, sin USDC |
| Antes de la trustline | `balance` del SAC falla con `Error(Contract, #13)` "trustline entry is missing"; `getLedgerEntries` del RPC no devuelve ninguna entrada |
| `change_trust` USDC:`GBVIQ7QV…H47B5` (firmado con el Stellar CLI) | [`71981e00…`](https://stellar.expert/explorer/testnet/tx/71981e00b38b8e6738074184585a01156b7b397bd6ecc71a5710c9fa5e8fce7b), exitosa, ledger 5437699, comisión de 100 stroops |
| Después | 1 subentrada, USDC del emisor correcto, saldo 0, autorizada; `balance` del SAC devuelve `"0"`; el RPC devuelve la entrada |

**Resultado:** on-chain, aprobado. Al sistema le faltaba la detección (nada revisaba la trustline; un depósito directo fallaba recién en la simulación). Corregido en #37 y desplegado (`eba56dd`): el depósito y el retiro directos leen la trustline desde el RPC y bloquean el formulario con instrucciones cuando falta. **Pendiente:** firmar la trustline con Freighter y revisar el aviso a mano en la web en vivo.

## 4. Errores y recuperación

**Criterio de aprobación:** los errores se muestran correctamente, ninguna operación se duplica y ningún estado queda inconsistente.

| Escenario | Qué se revisó | Resultado |
|---|---|---|
| Firma rechazada | Tests de la web: rechazar en el registro o en un intent no llama a nada. Una operación del pool rechazada en Freighter devuelve `wallet request rejected by user` y no se reintenta | No se envió nada ✅; el mensaje salía en inglés crudo → corregido en #41 |
| Falta la trustline | Corregido en #37 y #39 | ✅ |
| Saldo insuficiente | No hay revisión previa. Saldo privado insuficiente: el SDK dice `transaction planning rejected: no combination possible`. USDC público insuficiente en un depósito directo: el SAC da `Error(Contract, #10) resulting balance is not within the allowed range` (simulado desde una cuenta con 0 USDC) | No se envió nada ✅; mensaje ilegible → corregido en #41 (traducido, y enviar/retirar deshabilitados por encima del saldo sincronizado) |
| Sesión vencida | API en vivo: `issued_at` con 10 min de antigüedad o 2 min adelantado → `400 request_expired`; firma sobre otro monto → `400 invalid_signature`; cuenta no registrada → `404 user_not_found`; ninguno creó un intent. La web firma en cada clic | ✅ (no se detecta un cambio de cuenta en Freighter con la página abierta: pendiente) |
| Transacción fallida | Tests del worker (CI): envío ambiguo, descartada, rechazada, fallida on-chain, red caótica, retraso del RPC, usuario bloqueado a mitad de camino; cada uno verifica exactamente un depósito y una transferencia | ✅; una falla parcial ocultaba en la web los pasos ya enviados → corregido en #41 |
| Reintentos | Worker: backoff y un tope, luego revisión manual. Web: solo reintenta un timeout de la prueba, una vez, cuando no se envió nada; los botones se deshabilitan mientras trabaja | ✅ |
| Sin duplicados on-chain | Todo el historial de la intake: 6 pagos del anchor, cada uno seguido de exactamente 1 depósito en el pool y 1 transferencia privada; ninguna transacción fallida; saldo de USDC 0 | ✅ |

**Resultado:** backend aprobado. La web quedó aprobada después de #41; las pantallas no se probaron con Freighter.

## 5. API, base de datos y regresión

**Criterio de aprobación:** los tests pasan, no hay errores nuevos y las revisiones on-chain se pueden repetir.

Corrido en `develop` en `9bb595d` (el commit desplegado).

| Suite | Resultado |
|---|---|
| `make test-api` contra PostgreSQL 17 (clúster local temporal, misma versión mayor que el CI) | 268 aprobados, 0 fallidos, 3 ignorados (los tests de testnet de abajo). Los 99 tests `#[sqlx::test]` corrieron contra él (sqlx creó su esquema `_sqlx_test`) |
| `make test-pool-cli` | 7/7 |
| `make test-js` | 133/133 (web, Vitest) + 2/2 (`api-types`) |
| `make test-web-e2e` | 12/12 (Playwright, Chromium) |
| `make fmt-check`, `make lint` (clippy `-D warnings` en el API y pool-cli, ESLint, OpenAPI), `make typecheck`, `make build-js` | Sin problemas |
| `make codegen` | Sin diferencias: los tipos TS coinciden con `openapi.yaml` |
| `make test-testnet` (en el servidor; necesita las llaves del operador y de la intake) | Aprobado en los 4 despliegues del día (`185004Z`, `195845Z`, `201106Z`, `202653Z`): las 7 revisiones de `scripts/verify-testnet.sh` y los 3 tests `#[ignore]` (`testnet_sandbox_starts_an_interactive_deposit`, `testnet_prepares_an_allow_list_insert_without_sending_it`, `testnet_the_intake_is_initialised`) |

**Resultado:** aprobado. La revisión *pool deployment ledger is inside the RPC retention window* va a empezar a fallar alrededor del 2026-11-02 (ver Hallazgos).

## 6. Reverificación en el pool redesplegado (2026-11-06)

El primer pool salió de la ventana de historial del RPC público el 2026-11-02 (hallazgo 2). Se redesplegó el 2026-11-06 en el ledger 5 591 375 ([CAB6FI43…KR4](https://stellar.expert/explorer/testnet/contract/CAB6FI43BVMPMAXYCLVZ7R5DYQY3IUJF3G7JMSBEXJHWO7LEUE3H4KR4)), con el bootnode de SPP sirviendo los eventos más viejos que esa ventana. Después se corrieron todos los flujos a mano desde la app web en vivo, con Freighter. Todas las transacciones salieron bien; en las tres de la rampa, la dirección del usuario aparece 0 veces (Horizon: transacciones, operaciones y efectos).

| Flujo | UTC | Transacción |
| --- | --- | --- |
| Rampa: anchor → ómnibus (9,3039215 USDC, memo del intent) | 19:08:31 | [7ce42090…](https://stellar.expert/explorer/testnet/tx/7ce4209056f81d311f7a55a2f25e2a99e4ae5cba30ef469e0dd8db5635cf810f) |
| Rampa: depósito de Shield en el pool | 19:11:11 | [2a8dd765…](https://stellar.expert/explorer/testnet/tx/2a8dd76594d81d566b67cceaa9c7b35b7cd087c952e7d28e6ccb000b2b795bda) |
| Rampa: transferencia privada a las llaves del usuario | 19:12:11 | [4e3c5c0a…](https://stellar.expert/explorer/testnet/tx/4e3c5c0a1fcfe09df21434252c2d586aea475dc719ec731ba6d38ef3c72c7d9d) |
| Directo: depósito desde la wallet del usuario | 19:01:31 | [e2f171da…](https://stellar.expert/explorer/testnet/tx/e2f171dad9a6f67b3bc0d407fa18beb7e001c5f78f7650d5454f1ab588568aec) |
| Directo: transferencia privada desde el navegador | 21:13:11 | [eee163a9…](https://stellar.expert/explorer/testnet/tx/eee163a927aa368798072306ab7d15dfa16c18f1bc318ca0d0962cccf2e6be2d) |
| Directo: retiro de 5 USDC a la cuenta del usuario | 21:28:01 | [1a8bb88e…](https://stellar.expert/explorer/testnet/tx/1a8bb88ecbef914ead7a43019e80bbaf154f23cb44f6162caa03df41f1dfc71b) |

Una rampa posterior, el mismo día, tardó unos 13 minutos desde el pago del anchor hasta la transferencia privada. La causa fue la latencia del RPC público, no Shield ni SPP: ver `docs/testing/2026-11-06-rpc-latency.md` en el repositorio.

## Hallazgos

1. **Límites de depósito que no coinciden.** El `MaximumDepositAmount` del pool es `10000000000` stroops (1000 USDC); el anchor acepta hasta 10 000 USDC y el API no revisa el límite del pool al crear un intent. Un intent de rampa de entre 1000 y 10 000 USDC lo pagaría el anchor, lo rechazaría el pool y terminaría en `error` con los USDC en la intake. Los montos de la demo (CRC 5000 ≈ 9,30 USDC) no se ven afectados. **Resuelto para la demo (2026-11-06, #62–#64):** la API publica el tope como `pool.max_deposit` en `/v1/info`, la página de depósito lo muestra antes de ir al anchor, y un pago por encima del tope se detiene de inmediato en `error` con un mensaje claro.
2. **Ventana de historial del RPC.** El pool se desplegó en el ledger `5395126`; el RPC público guarda 120 960 ledgers (el más antiguo era `5316399`). A ~5 s por ledger, el despliegue sale de la ventana alrededor del **2026-11-02**, y desde entonces las pruebas fallan en ambos modos. Opciones: correr `tools/bootnode` de SPP o redesplegar (ver el AGENTS.md del repositorio, sección 9). **Resuelto (2026-11-06):** el pool se redesplegó y el bootnode de SPP ahora sirve el historial más viejo (sección 6).
