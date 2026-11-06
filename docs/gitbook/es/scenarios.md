# Escenarios

Cada resultado que puede mostrar la demo, y qué hace Shield con él. Hay dos filtros independientes:

1. **La decisión de elegibilidad del integrador**, simulada en `/onboard`. Solo sandbox: un integrador real aplica su propia política.
2. **El resultado de la fuente de fondeo** (modo rampa), que se elige en el widget del anchor sandbox de Lendara con una tarjeta de prueba.

## 1. Decisión de elegibilidad (`POST /v1/onboarding`, campo `scenario`)

Cada solicitud de registro lleva la firma SEP-53 de la cuenta sobre sus llaves; si la firma falta o es de otra cuenta, responde `400` y no se registra nada.

| Escenario | Elegibilidad | Allow-list (ASP) | Qué pasa después |
|---|---|---|---|
| `approved` (por defecto) | aprobado | Hoja insertada on-chain → `allowed` | Puede depositar (en ambos modos), transferir y retirar |
| `rejected` | rechazado | Nunca se inserta → `not_allowed` | El pool rechaza las pruebas del usuario; `POST /v1/intents` responde `409 user_not_allowed`; la web devuelve al usuario al registro |
| `blocked` | aprobado | Se inserta y luego la llave de nota se agrega a la **block-list** → `blocked` | Congelado: cada prueba exige no pertenecer a la block-list, así que nada se mueve hasta que el ASP lo desbloquee |

Si la red no está disponible durante el registro, el usuario queda en `pending` (`201`). Leer el estado (`GET /v1/users/{account}`) resuelve una escritura del ASP que ya está en curso, pero nunca envía una, así que una escritura que ni siquiera se pudo preparar espera a la siguiente llamada de registro firmada. Después de 20 segundos en `pending`, la web ofrece *Reintentar el registro*, que firma de nuevo y lo retoma (es idempotente: misma cuenta y mismas llaves).

Acciones de admin (`ADMIN_TOKEN`): `POST /v1/asp/block` y `POST /v1/asp/unblock`. La allow-list es de solo agregado (el contrato de membresía de SPP no permite quitar), así que revocar a un usuario significa bloquear su llave.

Bloquear a un usuario a mitad de un intent:

- `POST /v1/intents` revisa la allow-list con la fila del usuario bloqueada, así que un bloqueo confirmado al mismo tiempo o rechaza el intent nuevo o lo pasa a rechazado. Si el bloqueo entra mientras el anchor está respondiendo, la solicitud responde `409 user_not_allowed` y el usuario nunca recibe la URL interactiva.
- Cuando se confirma el bloqueo, los intents del usuario que todavía no tienen fondos (`created`, `awaiting_anchor`) pasan a `rejected`. Si el anchor igual paga uno de ellos, el pago se guarda como no emparejado para revisión manual.
- Una vez que el dinero está en custodia de Shield (`funds_received` y después), un intent solo termina en `completed` o `error`, nunca en `rejected` ni `expired`. El worker de depósitos revisa al usuario antes del depósito, antes de la transferencia privada y antes de reconstruir una transferencia que ya no puede entrar: el intent de un usuario bloqueado termina en `error` con los fondos en custodia de Shield (la cuenta ómnibus, o la nota propia de Shield después del depósito). No se envía nada a la llave congelada. No se persigue una transferencia ya enviada: lo que reciba una llave bloqueada queda congelado, porque gastar exige una prueba de que la llave no está en la block-list.
- Desbloquear no revive los intents rechazados.

## 2. El resultado del anchor (modo rampa, widget del sandbox de Lendara)

Sirve cualquier fecha de vencimiento futura y cualquier CVC de 3 dígitos.

| Tarjeta de prueba | Resultado en el sandbox | Intent en Shield | Pool |
|---|---|---|---|
| `4242 4242 4242 4242` | Aprobada: USDC pagados a la cuenta ómnibus de Shield con el memo del intent | `awaiting_anchor → funds_received → shielding → transferring → completed`, con tres hashes | Depósito desde la cuenta ómnibus + transferencia privada a las llaves del usuario |
| `4000 0000 0000 0002` | Rechazada por el anchor | `error`, con el mensaje del anchor | Sin cambios |
| `4000 0000 0000 9995` | Pago no recibido | `expired` | Sin cambios |
| `4000 0000 0000 0077` | Aprobada a los 30 s | Se queda en `awaiting_anchor` mientras el anchor está en `pending_anchor` y luego se completa | Igual que aprobada |

Shield sigue al anchor con `GET /sep24/transaction` (el sandbox no tiene webhooks) y detecta el pago en Horizon por memo, código del activo **y** emisor.

## 3. Casos límite con dinero (cubiertos por tests)

| Caso | Comportamiento |
|---|---|
| Alguien envía el activo con el memo de un intent (adelantándose al anchor) | Se registra como "coincide con el memo de un intent; no confirmado por el anchor"; solo el pago que reporta el anchor fondea el intent (verificado en testnet: el dust [`1f533b4e…`](https://stellar.expert/explorer/testnet/tx/1f533b4e04b51d2734162b4a2730d85cecbf9c796cfac47430157ba426c1f178) quedó en revisión, y el pago del anchor [`3a971137…`](https://stellar.expert/explorer/testnet/tx/3a97113724334b86130aeb18d0de249f7090dc8364da2ca4cb30d1017883c3b6) completó el intent) |
| El anchor reporta `completed` antes de que Shield lea su pago en Horizon | Espera y vuelve a revisar unos segundos después |
| El anchor reporta `completed` pero su pago nunca aparece (sin memo, activo equivocado, sin `stellar_transaction_id`) | Espera 15 minutos desde el primer reporte y luego pasa a `error` para revisión manual; un pago que llegue después se guarda como no emparejado |
| Otra persona pide un depósito a nombre de un usuario | Rechazado: `POST /v1/intents` exige la firma SEP-53 de la cuenta (`400 invalid_signature`) |
| Una solicitud de intent firmada se envía dos veces | La segunda responde `409 request_replayed`; al anchor se le pide una sola vez |
| Un cliente inunda el registro o los intents | `429 rate_limited` al superar `RATE_LIMIT_PER_MINUTE` por IP de cliente |
| Llega un pago después de que el intent expiró, o llega un segundo pago | Se guarda como no emparejado en `intake_payments`: dinero en custodia para revisión manual; nunca fondea nada de forma automática |
| Activo o emisor equivocado | No se empareja |
| El envío al pool da timeout pero la transacción entró | Primero se revisa el hash guardado; nunca se paga dos veces |
| La transacción se descartó | Se reenvía el **mismo** sobre firmado |
| El RPC todavía no indexó una transacción que entró | No se reemplaza hasta que siga sin verse durante 60 s después de que la secuencia de la cuenta ómnibus la haya superado |
| La prueba quedó vieja (por ejemplo, cambió la raíz del ASP) | Se prepara de nuevo y se reemplaza; los reintentos esperan cada vez más (backoff exponencial) |
| Fallas repetidas | `error` con "los fondos están en custodia de Shield y necesitan revisión" |
