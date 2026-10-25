# CLAUDE.md — Lendara Shield

> Contexto para Claude Code. Leer completo antes de cualquier tarea.
> Comunicación con Anouk: **en español**. Código, nombres, comentarios y mensajes de commit: **en inglés**.

---

## 0. Reglas de oro (no negociables)

### 0.1 Git: Anouk hace los commits y los push, NO Claude Code

- **NUNCA** ejecutes `git add`, `git commit`, `git push`, `git merge`, `git rebase`, `git reset`, `git stash`, `git tag`, `gh pr create` ni ningún comando que cree, modifique o publique historial o PRs.
- **Sí puedes** crear y cambiar de rama (`git switch -c <rama>`, `git switch <rama>`) para preparar cada PR.
- **Sí puedes** usar comandos de solo lectura: `git status`, `git diff`, `git log`, `git branch`, `git show`.
- **No puedes** hacer `fetch` ni `pull` (no tienes credenciales): pídeselo a Anouk.
- Si una tarea requiere otra operación de Git, **detente y dale a Anouk el comando exacto**.

**Formato de los comandos de commit para Anouk** (aprendido en el sandbox, no negociable):

- **Commits separados por tema** dentro del PR (p. ej. migración + dominio + endpoint + tests). Cada commit lleva su propio `git add` con **todos** los archivos de ese commit.
- Cada commit va en **una sola línea** para copiar y pegar de una vez: `git add <rutas> && git commit -m "tipo(scope): mensaje"`.
- **Rutas sin comillas.** Si una ruta tiene `(`, `)`, `[` o `]` (rutas de Next.js como `(app)` o `[id]`), agrega la **carpeta padre** en vez del archivo. Preferir carpetas a listas largas de archivos cuando la carpeta pertenece entera al commit.
- Antes de dar los comandos, corre `git status --porcelain` y verifica que la suma de los `git add` cubre **todos** los archivos cambiados (ni uno de más, ni uno de menos). Termina con `git status` para que Anouk confirme que no quedó nada.
- **Nunca** agregues `Co-Authored-By` ni ningún trailer de atribución en los mensajes de commit.

### 0.2 Trabajo en múltiples PRs pequeños

El historial de PRs se presenta en la hackathon y debe mostrar una progresión clara.

- Trabaja **un PR a la vez**, siguiendo el plan de la sección 11. No adelantes trabajo de PRs futuros.
- Cada PR es una unidad coherente y revisable: idealmente **menos de 400 líneas cambiadas**, sin contar código generado, lockfiles, migraciones generadas ni artefactos de circuitos.
- No mezcles temas: un PR de backend no toca la web; un PR de scaffold no implementa features.
- Si una tarea crece más de lo esperado, **propón dividirla antes de seguir**.

### 0.3 Protocolo al empezar una tarea

1. Confirma en qué PR del plan estamos (sección 11).
2. Resume en 3 a 6 líneas qué vas a hacer y qué archivos vas a tocar.
3. Si vas a agregar una dependencia relevante (librería, servicio, herramienta), **pregunta antes**.
4. Si el PR depende de una **decisión abierta** (sección 12), pregunta antes y espera el OK.

### 0.4 Protocolo al terminar una tarea

1. Corre formateo, lint, build y tests relevantes (sección 10) y reporta el resultado **real**. No digas que algo funciona si no lo verificaste.
2. Si el PR toca la red (pool, ASP, pagos), incluye los **hashes reales de testnet** de la prueba y su enlace a stellar.expert.
3. Entrega este bloque y **detente**:

```
## Resumen del PR
**PR del plan:** H5 - ASP onboarding
**Rama:** feat/api-asp-onboarding
**Archivos cambiados:** (lista)
**Migraciones:** migrations/00XX_nombre.sql (qué crea)
**Verificación:** fmt ✅ · clippy ✅ · test ✅ (N unit · N integration · N http) · cobertura dominio N% · build ✅
**Tests agregados:** (qué comportamiento cubre cada grupo, incluidos los casos de error)
**Prueba en testnet:** <hash> → https://stellar.expert/explorer/testnet/tx/<hash>
**Commits sugeridos** (Anouk los hace, separados por tema):
  1. `feat(api): add shield users and simulated KYC`
     archivos: ...
  2. `feat(api): add allow-list management through ASP contract`
     archivos: ...
**Título del PR (inglés):** feat(api): ASP onboarding and allow-list management
**Descripción del PR (inglés):**
  - What
  - Why
  - How to test (exact commands)
  - Out of scope / follow-ups
**Comandos para Anouk** (una línea por commit, rutas sin comillas):
  git add apps/api/migrations/0002_shield_users.sql apps/api/src/asp/ && git commit -m "feat(api): add shield users and simulated KYC"
  git add apps/api/src/http/onboarding.rs apps/api/tests/onboarding.rs && git commit -m "feat(api): add allow-list management through ASP contract"
  git status && git push -u origin feat/api-asp-onboarding
```

### 0.5 Seguridad

- **Nunca** escribas claves secretas de Stellar (`S...`), API keys reales, contraseñas, view keys ni claves de gasto de notas en archivos versionados.
- Todo secreto va en variables de entorno. Mantén `.env.example` actualizado con valores vacíos o falsos.
- `.env*` (excepto `.env.example`) en `.gitignore`. Agrega también `.storage/`, `artifacts/` y los proving keys descargados.
- **Solo testnet.** Nunca uses mainnet ni su network passphrase.
- **Separación de funciones:** la cuenta de administración del ASP, la cuenta de ingreso (intake) y la llave de auditoría son **tres llaves distintas**. Nunca reutilices una para otra cosa.
- **Shield nunca guarda las claves de gasto de los usuarios.** Las llaves de la piscina se derivan en la billetera del usuario (navegador). Shield solo conoce la llave pública registrada.
- **Higiene de privacidad en logs:** los logs nunca incluyen view keys, notas, secretos de notas ni el par "dirección del usuario + monto". Usa el `id` del intent. La base de datos puede guardar esa relación (Shield es el ASP), los logs no.

### 0.6 Integridad del dinero

- Los workers son **idempotentes**: antes de enviar una transacción, guarda su hash y su estado; al reintentar, consulta la red antes de volver a enviar. Un depósito al pool nunca puede ocurrir dos veces para el mismo intent.
- Montos en **stroops (`i128` / `i64`) o string decimal**, nunca `f64`.
- Toda transición de estado pasa por un único módulo y queda registrada (sección 7.4).

---

## 1. Qué es Lendara Shield

**Lendara Shield es la implementación de referencia del Shield Protocol**: una capa de privacidad con cumplimiento para Stellar, pensada para conectarse con rampas (anchors) y llevar el dinero de la gente directamente a un pool privado.

**El problema:** Stellar ya tiene herramientas de privacidad en developer preview (Stellar Private Payments de Nethermind y Confidential Tokens de OpenZeppelin), pero la privacidad se rompe en el borde fiat. El depósito en una rampa queda atado a una dirección, un monto y una hora. Además, un privacy pool con allow-list necesita a alguien que haga KYC, administre la lista (ASP), guarde la llave de auditoría y pague comisiones. Hoy cada app tendría que resolverlo sola.

**La idea:** un anchor ya hace KYC y ya opera una cuenta ómnibus. Si el anchor (o Shield, en su nombre) deposita en el pool desde su cuenta ómnibus y luego transfiere en privado a la llave del usuario, **la dirección del usuario nunca aparece on-chain**.

**Frase de producto:** *KYC once, pay privately, disclose selectively.*

**Equipo:** Anouk (Costa Rica, backend), Karu y Elisa (Argentina). Las tres son developers.

**Relación con otros repos:**

| Repo | Qué es | Relación |
|---|---|---|
| `Lendara-Org/Lendara-monorepo` | Sandbox de la rampa (SEP-1/10/24, Go) | Shield lo **consume** como cualquier wallet SEP-24 en el modo ramp. **No se modifica desde este repo.** |
| `Lendara-Org/lendara-shield` (este) | Shield Protocol + implementación | Proyecto nuevo e independiente. Remoto: `https://github.com/Lendara-Org/lendara-shield.git`. Carpeta local: `Bitacora/shield-lendara` |

**Contexto inmediato:**
- **HackMeridian 2026 (Lisboa), track Genesis.** Entrega el **lunes 26 de octubre de 2026** (confirmar la hora exacta con Anouk). Se entrega este repo.
- **Meridian 2026**, 28 y 29 de octubre: demos y entrevistas con otros equipos.
- Después: Instawards y SCF.

### 1.1 Mini glosario (para leer el resto del documento)

| Término | Significado en este repo |
|---|---|
| Pool / SPP | Privacy pool de Stellar Private Payments (Nethermind). Contrato Soroban con notas, commitments, nullifiers y pruebas Groth16 |
| Nota | Valor privado dentro del pool que solo su dueña puede gastar |
| ext_amount | Monto externo de una operación del pool: positivo al depositar, **0 en una transferencia privada**, negativo al retirar |
| ASP | Association Set Provider. Mantiene la allow-list y la block-list de llaves del pool. **Shield es el ASP** |
| Intake account | Cuenta ómnibus de Shield que recibe USDC del anchor y deposita en el pool |
| View key global | Llave de auditoría del pool. La guarda Shield, nunca se expone en la web pública |
| Intent | Una operación de Shield en el modo ramp (de fiat a saldo privado) |

---

## 2. Dos modos, un solo núcleo

Shield se usa de dos maneras. **No son dos productos ni dos códigos**: comparten el núcleo (ASP, pool, auditoría, máquina de estados) y difieren solo en **cómo entra el dinero**.

| | **Modo `ramp`** (con rampa) | **Modo `direct`** (sin rampa) |
|---|---|---|
| Para quién | Usuarios que llegan con moneda local | Usuarios que ya tienen USDC en su wallet |
| Cómo entra el dinero | Fiat → anchor SEP-24 → intake de Shield → pool → transferencia privada al usuario | El usuario deposita desde su propia wallet al pool |
| ¿Aparece la dirección del usuario al entrar? | **No** (entra la cuenta ómnibus de Shield) | **Sí** (trade-off documentado en la UI y en la docs) |
| Depende del sandbox | Sí (o de cualquier anchor SEP-24) | No |
| Qué hace Shield | KYC/ASP, recibe, deposita, transfiere, audita | KYC/ASP, audita (y relayer más adelante) |

**Reglas de diseño:**

- **El modo `ramp` es opcional y se activa por configuración.** Si `RAMP_ANCHOR_HOME_DOMAIN` no está definido, el modo ramp queda deshabilitado: los endpoints de intents responden `409 {"error":"ramp_mode_disabled"}` y la web oculta la opción. El modo `direct` funciona siempre.
- **El modo `ramp` habla con el anchor solo por estándares (SEP-1, SEP-10, SEP-24).** Shield actúa como un *cliente SEP-24*: se autentica con su intake account (SEP-10 con memo) y pide un depósito interactivo con `account = intake` y `memo = intent id`. Así funciona con el sandbox de Lendara **y con cualquier anchor SEP-24**, sin cambiar el sandbox. Esto es central para el pitch del protocolo.
- **Ambos modos comparten** el registro de usuarios, el ASP, el cliente del pool, el auditor y la máquina de estados. No dupliques lógica.

### 2.1 Flujo del modo `ramp` (de punta a punta)

```
1. Usuario conecta su wallet en la web de Shield y deriva sus llaves del pool.
   La web le manda a Shield sus llaves públicas (nota + cifrado) y su hoja ASP
   (deriveAspUserLeaf). NO se registra en el registry on-chain (expondría su G).
2. KYC simulado en Shield → si se aprueba, el ASP (operator) inserta su hoja en la allow-list.
3. POST /v1/intents {mode:"ramp", fiat_currency, amount}
   → Shield hace SEP-10 contra el anchor (intake account + memo id = id numérico del usuario)
   → Shield pide POST /sep24/transactions/deposit/interactive
       {asset_code:"USDC", account:<intake>, memo:<memo numérico del intent>, memo_type:"id"}
   → devuelve la URL interactiva del anchor.
4. El usuario completa el widget del anchor (en el sandbox: "Simular pago").
5. El anchor paga USDC a la intake account con el memo del intent.
6. El watcher de Shield detecta el pago (Horizon), lo empareja por memo
   y verifica asset, issuer y monto.
7. El worker deposita en el pool desde la intake account (ext_amount > 0).
8. El worker transfiere en privado a las llaves del usuario guardadas en Shield (ext_amount = 0).
9. Intent → completed. El usuario ve su saldo privado en la web.
```

Hallazgo para el pitch: en el explorador solo aparecen "el anchor pagó a Shield" y "Shield depositó en el pool". **La dirección del usuario no aparece en ningún paso.**

**Límite honesto (va en el threat model y en el pitch):** el anchor conoce al usuario y el monto (es quien hace KYC). Para un observador externo, el pago del anchor y el depósito al pool tienen el mismo monto y horas casi iguales, y en testnet el conjunto de anonimato es chico, así que la entrada se podría correlacionar. La dirección del usuario sigue sin aparecer, pero la mitigación real (depósitos agrupados, montos redondeados, demora aleatoria) es P4. Decirlo antes de que lo pregunte el jurado.

### 2.2 Flujo del modo `direct`

```
1. Igual que los pasos 1 y 2 del modo ramp (llaves + KYC + allow-list).
2. El usuario deposita desde su wallet al pool con el SDK web (ext_amount > 0, su dirección visible).
3. Transfiere en privado, consulta su saldo y retira desde la web.
```

---

## 3. El Shield Protocol (qué estandarizamos)

El objetivo a mediano plazo es que **cualquier anchor** pueda ofrecer depósitos privados. El protocolo tiene dos niveles:

| Nivel | Qué es | Estado |
|---|---|---|
| **Nivel 1: Shield como middleware** | Shield se interpone entre cualquier anchor SEP-24 y el pool (modo ramp, sección 2.1) | Se implementa en la hackathon |
| **Nivel 2: anchor nativo** | El anchor deposita directo en el pool a nombre de una llave privada. Extensión de SEP-24 | Borrador de especificación (PR P1) |

Borrador del Nivel 2 (a escribir en `docs/protocol/shield-protocol.md`, **no implementar en la hackathon**):

- `stellar.toml` del anchor anuncia `SHIELD_POOL_CONTRACT`, `SHIELD_ASP_ENDPOINT` y `SHIELD_PROTOCOL_VERSION`.
- `GET /sep24/info` agrega `features.shielded_deposit: true`.
- `POST /sep24/transactions/deposit/interactive` acepta `destination_type = "shielded"` y `shielded_recipient` (llave pública registrada) en lugar de `account`.
- El anchor actúa como ASP: aprueba llaves al completar el KYC.
- Retiro: el usuario retira del pool hacia la cuenta del anchor con prueba de pertenencia a la allow-list.

Nombre del estándar: **Shield Protocol** (sin "Lendara"), para que otros anchors lo adopten sin adoptar la marca de un competidor. El producto y el repo sí llevan "Lendara".

---

## 4. Arquitectura del monorepo

```
lendara-shield/
├── apps/
│   ├── api/            # Rust (Axum): ASP, intents, cliente SEP-10/24, watcher, worker, auditor
│   └── web/            # Next.js: onboarding, depósito, wallet privada, panel de auditoría
├── packages/
│   └── api-types/      # Tipos TS generados desde openapi.yaml
├── docs/
│   ├── protocol/       # Shield Protocol (borrador de especificación)
│   ├── spikes/         # Resultados de verificaciones técnicas (p. ej. spp.md)
│   ├── threat-model.md
│   └── scenarios.md    # Escenarios del sandbox para Shield
├── openapi.yaml        # Contrato de la API: FUENTE DE VERDAD
├── docker-compose.yml  # Postgres local
├── Makefile            # dev, build, test, lint, codegen, db-up, migrate, setup-testnet
├── .env.example
├── .github/
│   ├── workflows/ci.yml
│   └── pull_request_template.md
└── README.md           # Incluye "demo en 5 minutos"
```

Decisiones clave:

- **`apps/api` no es parte del workspace de pnpm.** El workspace incluye `apps/web` y `packages/*`. El `Makefile` raíz orquesta todo.
- **`openapi.yaml` es el contrato** y se escribe ANTES de implementar los endpoints. De él se generan los tipos TS (`openapi-typescript`). Del lado Rust, los tipos se escriben a mano y un **test de contrato** valida las respuestas contra el schema (post-hackathon; ver P11).
- **Los contratos del pool no se copian a este repo.** Se usan los de Nethermind tal como están (despliegue por script o los ya desplegados en testnet). Si hace falta fijar una versión, se documenta el commit en `docs/spikes/spp.md`.
- **El sandbox no se modifica desde aquí.** Si algo del sandbox bloquea, se reporta a Anouk como PR para el otro repo.

---

## 5. Stack

### 5.1 Backend (`apps/api`) — Rust

> **Decisión D1 (sección 12): confirmar en el spike H1.** La recomendación es Rust porque el SDK nativo de Stellar Private Payments es Rust y Anouk trabaja con Rust/Axum. Si el spike muestra que el SDK nativo no sirve del lado servidor, el plan B es Go (mismos patrones que el sandbox) + un sidecar Node con el SDK web.

- **Rust estable** (fijar en `rust-toolchain.toml`).
- **HTTP:** `axum` + `tower-http` (CORS, trace, timeout, request id).
- **Async:** `tokio`.
- **Base de datos:** PostgreSQL + `sqlx` (consultas verificadas en compilación, modo offline con `cargo sqlx prepare` para CI). Migraciones con `sqlx migrate`.
- **Errores:** `thiserror` en dominio, conversión centralizada a respuestas HTTP `{ "error": "<code>", "message": "..." }`.
- **Logs:** `tracing` + `tracing-subscriber` (JSON en producción). Respeta la higiene de 0.5.
- **Config:** variables de entorno cargadas en un struct al iniciar; fallar rápido si falta algo obligatorio.
- **Stellar:**
  - Pool: SDK nativo de Stellar Private Payments (verificar crate, versión y API en el spike).
  - XDR, firma y strkeys: crates oficiales `stellar-xdr` y `stellar-strkey`; `ed25519-dalek` para firmar. **Verifica los nombres y versiones vigentes antes de agregarlos** y repórtalos a Anouk.
  - Envío de transacciones Soroban: Stellar RPC testnet. Lectura de pagos clásicos: Horizon REST (`reqwest`).
  - SEP-10 cliente: leer el challenge, validar que lo firmó el `SIGNING_KEY` del `stellar.toml` del anchor, firmar y enviarlo. **Consulta SEP-10 para cada validación; no inventes comportamiento.**
- **Testnet:** Horizon `https://horizon-testnet.stellar.org`, RPC `https://soroban-testnet.stellar.org`, passphrase `Test SDF Network ; September 2015`.

### 5.2 Frontend (`apps/web`)

- **Next.js** (App Router) + **TypeScript estricto** + **Tailwind CSS**.
- **Wallet:** Freighter (el SDK web del pool trae `FreighterSigner`).
- **Pool en el navegador:** paquete npm `stellar-private-payments` (alpha). Prueba y deriva llaves en el cliente; **las llaves de gasto nunca salen del navegador**.
- **Rampa:** para abrir el widget del anchor se usa la URL interactiva que devuelve `POST /v1/intents`. Si `@lendara/widget` (el loader del sandbox) sirve tal cual, se reutiliza; si no, se abre en popup.

### 5.3 Herramientas

- **pnpm workspaces**, **Docker Compose** (Postgres 16+), **Makefile** raíz: `make dev`, `make build`, `make test`, `make lint`, `make fmt`, `make codegen`, `make db-up`, `make db-down`, `make migrate`, `make setup-testnet`, `make coverage`, `make test-testnet`.
- **CI (GitHub Actions):** fmt, clippy, test (con servicio Postgres), `cargo sqlx prepare --check`, `cargo deny check`, build del API; lint, typecheck, test y build de la web; verificación de que el código generado está al día; **gitleaks** con regla para semillas Stellar `S...`. Caché de cargo y pnpm para que el CI tarde pocos minutos.
- Pre-commit opcional con los mismos chequeos.

---

## 6. Especificaciones de referencia (consultarlas, no inventar)

- Stellar Private Payments: anuncio y docs en https://stellar.org/blog/developers/developer-preview-stellar-private-payments y https://developers.stellar.org/docs/build/apps/privacy
- Confidential Tokens (segundo carril, post-hackathon): https://stellar.org/blog/developers/developer-preview-confidential-tokens-on-stellar
- SEP-1: https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0001.md
- SEP-10: https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0010.md
- SEP-24: https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0024.md
- Privacy Pools (Buterin et al.): https://privacypools.com/whitepaper.pdf

Datos conocidos de SPP (del anuncio oficial; **verificar contra el repo en el spike**):

- Pruebas Groth16 verificadas on-chain con los host functions de Protocol 25 (X-Ray) y 26 (Yardstick).
- ASP por llave, configurable por pool: block-list, allow-list o ambos. Cada operación prueba pertenencia o no pertenencia.
- View key global para un auditor y divulgación selectiva por nota.
- Contratos en testnet: pool, verificador Groth16, ASP de pertenencia y no pertenencia, registro de llaves públicas. Pools existentes de XLM y EURC (la demo hospedada corre solo con block-list).
- SDK web en npm (`stellar-private-payments@alpha`), SDK nativo en Rust y CLI `spp`. El SDK sincroniza el historial con un indexer ("bootnode") para no depender de la ventana de 7 días de eventos del RPC.
- **Developer preview, contratos sin auditar. Solo testnet.**

---

## 7. Diseño funcional

### 7.1 Cuentas y activo en testnet

| Cuenta | Para qué | Variable |
|---|---|---|
| Operator | Despliega y administra el pool; administra el ASP | `SHIELD_OPERATOR_SECRET` |
| Intake | Cuenta ómnibus: recibe USDC del anchor, deposita en el pool y transfiere en privado | `SHIELD_INTAKE_SECRET` |
| Auditor | View key global (GVK): llave **Baby JubJub**, no de Stellar. Se genera con `tools/gvkey-gen` de SPP; su pública se fija al desplegar el pool | `SHIELD_GVK_KEY_FILE` (ruta al JSON `{privateKey, publicKey}`, fuera del repo) |

- **Activo:** el USDC de prueba del sandbox de Lendara (mismo issuer, `RAMP_ASSET_ISSUER`), para que el modo ramp funcione de punta a punta. La intake necesita trustline a ese activo.
- **SAC:** el USDC del sandbox es un activo clásico; el pool necesita un token SEP-41. `setup-testnet` despliega (o reutiliza si ya existe) su Stellar Asset Contract y guarda el contract id en `SHIELD_ASSET_CONTRACT`.
- **Llaves del pool de la intake:** la intake es dueña de la nota intermedia (depósito) antes de transferirla al usuario. El SDK las deriva en el servidor firmando con `SHIELD_INTAKE_SECRET` y las guarda en su wallet SQLite (`SHIELD_INTAKE_WALLET_PATH`, fuera del repo). Verificado en H1.
- **Script `make setup-testnet`** (`apps/api/src/bin/setup_testnet.rs`): crea y fondea las cuentas con Friendbot, crea la trustline de la intake, despliega o configura el pool (sección 7.2) e imprime las variables para `.env`. **Idempotente.**

### 7.2 Configuración del pool

- **Decidido en H1: plan A.** Configuración: `allowlist-blocklist` + `gvk-traceable`, contratos del tag `sdk-v0.2.1` de SPP con su `deploy.sh`. **La allow-list solo permite agregar** (`insert_leaf`); bloquear = insertar en la block-list (`asp-non-membership`, que permite `delete_leaf`).
- **Plan A:** desplegar un pool propio de SPP para el USDC del sandbox, en modo **allow-list + block-list**, con Shield (operator) como ASP. Esto permite demostrar el portero.
- **Plan B:** usar un pool existente de testnet (XLM o EURC). Se pierde el control del ASP (la demo hospedada usa solo block-list): el rechazo de KYC se demuestra en Shield (no se deposita por esa persona) y se explica que en producción sería el ASP del pool.
- La decisión se toma en el spike H1 y se registra en `docs/spikes/spp.md` y en la sección 13.

### 7.3 Modelo de datos (borrador)

- `shield_users` (id, stellar_account, sep10_memo `bigint` UNIQUE, note_public_key, encryption_public_key, asp_leaf, kyc_status, asp_status, kyc_scenario, created_at, updated_at)
- `asp_events` (id, user_id, action `allow|revoke|block|unblock`, tx_hash, created_at)
- `intents` (id, mode `ramp`, user_id, status, fiat_currency, amount_fiat, asset_code, amount_stroops, anchor_home_domain, anchor_tx_id, memo `bigint` UNIQUE (memo `id` de SEP-24), intake_payment_tx_hash, pool_deposit_tx_hash, pool_transfer_tx_hash, next_attempt_at, attempts, message, created_at, updated_at, completed_at)
- `intent_events` (id, intent_id, from_status, to_status, created_at)

Estados y enums como `text` + `CHECK`. Montos en `bigint` (stroops) o `numeric` leído como string.

### 7.4 Máquina de estados del intent (modo ramp)

```
created ──(SEP-24 interactivo creado)──► awaiting_anchor
awaiting_anchor ──(pago del anchor detectado y válido)──► funds_received
funds_received ──(depósito al pool enviado)──► shielding
shielding ──(depósito confirmado)──► transferring
transferring ──(transferencia privada confirmada)──► completed

Salidas de error:
  created ──► rejected        (la llave del usuario no está en la allow-list)
  awaiting_anchor ──► expired (el anchor marcó la transacción como expirada o pasó el TTL)
  awaiting_anchor ──► error   (el anchor terminó en error, p. ej. KYC rechazado en el sandbox)
  funds_received | shielding | transferring ──► error (con `message`; requiere revisión)
```

- Implementada en **un solo módulo** (`apps/api/src/intent/state.rs`), con transiciones explícitas y validadas. Una transición inválida es un error, no un no-op.
- Toda transición registra un `intent_events`.
- Tests unitarios de todas las transiciones válidas e inválidas.
- **Idempotencia:** se guarda el hash de cada transacción antes de enviarla; al reintentar se consulta su estado en la red. El SDK lo permite: `prepare_*` → `prove_next` → `sign` (da el hash) → guardar → `submit` → `confirm(hash)`. No usar `deposit()`/`transfer()` de un solo paso en el worker.
- Para seguir el estado del anchor se consulta `GET /sep24/transaction?id=...` (el sandbox no tiene webhooks).

### 7.5 KYC simulado y ASP

- Onboarding: el usuario conecta Freighter, deriva sus llaves del pool y llama a `POST /v1/onboarding` con su cuenta, sus llaves públicas y su hoja ASP. Shield valida y, si el KYC aprueba, el operator invoca `insert_leaf` en el contrato ASP. **El SDK no trae funciones de admin del ASP:** H5 implementa un adaptador `AspContract` (invocación Soroban firmada por el operator).
- **Escenarios de KYC de Shield** (para el modo direct y para la demo), elegidos con un campo `scenario` en el onboarding (solo sandbox):

| Escenario | Resultado |
|---|---|
| `approved` (por defecto) | KYC aprobado → llave en la allow-list |
| `rejected` | KYC rechazado → la llave no entra; los depósitos fallan |
| `blocked` | Aprobado y luego bloqueado → demuestra el congelamiento por llave |

- **Modo ramp:** se suman los escenarios del sandbox por monto mágico. `.13` (KYC rechazado en el anchor) → el intent pasa a `error` y Shield no deposita. `.99` → `expired`. `.50` → demora en el anchor (prueba el polling). Documentar todo en `docs/scenarios.md`.
- Las reglas viven en un solo módulo (`apps/api/src/asp/`).

### 7.6 Auditoría

- `GET /v1/audit/transactions` (protegido con `AUDITOR_TOKEN`): con la view key global, devuelve las operaciones del pool descifradas (tipo, monto, llaves involucradas, hash).
- `GET /v1/audit/public/{tx_hash}`: lo que ve cualquiera en la red para esa transacción (para la comparación lado a lado).
- La view key **nunca** se envía al navegador. La web consulta al API.

### 7.7 Relayer

- Fuera del MVP. Diseño previsto: transacciones **fee-bump** de Stellar, donde Shield paga la comisión de operaciones firmadas por el usuario. Verificar compatibilidad con las transacciones del pool antes de implementarlo (PR P3).

### 7.8 API (borrador; se define en `openapi.yaml`)

| Endpoint | Auth | Qué hace |
|---|---|---|
| `GET /health` | — | Salud del servicio y de la base de datos |
| `GET /v1/info` | — | Red, contrato del pool, activo, modo del ASP, modos habilitados (`ramp`, `direct`) |
| `POST /v1/onboarding` | — (hackathon) | Registra usuario + KYC simulado + allow-list |
| `GET /v1/users/{account}` | — (hackathon) | Estado de KYC y del ASP |
| `POST /v1/intents` | — (hackathon) | Crea un intent en modo ramp y devuelve la URL interactiva del anchor |
| `GET /v1/intents/{id}` | — (hackathon) | Estado del intent con sus hashes |
| `POST /v1/asp/block` | `ADMIN_TOKEN` | Bloquea una llave |
| `GET /v1/audit/transactions` | `AUDITOR_TOKEN` | Vista de la auditora |
| `GET /v1/audit/public/{tx_hash}` | — | Vista pública de una transacción |

Autenticación de integradores con API keys hasheadas (como en el sandbox) queda para el PR P2. En la hackathon, los endpoints de admin y auditoría usan tokens de entorno.

### 7.9 Web (`apps/web`)

| Ruta | Qué muestra |
|---|---|
| `/` | Qué es Shield, los dos modos y botones a la demo |
| `/onboard` | Conectar Freighter, derivar llaves, registrar, KYC simulado con selector de escenario |
| `/deposit` | Elegir modo: **Con rampa** (moneda local, abre el widget del anchor, progreso en vivo del intent) o **Desde mi wallet** (depósito directo con aviso de que la dirección es visible) |
| `/wallet` | Saldo privado, transferencia privada a otra dirección registrada, retiro |
| `/audit` | Comparación lado a lado: lo que ve el explorador frente a lo que ve la auditora |

Polling del intent cada 2 a 3 segundos hasta un estado final. Enlaces a stellar.expert en cada hash, con el `ext_amount` resaltado.

---

## 8. Validación

- **Explorador:** stellar.expert en testnet. El criterio de éxito del demo es ver el depósito con `ext_amount > 0` desde la intake y la transferencia con `ext_amount = 0`, sin la dirección del usuario.
- **Prueba E2E manual del modo ramp:** onboarding `approved` → intent con monto `.00` → `completed` con los tres hashes.
- **Prueba del portero:** onboarding `rejected` → el depósito falla; intent con monto `.13` → `error` sin depósito al pool.
- **Modo direct:** onboarding `approved` → depósito desde Freighter → transferencia → retiro.

---

## 9. Entregables de la hackathon

1. Repo público con historial de PRs limpio.
2. Demo funcionando en testnet (local o desplegado) con los dos modos.
3. README con: qué es, el problema, los dos modos, arquitectura, cómo correr local, "demo en 5 minutos", escenarios, hashes de prueba, equipo, y la nota de que todo es testnet con contratos sin auditar.
4. Borrador del Shield Protocol en `docs/protocol/`.
5. Video de 2 a 3 minutos con la historia de María (el equipo lo graba).

---

## 10. Convenciones y calidad

### Git (lo ejecuta Anouk)

- **Conventional Commits** con scope: `api`, `web`, `types`, `protocol`, `repo`, `docs`. Ejemplo: `feat(api): add ramp intents state machine`.
- **Ramas:** `<tipo>/<scope>-<descripcion-corta>`, p. ej. `feat/api-asp-onboarding`, `spike/spp-verification`.
- **Flujo:** features → `develop` (squash); `develop` → `main` solo para releases (merge commit). Default branch en GitHub: `develop`. Aprobaciones requeridas: 0 durante la hackathon.
- Plantilla de PR en `.github/pull_request_template.md` (Qué, Por qué, Cómo probar, Fuera de alcance, Hashes de testnet).

### Rust

**Lint y estilo**

- `cargo fmt --check` y `cargo clippy --all-targets --all-features -- -D warnings` obligatorios.
- En `lib.rs`/`main.rs`: `#![forbid(unsafe_code)]`. Lints del crate en `Cargo.toml` (`[lints.clippy]`): `pedantic` como `warn` (con `module_name_repetitions` y `missing_errors_doc` permitidos), más `unwrap_used`, `expect_used`, `panic`, `dbg_macro`, `todo` y `print_stdout` como `deny` fuera de tests.
- Sin `unwrap()`/`expect()`/`panic!` fuera de tests y del arranque (`main` puede fallar rápido con contexto).
- Sin `as` para convertir montos ni enteros con signo: usar `TryFrom` y devolver error.

**Arquitectura (puertos y adaptadores)**

```
apps/api/src/
├── main.rs          # solo arranque: config, tracing, pool de DB, router, workers, shutdown
├── lib.rs           # expone build_app(state) para que los tests levanten el router
├── config.rs        # struct Config desde env; valida todo al iniciar
├── domain/          # tipos puros: Stroops, AccountId, PoolPublicKey, IntentId, Memo...
├── intent/          # state.rs (máquina de estados pura) + service.rs (orquesta)
├── asp/             # reglas de KYC/allow-list puras + service
├── ports/           # traits: PoolClient, AnchorClient, HorizonClient, AspContract, Clock
├── adapters/        # implementaciones reales de los traits (red) 
├── db/              # repositorios sqlx (una función por consulta)
├── http/            # handlers Axum finos: parsean, llaman al service, mapean errores
└── worker/          # watcher y shielding worker
```

- **El dominio no importa** `sqlx`, `reqwest`, `axum` ni ningún SDK. Los services reciben los puertos por inyección (`Arc<dyn Trait>` o genéricos), así los tests usan dobles en memoria.
- **Handlers finos:** sin lógica de negocio en `http/`.
- **Newtypes con validación en el constructor** (`Stroops(i64)` no negativo, `AccountId` validado con `stellar-strkey`, `IntentId(Uuid)`). Si existe, el valor es válido ("parse, don't validate").
- **Reloj inyectado** (`trait Clock`): nada de `Utc::now()` en el dominio, para poder testear TTL y reintentos sin esperar.
- **Errores:** un `enum` por módulo con `thiserror`; un único `ApiError` que mapea a HTTP con códigos estables (`{ "error": "<snake_case_code>", "message": "..." }`). Los códigos se documentan en `openapi.yaml`.
- **Secretos** con `secrecy::SecretString` (no se imprimen en `Debug` ni en logs).
- **Red saliente:** todo cliente HTTP con timeout explícito; reintentos con backoff y tope solo en operaciones idempotentes.
- **Workers:** toman trabajo con `SELECT ... FOR UPDATE SKIP LOCKED` y `next_attempt_at`, para que dos instancias nunca procesen el mismo intent. Apagado ordenado (graceful shutdown) con `tokio::signal` y `CancellationToken`.
- **Base de datos:** una transacción de DB por transición de estado (cambio de `intents` + inserción en `intent_events` juntos). Índices para cada consulta del worker. Restricciones en la DB (`CHECK`, `UNIQUE`, `FOREIGN KEY`) además de la validación en Rust; p. ej. `UNIQUE (memo)` en `intents` y `UNIQUE (intake_payment_tx_hash)`.
- **Documentación:** `///` en todo lo `pub` del dominio y de los puertos; comentarios solo para explicar el *porqué*.

### TypeScript

- `strict: true`, `noUncheckedIndexedAccess: true`, `exactOptionalPropertyTypes: true`. Sin `any` ni `as` salvo justificado en un comentario.
- ESLint (config de Next + `@typescript-eslint/strict-type-checked`) + Prettier.
- Respuestas del API validadas con los tipos generados de `packages/api-types`; nunca tipos a mano para la API.
- Componentes de servidor por defecto; `"use client"` solo donde hay wallet, SDK del pool o estado interactivo.
- La lógica (derivar llaves, armar operaciones del pool, polling) va en hooks/módulos testeables, no dentro de los componentes.
- Accesibilidad básica: labels en los formularios, estados de carga y error visibles, navegable con teclado.

### 10.1 Estrategia de pruebas

**Regla:** ningún PR de código entra sin tests de lo que agrega, incluidos los **caminos de error**. La máquina de estados, las reglas del ASP, el emparejamiento por memo y la validación del challenge SEP-10 se escriben **test primero (TDD)**.

**Backend (Rust)**

| Nivel | Qué cubre | Cómo | Dónde |
|---|---|---|---|
| Unitarios | Dominio puro: transiciones válidas e inválidas, reglas del ASP, newtypes, parseo de montos, emparejamiento por memo, validación SEP-10 | `#[test]` / `#[tokio::test]`, sin red ni DB | `mod tests` en el mismo archivo |
| Propiedades | Invariantes: un monto nunca se vuelve negativo ni pierde precisión; ninguna secuencia de eventos salta estados; un intent nunca deposita dos veces | `proptest` | `mod tests` |
| Integración DB | Repositorios, constraints, transacciones, `SKIP LOCKED` con dos workers concurrentes | `#[sqlx::test]` (base aislada por test, con migraciones) | `apps/api/tests/` |
| HTTP | Cada endpoint: éxito, validación (400), no encontrado (404), auth (401/403), `ramp_mode_disabled` (409) | `build_app()` + `tower::ServiceExt::oneshot` con dobles de los puertos | `apps/api/tests/http_*.rs` |
| Adaptadores | Clientes de Horizon, RPC y anchor (SEP-1/10/24) contra respuestas grabadas: éxito, 4xx, 5xx, timeout, JSON inválido | `wiremock` con fixtures en `apps/api/tests/fixtures/` | `apps/api/tests/adapters_*.rs` |
| Worker | Idempotencia: caída después de enviar y antes de guardar; reintento consulta la red y no reenvía; dos workers simultáneos | Dobles en memoria de `PoolClient` con fallas inyectadas + `#[sqlx::test]` | `apps/api/tests/worker_*.rs` |
| Testnet (E2E) | Flujo real: onboarding → intent → `completed` con hashes | `#[ignore]`, se corre con `make test-testnet` (necesita `.env`) | `apps/api/tests/testnet_*.rs` |

- Fixtures de testnet **sin secretos**: solo cuentas públicas y hashes.
- Nada de `sleep` en los tests: usar el `Clock` falso y `tokio::time::pause()`.
- Nombres de test que describen comportamiento: `rejects_transition_from_completed_to_shielding`.
- **Cobertura** con `cargo llvm-cov` (`make coverage`; excluye `main.rs`, que solo conecta piezas): meta **≥ 90 %** en `domain/`, `intent/` y `asp/`; ≥ 75 % en total. Se reporta en cada PR; no se persigue el número con tests vacíos.

**Frontend (web)**

| Nivel | Qué cubre | Cómo |
|---|---|---|
| Unitarios | Hooks y módulos de lógica (polling, formateo de montos, mapeo de estados del intent) | Vitest |
| Componentes | Formularios y estados de carga, error y éxito; selector de escenario; aviso del modo direct | Vitest + Testing Library + `user-event` |
| Mock del API | Respuestas tipadas desde `packages/api-types`, incluidos errores | MSW |
| E2E smoke | `/` → `/onboard` → `/deposit` con el API y la wallet simulados | Playwright (un smoke en H7; la suite completa en P11) |

- Freighter y el SDK del pool se envuelven en un módulo propio con interfaz, para poder simularlos en los tests.

**CI** corre todo menos los tests `#[ignore]` de testnet, más `cargo deny check` (licencias y advisories) y `pnpm audit --prod`.

### Verificación antes de entregar un PR

- API: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo llvm-cov` (si cambió dominio), `cargo build`.
- Web: `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build` en los paquetes tocados.
- Si `openapi.yaml` cambió: `make codegen` y confirmar que el código generado está al día.
- Si el PR toca la red: correr el test de testnet correspondiente y pegar los hashes.

**Dependencias de prueba y calidad previstas** (aprobadas por Anouk al definir este documento; verifica versiones vigentes y repórtalas al agregarlas): `proptest`, `wiremock`, `sqlx` con feature `migrate`, `tower` (`ServiceExt`), `secrecy`, `tokio-util` (`CancellationToken`), `cargo-llvm-cov`, `cargo-deny`; en la web `vitest`, `@testing-library/react`, `@testing-library/user-event`, `msw`, `@playwright/test`. Cualquier otra dependencia: preguntar antes.

---

## 11. Plan de PRs

Cada PR deja el repo funcionando. Marcar ✅ al completar.

### Fase H: hackathon (domingo 25 noche y lunes 26)

| # | PR | Rama | Contenido | Criterio de aceptación |
|---|---|---|---|---|
| H0 ✅ | Scaffold | `chore/repo-scaffold` | Estructura, pnpm workspace, `Makefile`, `docker-compose.yml`, `.gitignore`, `.editorconfig`, `.env.example`, plantilla de PR, CI básico con gitleaks, README esqueleto | `make db-up` levanta Postgres; `pnpm install` funciona; CI en verde |
| H1 ✅ | Spike de SPP | `spike/spp-verification` | **Timebox: 90 minutos.** Verificar en el repo de Nethermind: (a) si se puede desplegar un pool propio para un activo SEP-41 arbitrario con allow-list y admin propio; (b) si el SDK nativo de Rust funciona del lado servidor con un signer propio (depósito + transferencia a una llave registrada); (c) cómo funciona el registro de llaves públicas; (d) el SDK web en Next.js; (e) si la intake puede derivar y guardar sus propias llaves del pool en el servidor sin Freighter; (f) en el código del sandbox (`Bitacora/lendara`), si SEP-10 acepta `memo` en el challenge y si SEP-24 acepta `memo`/`memo_type` hacia una cuenta ajena (D4). Script mínimo que hace depósito → transferencia → retiro desde el servidor. Resultado en `docs/spikes/spp.md` con la decisión de D1 y D2 | Hashes reales de las tres operaciones; decisiones D1 y D2 confirmadas con Anouk |
| H2 ✅ | Esqueleto del API | `feat/api-skeleton` | Crate, config por env, Axum, `/health`, Postgres + migraciones, `tracing`, manejo de errores | `make dev-api` responde `/health` con la base conectada |
| H3 | Contrato OpenAPI | `feat/repo-openapi` | `openapi.yaml` (sección 7.8), `packages/api-types` con `openapi-typescript`, `make codegen` | Codegen corre sin errores; los tipos compilan |
| H4 | Setup testnet + pool | `feat/api-testnet-setup` | `make setup-testnet` (sección 7.1), despliegue del SAC del USDC del sandbox (plan A), pool según el plan A o B, `GET /v1/info` | El script es idempotente; `/v1/info` devuelve el contrato del pool |
| H5 | Onboarding y ASP | `feat/api-asp-onboarding` | `shield_users`, `asp_events`, KYC simulado con escenarios, allow-list y block-list (o simulación en plan B), tests | `approved` agrega la llave; `rejected` no; `blocked` congela; hashes reales |
| H6a | Intents y cliente del anchor | `feat/api-ramp-intents` | Tabla `intents`, máquina de estados con tests, cliente SEP-1/SEP-10/SEP-24, `POST /v1/intents`, `GET /v1/intents/{id}`, modo ramp opcional por config | Se crea un intent `awaiting_anchor` con la URL interactiva real del sandbox |
| H6b | Watcher y worker de shielding | `feat/api-shielding-worker` | Watcher de pagos a la intake (Horizon, por memo), worker idempotente: depósito al pool + transferencia privada, seguimiento del estado del anchor | Un intent llega a `completed` con los hashes de depósito (`ext_amount > 0`) y transferencia (`ext_amount = 0`) |
| H7 | Web: onboarding, depósito y wallet | `feat/web-shield-flow` | Next.js, Freighter, SDK web del pool, rutas `/onboard`, `/deposit` (ambos modos), `/wallet` | Depósito de punta a punta en los dos modos desde la web |
| H8 | Auditoría | `feat/audit-panel` | `GET /v1/audit/*` en el API y la ruta `/audit` en la web | La comparación se entiende en 5 segundos |
| H9 | README, protocolo y demo | `docs/repo-readme` | README final, `docs/protocol/shield-protocol.md` (borrador del Nivel 2), `docs/scenarios.md`, `docs/threat-model.md` corto, guion del video | README listo para el jurado |
| H10 | Deploy (opcional) | `chore/repo-deploy` | API + Postgres (Fly.io o Railway) y web (Vercel), HTTPS | URLs públicas funcionando. **Solo si H6b y H7 están listos** |

**División del equipo:** Anouk en `apps/api` (H1, H2, H4, H5, H6a, H6b). Karu en la web (H7). Elisa en auditoría y documentación (H8, H9). H0 y H3 los hace quien llegue primero; H3 desbloquea el trabajo en paralelo.

**Calendario y línea de corte:**

| Cuándo | Meta |
|---|---|
| Domingo 25, noche | H0, H1 (spike) y H2. Decisiones D1 y D2 tomadas |
| Lunes 26, mañana | H3, H4, H5, H6a. Web conectada a la wallet y al pool (H7 en paralelo) |
| **Lunes 26, mediodía** | **H6b: modo ramp de punta a punta en testnet.** Si a esta hora no está, se recortan H8 a una versión mínima y H10 sin discusión |
| Lunes 26, tarde | H7 completo, H8, H9, video, entrega temprano (confirmar hora exacta) |

**Prioridad absoluta:** un intent del modo ramp que llegue a `completed` con una transferencia privada real en testnet, sin la dirección del usuario on-chain. Todo lo demás es secundario.

### Fase P: después de la hackathon (orden sugerido)

| # | PR | Contenido |
|---|---|---|
| P1 | Especificación del Shield Protocol | Extensión de SEP-24 completa (Nivel 2), ejemplos de requests y responses, discusión de seguridad |
| P2 | Integradores | Proyectos y API keys hasheadas (patrón del sandbox), auth en todos los endpoints |
| P3 | Relayer | Fee-bump para operaciones del usuario; el usuario no necesita XLM |
| P4 | Endurecimiento del conjunto de anonimato | Depósitos agrupados (batching) y montos redondeados desde la intake |
| P5 | Divulgación selectiva | UI para que el usuario comparta un pago puntual con un tercero |
| P6 | Agentes | Llaves de agentes registradas bajo el KYC de su dueño ("know your agent"), límites de gasto, auditoría del dueño |
| P7 | Segundo carril: Confidential Tokens | Router de privacidad `confidential` (montos ocultos, partes visibles) para nómina |
| P8 | SDK `@lendara/shield` | Cliente TS para integradores + opción `privacy` en el widget |
| P9 | Retiro a fiat | Retiro del pool hacia la cuenta del anchor (SEP-24 withdraw) |
| P10 | Anchor nativo (Nivel 2) | Implementar el Shield Protocol dentro del sandbox (PR en el otro repo) |
| P11 | Calidad | Test de contrato contra `openapi.yaml`, E2E con Playwright, cobertura, threat model completo |

---

## 12. Decisiones abiertas

Cada una tiene una recomendación. Si Anouk no responde y el PR no puede esperar, sigue la recomendación y dilo explícitamente en el resumen.

| # | Decisión | Recomendación | Antes del PR |
|---|---|---|---|
| D1 | Lenguaje del backend | ✅ **Rust (Axum)** + `stellar-private-payments = "=0.2.1"` (H1) | — |
| D2 | Pool propio o existente | ✅ **Plan A**: pool propio `allowlist-blocklist` + `gvk-traceable` (H1) | — |
| D3 | Cómo se identifica al receptor en el pool | ✅ **Llaves públicas crudas** (nota + cifrado) guardadas en Shield; sin registry on-chain porque expone la G (H1, hallazgo 8). Verificar con hash real en H6b | — |
| D4 | Tipo de memo hacia el anchor | ✅ **`id`**: SEP-10 con el memo del usuario, SEP-24 con el memo del intent (columnas `bigint`). El sandbox acepta ambos (H1) | — |
| D5 | Hosting | Fly.io o Railway para API + Postgres, Vercel para la web | H10 |
| D6 | Si el sandbox no acepta memo en SEP-10 o SEP-24 | ✅ No hace falta: el sandbox acepta memos (H1) | — |

---

## 13. Estado actual y convenciones aprendidas

> Actualizar al cerrar cada PR.

### Estado
- H0 (scaffold) ✅ en `chore/repo-scaffold` (25-oct): pnpm workspace, Makefile, Postgres 17, CI (JS, gitleaks, actionlint), validación del título del PR, lefthook + commitlint, Apache-2.0.
- H1 (spike de SPP) ✅ en `spike/spp-verification` (25-oct): depósito, transferencia privada, retiro y auditoría con GVK desde el servidor, con hashes reales. Detalle en `docs/spikes/spp.md`.
- H2 (esqueleto del API) ✅ en `feat/api-skeleton` (25-oct): crate `shield-api`, config validada, `ApiError`, tracing JSON, `/health` (200/503), request id, CORS, timeout, apagado ordenado, migraciones embebidas, job `api` en el CI, 23 tests, cobertura 97 % de líneas (sin `main.rs`).
- Siguiente: H3 (contrato OpenAPI).

### Riesgos identificados (25-oct)
1. **Tiempo:** 12 PRs en ~1 día. ~~Plan B del backend~~ descartado en H1.
2. **SAC del USDC** necesario para el pool propio (H4). Revisar también los flags del issuer del USDC del sandbox: con `AUTH_REVOCABLE` o `AUTH_CLAWBACK_ENABLED` el issuer puede congelar todo el pool (va al threat model).
3. **Correlación por monto y hora** en la entrada del modo ramp (documentado en 2.1; mitigación en P4).
4. ~~Llaves del pool de la intake~~ resuelto en H1.
5. ~~Memo en SEP-10/SEP-24~~ resuelto en H1.
6. **Ventana de 7 días del RPC:** el prover necesita todo el historial del pool. Un pool desplegado el 26-oct dura hasta ~2-nov sin bootnode. Después: correr `tools/bootnode` de SPP o redesplegar.
7. **Modo direct:** quien firma una operación del pool queda visible; hasta el relayer (P3), la dirección del usuario aparece como usuaria del pool en cada transferencia.

### Decisiones tomadas
- Nombre del producto y del repo: **Lendara Shield** (`lendara-shield`). Nombre del estándar: **Shield Protocol**.
- Dos modos (`ramp` y `direct`) sobre un solo núcleo; el modo ramp es opcional por config y consume cualquier anchor SEP-24 solo por estándares.
- Mismas reglas de Git, PRs y flujo de ramas que el sandbox.

### Herramientas y trampas conocidas
- El pnpm global de la máquina de Anouk es 9.x; el repo fija `pnpm@12.10.1` en `packageManager` y corepack lo usa automáticamente dentro de la carpeta.
- gitleaks y actionlint no están instalados localmente: correrlos con Docker usando las mismas imágenes del CI.
- Los targets del `Makefile` cuyo código llega en un PR futuro imprimen "arrives in Hx" en vez de fallar. Cada PR los reemplaza por el comando real.
- API: las migraciones van embebidas (`sqlx::migrate!`), se aplican al arrancar y con `make migrate`; no hace falta `sqlx-cli` hasta que existan consultas con `query!` (entonces: `cargo sqlx prepare` y el chequeo en el CI).
- API: `sqlx` 0.9 (versión mayor nueva, oct-2026). La API usada (`PgPoolOptions`, `migrate!`, `#[sqlx::test]`) no cambió respecto a 0.8.
- API: `cargo test` necesita Postgres arriba (`make db-up`) y `DATABASE_URL`; el `Makefile` la toma del `.env`. `#[sqlx::test]` crea una base aislada por test.
- `make lint/test/build` corren Rust + JS; el CI usa los targets separados (`-api`, `-js`).
- SPP: fijar el tag `sdk-v0.2.1` (contratos, crate y npm coinciden). El tag `alpha` de npm está viejo: usar `stellar-private-payments@0.2.1`.
- SPP: compilar en `--release` (el prover en debug es muy lento). La primera ejecución descarga los artefactos de circuito (~30 s).
- `deploy.sh` de SPP termina con dos `notice` de custodia (admin sin multisig): esperado en testnet.
- Heredado del sandbox: pnpm puede bloquear paquetes publicados hace menos de un día (`minimumReleaseAge`); usar la versión anterior y no agregar excepciones.

---

## 14. Cómo comportarte (resumen)

- Español con Anouk; inglés en el código.
- Un PR a la vez, pequeño y coherente. Plan primero, código después.
- **Nunca** `git add`, `commit`, `push` ni PRs. Al final, entrega el bloque "Resumen del PR" y detente.
- Pregunta antes de agregar dependencias o tomar decisiones abiertas.
- No inventes comportamiento de SPP ni de las SEPs: consulta la fuente y cítala.
- Verifica (fmt, lint, tests, build y testnet) antes de decir que algo funciona, y reporta resultados reales con hashes.
- Solo testnet. Nada de secretos en el repo. Higiene de privacidad en los logs.
- Tests de todo lo que agregas, incluidos los errores (sección 10.1). Código impecable: puertos y adaptadores, newtypes, sin `unwrap`.
- Respuestas **cortas**: siguiente paso + comandos. Al cerrar cada PR: comandos de commit (separados, una línea cada uno, sin comillas en rutas, sin `Co-Authored-By`) + título y descripción del PR en inglés, listos para pegar.
- Si algo del plan parece incorrecto o hay una forma claramente mejor, dilo con argumentos antes de desviarte.
