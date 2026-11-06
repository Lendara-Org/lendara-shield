# Modelo de amenazas

Qué oculta Lendara Shield, frente a quién, y los límites que conocemos. Shield corre en **testnet**, sobre un protocolo en developer preview con **contratos sin auditar**. El tramo 2 amplía este documento a un modelo de amenazas al estilo STRIDE, con un plan de monitoreo para producción.

## Quién ve qué

| Observador | Modo rampa | Modo directo |
|---|---|---|
| **El público** (explorador, RPC) | El anchor le pagó a la cuenta ómnibus de Shield; Shield depositó en el pool; Shield hizo transferencias privadas. **No la dirección del usuario**, ni quién recibió qué | La dirección del usuario deposita (monto visible), firma sus propias transferencias privadas (monto y destinatario ocultos) y retira (monto visible) |
| **La fuente de fondeo** | Al usuario y el monto (recibe el pago y aplica sus propios controles). No lo que pasa dentro del pool | — |
| **Shield** (operador, ASP) | Qué cuenta de Stellar es dueña de qué llaves del pool (es el ASP); cada intent: cuenta, monto, hashes | Qué cuenta es dueña de qué llaves |
| **El auditor** (Global View Key) | Cada nota del pool: llave del dueño y monto, que Shield relaciona con cuentas | Igual |
| **Otro usuario** | Nada sobre los saldos o transferencias de otros | Igual |

## Activos y cómo se protegen

| Activo | Dónde vive | Protección |
|---|---|---|
| Las llaves de gasto de los usuarios | Solo en el navegador del usuario (derivadas de una firma de Freighter, guardadas en el almacenamiento del SDK en el navegador) | Nunca se envían a Shield; Shield solo recibe llaves públicas y la hoja del ASP |
| Global View Key (auditor) | Un archivo en el servidor de Shield (`SHIELD_GVK_KEY_FILE`) | Solo se pasa al proceso `pool-cli` del auditor; nunca al worker de depósitos, al navegador ni a los logs; el endpoint de auditoría exige `AUDITOR_TOKEN` (comparación en tiempo constante) |
| Llave del operador (admin del pool, ASP) | Entorno del servidor | Separada de las llaves ómnibus y del auditor (el API no arranca si las semillas del operador y de la ómnibus son iguales); los endpoints de admin exigen `ADMIN_TOKEN` |
| Llave ómnibus (intake) y su wallet del pool | Entorno del servidor y una wallet SQLite fuera del repo | Llave separada; `pool-cli` corre con un entorno vacío más solo lo que necesita |
| Fondos en custodia | La cuenta ómnibus, entre el pago del anchor y la transferencia privada | Worker idempotente (el hash se guarda antes de enviar y nunca se vuelve a preparar mientras pueda haber entrado); las fallas repetidas se detienen en `error` para revisión manual |
| La relación usuario ↔ llave | La base de datos de Shield | Nunca en los logs: los logs llevan ids de intents y plantillas de rutas, nunca "dirección + monto" |
| El registro de una cuenta | `POST /v1/onboarding` | La solicitud lleva la firma SEP-53 de la cuenta (`signMessage` de Freighter) sobre las llaves y la hoja exactas, así que nadie puede asociar sus llaves a la cuenta de otra persona |
| Iniciar un depósito | `POST /v1/intents` | La solicitud lleva la firma SEP-53 de la cuenta sobre la moneda, el monto, un `request_id` de un solo uso y `issued_at` (de 5 minutos como máximo), así que nadie más puede abrir depósitos en el anchor a nombre de un usuario, y una solicitud capturada no se puede reenviar |
| La sesión del widget del anchor | La URL interactiva SEP-24 (lleva el token de un solo uso del anchor) | Solo se devuelve a la solicitud firmada que creó el intent; `GET /v1/intents/{id}` nunca la incluye |
| Los fondos del operador de Shield y el anchor | El registro (escrituras del ASP on-chain) y la creación de intents (depósitos en el anchor) | Límite de solicitudes por IP de cliente (`RATE_LIMIT_PER_MINUTE`, 20 por defecto) |
| El dinero de un usuario bloqueado | Los intents de un usuario que el ASP bloquea | Los intents sin fondos terminan en `rejected` cuando se confirma el bloqueo; uno con fondos se detiene antes de su depósito, su transferencia o una transferencia reconstruida, y termina en `error` con los fondos en custodia de Shield, nunca enviados a la llave congelada |
| Fondear un intent | El watcher y la sincronización con el anchor | Los memos de los intents son aleatorios (63 bits). Un pago con el memo de un intent solo lo fondea si el anchor reporta esa misma transacción (`stellar_transaction_id`); todo lo demás queda en revisión, así que el polvo (dust) enviado con el memo de otra persona no puede secuestrar su depósito |

## Límites conocidos (dichos antes de que alguien pregunte)

1. **Correlación por monto y hora en la rampa.** El pago del anchor a Shield y el depósito de Shield en el pool llevan el **mismo monto** con segundos de diferencia, y el conjunto de anonimato en testnet es mínimo. Un observador no puede ver la dirección del usuario, pero puede vincular "este pago del anchor" con "este depósito". Mitigación, todavía no construida: depósitos agrupados, montos redondeados y demoras aleatorias desde la cuenta ómnibus (tramo 3).
2. **La fuente de fondeo conoce al usuario y el monto.** Es inherente a cualquier parte que recibe el pago. Shield elimina el vínculo público, no lo que sabe la fuente de fondeo.
3. **El modo directo revela al firmante.** Quien firma una transacción del pool es su cuenta de origen, así que en modo directo la dirección del usuario aparece como usuaria del pool en cada transferencia (los montos y destinatarios siguen ocultos). Mitigación: un relayer con fee-bump (tramo 1). La web lo dice junto al botón.
4. **Custodia.** En modo rampa, la cuenta ómnibus de Shield tiene por un momento los fondos del usuario, y Shield tiene las llaves del pool de esa nota intermedia. Es el mismo modelo de confianza que cualquier paso de rampa custodial; el nivel 2 del protocolo (anchors que depositan de forma nativa en el pool) lo elimina.
5. **El emisor del activo todavía puede actuar.** Hoy el emisor del USDC del sandbox no tiene `AUTH_REVOCABLE` ni `AUTH_CLAWBACK_ENABLED`, pero no es `AUTH_IMMUTABLE`: podría activarlos más adelante y congelar el saldo del pool o hacerle clawback. El emisor además aparece como una cuenta visible en cada transferencia de su activo.
6. **Shield y el auditor son partes de confianza.** Shield sabe qué cuenta es dueña de qué llaves (es el ASP); el auditor puede descifrarlo todo. Es el compromiso de cumplimiento de un pool con permisos. Repartir los roles entre partes independientes es trabajo futuro.
7. **Contratos sin auditar, en developer preview.** Stellar Private Payments `sdk-v0.2.1`. En testnet, el admin del pool es una sola llave (sin multisig).
8. **Ventana de historial y bootnode.** Para generar pruebas se necesita el historial completo de eventos del pool; el RPC público guarda unos 7 días. Shield corre el bootnode de SPP para los eventos más viejos (el primer pool superó la ventana antes de que existiera y se redesplegó el 6 de noviembre). Los clientes confían en el bootnode para ese historial: podría omitir o falsificar eventos viejos o indicar un ledger de traspaso equivocado, y en ese caso las pruebas fallan, pero no permite robar fondos (los contratos verifican cada prueba). También ve la IP y el momento de cada cliente que sincroniza a través de él. Cualquiera puede correr su propio bootnode o un RPC de archivo.
9. **`GET /v1/users/{account}` es público.** Le dice a cualquiera si una cuenta se registró en Shield, su estado de elegibilidad y de allow-list, y el hash de su inserción en la allow-list: la misma señal de "esta dirección usa el pool" que evitamos al no usar el registro público de llaves de SPP. Solución (todavía no construida): exigir una sesión derivada de la firma de la cuenta para leer su estado. Una firma en cada lectura no es opción: la web consulta este endpoint mientras el registro está pendiente, así que primero hacen falta sesiones (tramo 1). `GET /v1/intents/{id}` tampoco tiene autenticación, pero necesita el id aleatorio del intent y no devuelve ni la cuenta ni la URL interactiva.
10. **Confianza en el navegador.** El lock del circuito se fija al compilar la app web, así que los artefactos del circuito que se sirven junto a ella se verifican contra él, pero los usuarios igual confían en la app y en quien la sirve. Una Content-Security-Policy limita los scripts, los workers y el acceso a la red al origen de la app, al API de Shield, al RPC y al bootnode; todavía permite scripts inline, porque Next los inserta en las páginas renderizadas de forma estática.
11. **El límite de solicitudes es por IP de cliente.** Detrás de un proxy inverso, todos los clientes comparten la dirección del proxy, así que el límite pasa a ser global. Un atacante con muchas direcciones igual puede registrar muchas cuentas nuevas (cada una le cuesta al operador una escritura on-chain).

## Todavía no construido

Sesiones y llaves de API por proyecto y el relayer (tramo 1); gestión de llaves en un KMS/HSM y divulgación selectiva por nota para los usuarios (tramo 2); hosting de producción y refuerzo del conjunto de anonimato (tramo 3); límite de solicitudes global o que tenga en cuenta el proxy.
