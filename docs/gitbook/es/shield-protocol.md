# Shield Protocol — borrador 0.1

> Estado: **borrador 0.1**. Lendara Shield implementa el nivel 1 en testnet. El nivel 2 es una propuesta y **no** está implementado.

El Shield Protocol permite que una fuente de fondeo entregue el dinero **dentro de un pool privado** en lugar de enviarlo a la dirección pública del usuario, mientras la elegibilidad, la allow-list y la auditoría quedan en manos de partes responsables. Su nombre no lleva la marca "Lendara" para que cualquier integrador lo pueda adoptar. La primera fuente de fondeo que especifica es un anchor SEP-24.

**Piezas base:** [SEP-1](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0001.md), [SEP-10](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0010.md), [SEP-24](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0024.md) y un pool de [Stellar Private Payments](https://developers.stellar.org/docs/build/apps/privacy) con un ASP (allow-list + block-list) y una Global View Key.

## Roles

| Rol | Qué hace |
|---|---|
| **Usuario** | Tiene una wallet de Stellar; deriva las llaves del pool en la wallet (las llaves de gasto nunca salen de ahí) |
| **Fuente de fondeo** | Paga el activo (hoy, un anchor SEP-24). Aplica sus propios controles; el protocolo no los define |
| **Integrador** | Decide cuáles de sus usuarios son elegibles, según su propia política |
| **ASP** | Escribe la decisión del integrador en la allow-list y la block-list del pool |
| **Operador del pool** | Despliega y administra el pool |
| **Auditor** | Tiene la Global View Key del pool; puede descifrar cada nota |
| **Shield** (nivel 1) | ASP + cuenta ómnibus + un cliente para la fuente de fondeo (hoy SEP-24) |

## Nivel 1 — Shield como middleware (implementado)

Shield se ubica entre **cualquier** anchor SEP-24 y el pool. El anchor no se modifica.

### Registro

1. La wallet del usuario deriva las llaves del pool (llave de nota y llave de cifrado) y la hoja de membresía del ASP `leaf = H(note_public_key, membership_blinding)`.
2. El usuario le envía a Shield **solo** `{stellar_account, note_public_key, encryption_public_key, asp_leaf}`. El usuario **no** se registra en el registro público de llaves del pool: eso es una transacción pública desde su propia dirección.
3. El integrador decide si el usuario es elegible. Si se aprueba, Shield (como ASP) inserta `asp_leaf` en la allow-list on-chain. Bloquear es insertar la llave de nota en la block-list. En SPP, la allow-list es de solo agregado (append-only).

### Depósito (un intent)

1. El usuario le pide a Shield depositar `amount` de `fiat_currency`.
2. Shield se autentica con el anchor mediante **SEP-10**, como su cuenta ómnibus, con un memo `id` que identifica al usuario (cada usuario es un cliente distinto para el anchor). Shield verifica el challenge por completo antes de firmarlo.
3. Shield llama a `POST /transactions/deposit/interactive` (**SEP-24**) con:
   - `asset_code`: el activo del pool
   - `account`: **la cuenta ómnibus de Shield**
   - `memo_type: id`, `memo`: un número que identifica este intent
   - `source_asset: iso4217:<fiat_currency>`
4. El usuario completa el flujo interactivo del anchor. El anchor le paga a la cuenta ómnibus con ese memo.
5. Shield detecta el pago (Horizon), lo empareja por memo, **código y emisor del activo**, y registra el monto realmente recibido.
6. Shield deposita ese monto en el pool desde la cuenta ómnibus (`ext_amount > 0`). La nota es de Shield.
7. Shield la transfiere en privado a las llaves registradas del usuario (`ext_amount = 0`).

**Requisito de idempotencia:** cada transacción del pool se prueba, se firma y su hash se **guarda antes de enviarla**. En un reintento, la implementación revisa el hash guardado y reenvía el mismo sobre. Solo puede preparar un reemplazo cuando la original ya no puede entrar, es decir, cuando su número de secuencia ya se consumió y el hash sigue sin aparecer después de un período de gracia que cubre el retraso de indexación del RPC.

**Máquina de estados:** `created → awaiting_anchor → funds_received → shielding → transferring → completed`. Las salidas de error son `rejected`, `expired` y `error`. Una vez que los fondos están en custodia, un intent solo puede terminar en `completed` o `error`.

### Auditoría

- El auditor descifra la actividad del pool con la Global View Key. El ASP relaciona las llaves de nota con sus usuarios.
- Una implementación **no debe** exponer la llave de vista a los navegadores, y **debería** entregarla solo al proceso que audita.

### Qué logra el nivel 1

On-chain, el camino del depósito muestra "anchor → ómnibus" y "ómnibus → pool". La dirección del usuario no aparece en ninguno. Ver el [modelo de amenazas](threat-model.md) sobre la correlación por monto y hora, y sobre la custodia.

## Nivel 2 — Depósitos privados nativos (propuesta)

El anchor deposita **directo en el pool** a las llaves del usuario, lo que elimina el paso de custodia de Shield. Es una extensión de SEP-24.

### Descubrimiento (SEP-1 `stellar.toml`)

```toml
SHIELD_PROTOCOL_VERSION = "0.1"
SHIELD_POOL_CONTRACT = "C…"        # SPP pool for the asset
SHIELD_ASP_ENDPOINT = "https://…"  # where wallets onboard keys (may be the anchor itself)
```

### Capacidad (SEP-24 `GET /info`)

```json
{
  "deposit": { "USDC": { "enabled": true } },
  "features": { "account_creation": true, "claimable_balances": true, "shielded_deposit": true }
}
```

### Depósito interactivo (SEP-24)

`POST /transactions/deposit/interactive` acepta, **en lugar de** `account`:

| Campo | Significado |
|---|---|
| `destination_type` | `"shielded"` |
| `shielded_recipient` | Las llaves públicas del pool del destinatario, `<note_public_key>:<encryption_public_key>` (hex `0x`) |

La sesión SEP-10 sigue identificando al cliente. El anchor debe rechazar un `shielded_recipient` cuya hoja no esté en la allow-list del pool (el anchor es el ASP o trabaja con él).

### Liquidación

La cuenta de distribución del anchor llama al depósito del pool con una nota de salida para `shielded_recipient`. Reporta el resultado mediante `GET /transaction`, como siempre:

```json
{
  "transaction": {
    "id": "…",
    "status": "completed",
    "stellar_transaction_id": "<pool deposit hash>",
    "shielded": { "pool_contract": "C…", "ext_amount": "9.3039215" }
  }
}
```

### Retiro (esbozo)

El usuario retira del pool a la cuenta del anchor con una prueba de membresía válida (retiro SEP-24, `ext_amount < 0`), y el anchor paga en fiat. Los detalles quedan por especificar.

### Consideraciones de seguridad

- **Correlación.** El monto público de un depósito es igual a la cotización en fiat. Los anchors deberían agrupar depósitos, redondear montos y agregar demoras (las mismas mitigaciones aplican al nivel 1).
- **Custodia.** El nivel 2 elimina la custodia ómnibus del nivel 1. El anchor nunca tiene la nota.
- **Separación de funciones.** La llave de firma del ASP, la llave de distribución del anchor y la llave de vista del auditor deberían pertenecer a partes distintas o, al menos, ser llaves distintas.
- **Poderes del emisor.** Un activo clásico cuyo emisor puede revocar o hacer clawback puede congelar el saldo del pool. Los pools deberían preferir activos con `AUTH_IMMUTABLE` o documentar el riesgo.
- **Quién firma es público.** Las operaciones propias del usuario en el pool revelan su cuenta como firmante, salvo que un relayer (fee-bump) las envíe.

## Preguntas abiertas

1. ¿`shielded_recipient` debería ser una búsqueda en el registro (por dirección `G…`) o llaves directas? Las llaves directas evitan la transacción pública en el registro; un registro facilita los pagos entre pares.
2. ¿Cómo le prueba un anchor a una wallet que su depósito fue a las llaves correctas sin revelarlas públicamente? (La divulgación selectiva por nota es una candidata.)
3. Federación de ASP: ¿varios integradores pueden compartir la allow-list de un mismo pool?
