# Lendara Shield — Estrategia para SCF, modelo de negocio e integración en wallets

Documento único de trabajo, actualizado con la investigación del 5 de noviembre de 2026, hora de Argentina. Reúne el contexto recuperado de la conversación, las fuentes consultadas y las decisiones pendientes. No es una postulación enviada.

## 1. Objetivo y argumento central

Preparar una postulación sólida al Stellar Community Fund (SCF), usando Raven y fuentes oficiales, para llevar Lendara Shield desde su implementación de referencia en testnet hacia una integración en una wallet existente.

La comparación con Solana y otras blockchains es central: allí ya existen protocolos de privacidad que cobran comisiones y productos que los incorporan. Esos antecedentes permiten fundamentar una oportunidad de producto, distribución e ingresos para Shield en Stellar. La adopción y rentabilidad de Shield deberán validarse con su propio piloto.

El resultado buscado es que una wallet muestre un saldo privado y permita usarlo dentro de su experiencia. Conectar una wallet a la web de Shield es un paso distinto de incorporarlo al producto. No se acordó construir una nueva wallet de consumo.

Todavía no se eligieron partner, presupuesto, track de SCF, tarifa comercial, cronograma ni fecha de lanzamiento en mainnet. Se pidió conservar todo en este único archivo.

## 2. Qué es Shield y qué problema resuelve

**Lendara Shield** es la implementación de referencia del **Shield Protocol**, una capa de operación de pagos privados para Stellar. Su propuesta es conectar wallets, aplicaciones de pagos y rampas fiat con un pool de Stellar Private Payments (SPP).

Un pool necesita alguien que gestione participantes habilitados y bloqueados, opere la política de acceso, proteja las claves de auditoría y cubra costos. Shield busca ofrecer esa operación de forma compartida mediante una API, actuando como **ASP — Association Set Provider**.

También aborda la entrada del dinero: cuando una rampa paga a la dirección propia del usuario, publica dirección, monto y momento. El modo Ramp de Shield recibe en una cuenta ómnibus y luego asigna el saldo dentro del pool a las claves privadas del usuario.

La propuesta es que cada aplicación determine qué usuarios son elegibles bajo una política acordada y Shield opere el pool. La autenticación para múltiples integradores y el proceso de elegibilidad de producción siguen pendientes. El KYC del prototipo está simulado.

**Lemas originales:** “Pay privately, disclose selectively.” / “KYC once, pay privately, disclose selectively.”

## 3. Cómo funciona y qué está implementado

### Registro

El navegador deriva las claves del pool. Shield recibe las claves públicas y la hoja del ASP, verifica la firma de la cuenta mediante SEP-53 y agrega la hoja aprobada a la lista on-chain. Las claves de gasto del usuario permanecen con él. En Ramp se evita registrar al usuario en el registro público de claves de SPP, porque publicaría su dirección.

### Modo Ramp: moneda local → saldo privado

1. El usuario se registra y pasa el escenario de elegibilidad/KYC del sandbox.
2. Shield descubre y autentica con el ancla mediante SEP-1/SEP-10 y solicita un depósito interactivo SEP-24.
3. El usuario paga en moneda local. El ancla envía USDC a la cuenta ómnibus de Shield, identificando el intento con un memo.
4. Shield comprueba que el pago recibido sea exactamente el informado por el ancla y corresponda al activo y emisor esperados.
5. Shield deposita desde la cuenta ómnibus al pool y transfiere privadamente a las claves del usuario.
6. El usuario ve el saldo privado; el auditor autorizado puede descifrar la actividad.

En el recorrido documentado, la dirección Stellar del usuario no figura en el pago del ancla, depósito al pool ni transferencia firmada por intake. El ancla y Shield sí conocen información de identidad/pago, y sigue siendo posible correlacionar movimientos por monto y tiempo.

Se usan interfaces SEP-24 para evitar modificaciones al ancla. La integración probada aquí es el sandbox de Lendara; “cualquier ancla SEP-24” es un objetivo de interoperabilidad, no compatibilidad universal ya demostrada.

### Modo Direct: USDC propio → pool

El usuario deposita desde su wallet y luego transfiere o retira. Su dirección es visible en el depósito y, según la documentación local, como firmante de sus operaciones hasta implementar un relayer. No corresponde prometer anonimato completo del remitente en este modo.

### Claves, pruebas y reintentos

- Operador, intake y auditor usan claves separadas. La **Global View Key (GVK)** permite ver todo el pool, sin autorizar gastos. Debe definirse su custodia para cada integración.
- La divulgación actual consiste en acceso autorizado a la auditoría del pool. La divulgación por nota controlada por el usuario es un pendiente del roadmap.
- Las operaciones Direct generan pruebas Groth16 del lado del cliente. Las operaciones Ramp de intake usan el CLI Rust invocado por la API; no todas las pruebas se generan en el navegador del usuario.
- Cada transacción se prepara, prueba, firma y guarda antes de enviarse. Los reintentos consultan su hash y estado para evitar pagos duplicados.
- El pool aplica la política configurada mediante pruebas; Shield mantiene las listas y el proceso operativo.
- “Compliant” expresa el objetivo de diseño con controles y auditoría; el KYC simulado no demuestra aprobación regulatoria ni cumplimiento de producción.

**Estado:** solo testnet. El proyecto describe SPP como una developer preview con contratos no auditados. No usar fondos reales. La investigación no volvió a ejecutar el flujo completo ni comprobó su disponibilidad actual; los intentos de lectura web de la demo y GitHub fallaron sin demostrar que estuvieran caídos.

## 4. Comparación con otras blockchains y evidencia de ingresos

| Producto | Qué encontramos | Qué aporta a la propuesta de Shield |
|---|---|---|
| Privacy Cash | La documentación del protocolo describe pools ZK, relayers, screening de direcciones y SDK. En Solana publica depósitos sin comisión de protocolo y retiros con 0,35% más 0,006 SOL por destinatario, o equivalente del token. | Antecedente de infraestructura de privacidad con cobro por uso. El flujo convencional empieza con un depósito cripto; el Ramp de Shield combina el ingreso fiat con una cuenta ómnibus y la asignación privada al usuario. |
| Hush, wallet de Solana | Sus propias páginas describen una wallet que utiliza Privacy Cash y desglosa las comisiones del proveedor. Hush dice no retener una parte de esas comisiones. | Ejemplo particularmente cercano al objetivo: una wallet incorpora infraestructura de privacidad de otro proyecto. La infraestructura puede cobrar aunque la wallet no capture esa comisión. |
| Nullmask | Su documentación actual describe Ethereum y Base, saldos privados y uso desde una wallet existente agregando una red. Su página de analíticas publica 0,5% sobre depósitos y 0,5% sobre retiros. | Antecedente de experiencia de saldo privado dentro del entorno de una wallet. No debe presentarse como producto de Solana basándose en las notas iniciales. |
| RAILGUN | Sus documentos publican 0,25% al hacer shield y 0,25% al hacer unshield, más el costo del relayer. No aplican otra comisión porcentual de protocolo a todas las operaciones internas. | Referencia para decidir en qué momento cobrar: entrada/salida frente a cada pago. Sus ingresos se destinan según la gobernanza del protocolo; no equivalen automáticamente a ingresos de una empresa operadora. |
| Privacy Pools / 0xbow | Su sitio ofrece SDK para wallets y aplicaciones, un ASP que revisa depósitos y divulgación mediante viewing keys. Expresamente dice no tener una clave global que permita desanonimizar a todos. | Comparable directo para la propuesta de infraestructura compartida y ASP. La GVK de Shield supone un modelo de confianza distinto: un auditor autorizado puede ver el pool entero. No es una característica que todos los clientes preferirán. |
| Lendara Shield | Referencia en testnet: ASP, pool SPP, modo Direct y modo Ramp con ancla SEP-24 → intake → pool → transferencia privada; auditoría con GVK. | Propuesta para Stellar que combina operación del pool, entrada fiat y auditabilidad. La integración con una wallet externa y el SDK estable son trabajo a validar, no alianzas ya conseguidas. |

La documentación actual de Privacy Pools v2 sitúa su quickstart en Sepolia y dice que el SDK todavía requiere acceso de partners. No mezclar funcionalidades v2 con ingresos o despliegues v1 para presentar un solo producto como si todo estuviera disponible en mainnet.

### Ingresos observados

La página de DefiLlama de Privacy Cash consultada devolvió **USD 123.198 de fees y de protocol revenue en los últimos 30 días**. Es una observación del agregador para el protocolo, no una cifra de Solana aislada, una auditoría contable ni beneficio neto. La ventana cambia diariamente y no se reprodujo aquí desde transacciones on-chain.

La conclusión utilizable es que existe actividad que genera comisiones en esta categoría. La proyección para Shield debe calcularse con sus propios clientes, volumen, tarifa retenida y costos.

En Nullmask se confirmó el esquema de comisiones anunciado, pero no se fijó una cifra de ingresos comparable: la extracción de la página dinámica no permitió comprobar de forma estable los totales y sus períodos. No usar el millón mensual de las notas como dato verificado. También habría que separar comisiones de plataforma de recompensas de tokens si se usa su analítica.


### Qué diferencia puede defender Shield

La oportunidad propuesta combina tres elementos: adaptación a Stellar y sus rampas SEP-24, operación compartida del ASP y auditoría autorizada del pool. Son características para validar con el cliente objetivo, no prueba de que Shield sea el primero, el único o el más privado.

Privacy Pools es especialmente útil para contrastar el modelo de confianza: su divulgación controlada por el usuario difiere de la GVK de Shield, que da visibilidad global al auditor. Esa diferencia puede ser apropiada para ciertos clientes y poco atractiva para otros.

Las funciones y tarifas de la tabla provienen de las páginas de los equipos; no se auditaron sus contratos ni implementaciones. La cifra de ingresos es una observación del agregador, no una proyección para Shield.

### Notas originales que siguen pendientes de verificar

Se conservan como pistas de investigación, sin usarlas como hechos en la postulación:

- Privacy Cash: más de USD 120 millones en sus primeros 100 días y USD 95.000–120.000 en ingresos de 30 días, atribuidos inicialmente a Solanacompass sin URL ni fecha. La observación actual de la tabla tiene su propia ventana y no comprueba esas cifras históricas.
- Nullmask: cerca de USD 1 millón mensual, atribuido a Spectreai sin enlace exacto. La clasificación inicial como Solana/BSC no está respaldada por la documentación consultada, que describe Ethereum y Base.
- RAILGUN: ingresos de cientos de miles mensuales y referencias a Proof of Innocence/OFAC sin fuente exacta. Falta verificar métricas y mecanismos antes de compararlos con el ASP de Shield.
- Privacy Pools: miles o decenas de miles mensuales y posible yield de Aave. Deben identificarse versión, red, período y mecanismo; no mezclar v1 y v2.
- Otros nombres para investigar: Hinkal, Privara, Tempo Zones, Silent Data y BenPay. No se verificaron aquí sus productos o modelos comerciales.

## 5. Modelo de negocio propuesto para Shield

Las opciones a validar son una comisión pequeña sobre operaciones completadas o volumen y un cobro mensual por infraestructura, soporte u operación para integradores. Cualquier reparto con la wallet debe negociarse.

**Ingresos = integradores que pagan × abono mensual + volumen sujeto a comisión × tarifa retenida.**

Para estimar sostenibilidad hay que restar costos de pruebas, transacciones, RPC e historial, screening, soporte, gestión de claves y revisiones. Separar comisiones cobradas, pagos a partners, ingresos retenidos y beneficio. Depósitos, TVL y volumen no son ingresos; tampoco corresponde contar dos veces la misma operación.

El rendimiento sobre saldos ociosos surgió como posibilidad futura. No está implementado ni debe financiar el caso base: requiere verificar mecanismos del pool, liquidez y riesgos. No se aportaron ingresos comprobados, tarifas acordadas ni una base de usuarios de Shield. Toda proyección será hipotética hasta contar con datos del piloto.

## 6. Propuesta para SCF e hitos

> Los protocolos de privacidad de otras redes ofrecen antecedentes de monetización por comisiones e integración con wallets. Privacy Cash y su uso documentado por Hush ilustran esa relación entre infraestructura y producto final; Privacy Pools aporta otro antecedente de ASP y SDK para aplicaciones. Lendara Shield busca validar esa oportunidad en Stellar, combinando la operación de un pool de Stellar Private Payments con entrada fiat vía SEP-24 y auditoría autorizada. El financiamiento propuesto permitiría convertir el prototipo testnet en una integración reproducible y validar un piloto con una wallet.

El trabajo a financiar debe quedar concreto: autenticación de integradores, SDK estable, responsabilidades de elegibilidad y auditoría, recuperación de operaciones y un piloto reproducible. Especificar el alcance del relayer y la mitigación de correlación, en lugar de resumirlos como “listo para producción”.

| Hito propuesto | Entregable | Evidencia de aceptación |
|---|---|---|
| 1. Alcance con una wallet | Requisitos, responsabilidades, API autenticada y contrato del SDK | Alcance confirmado por el partner, controles de autorización/aislamiento y firma documentada |
| 2. Paquete de integración en testnet | SDK TypeScript, ejemplo y documentación de registro, saldo, depósito, transferencia, retiro y acceso de auditoría | Otro desarrollador reproduce los flujos, incluidos errores y reintentos |
| 3. Piloto dentro de una wallet | Saldo privado y operaciones desde su experiencia | Integración aceptada por el partner y transacciones reproducibles; distinguirla de conectar a una dapp externa |
| 4. Preparación operativa y de privacidad | Gestión de claves e historial, alcance del relayer, mitigación de correlación y plan de revisión | Mediciones de costo, latencia y confiabilidad; límites documentados y plan de remediación |

Son propuestas sujetas a presupuesto y validación, no compromisos ni etapas impuestas por SCF. Mainnet depende de la madurez de SPP, revisión de seguridad y requisitos operativos aplicables.

Medir integradores independientes, registros elegibles, pagos completados, tasa de éxito, latencia de prueba/confirmación, recuperación sin duplicados, costo por flujo, repetición de uso y aceptación del partner. Falta establecer la línea base.

## 7. Investigación con Raven y wallets candidatas

Se usaron `mcp__stellar_raven__search` y `mcp__stellar_raven__execute`:

- `lumenloop.find_similar_scf_submissions` devolvió tres coincidencias semánticas de Lightsail hardware wallet integration, correspondientes a Public Goods Q4 2025, Q3 2025 y Q1 2026. Son antecedentes adyacentes; no se leyeron aún las propuestas completas ni se comprobaron premios o montos.
- `scout.searchProjects` permitió inspeccionar 12 de 67 registros clasificados como wallets activas. La muestra no es un ranking completo. Una consulta adicional profundizó en xBull.
- `lumenloop.search_content_semantic` encontró fuentes sobre SCF, seguidas luego hasta páginas oficiales. La comparación internacional se hizo con navegación web y documentación de producto.

| Candidata exploratoria | Motivo para investigarla | Fuente y límite |
|---|---|---|
| xBull / Creit Tech | Posible integración orientada a desarrolladores; el directorio describe wallet web/extensión y firma Soroban | [Directorio](https://stellarlight.xyz/project/xbull), [sitio](https://xbull.app/), [repositorio](https://github.com/Creit-Tech/xBull-Wallet). Estado fechado 2026-08-29; commit principal indexado de 2025-08-13. Verificar mantenimiento, APIs de firma y licencia. |
| Beans | Posible encaje con pagos y entrada/salida de fondos | [Directorio](https://stellarlight.xyz/project/beans), [sitio](https://beansapp.com/). Estado fechado 2026-09-02; sin compatibilidad o interés comercial comprobados. |
| Decaf | Posible encaje con fiat y USDC | [Directorio](https://stellarlight.xyz/project/decaf), [sitio](https://decaf.so/). Estado fechado 2026-09-07; sin compatibilidad o interés comercial comprobados. |

Son candidatas, no partners. No se contactó a nadie ni se verificaron auditorías o aptitud de despliegue. La conexión con Freighter de la referencia no implica integración dentro de Freighter. Algunas afirmaciones de Raven sobre cambios de nombre de wallets no se corroboraron y no deben reutilizarse.

## 8. Convocatoria SCF y decisiones pendientes

La [página oficial de awards](https://communityfund.stellar.org/awards) consultada mostraba **SCF #46, cierre el 9 de noviembre de 2026**. El resumen indexado por Raven decía 8 de noviembre: resolver esa discrepancia leyendo el detalle oficial y confirmar hora, zona horaria y portal antes de enviar.

El [anuncio oficial de SCF v7](https://stellar.org/blog/ecosystem/introducing-scf-v7) es una fuente para revisar reglas de los tracks. Todavía no se determinó la elegibilidad de Shield ni el track adecuado. El monto solicitado debe surgir del trabajo y la capacidad del equipo.

Pendientes para continuar:

1. Guardar preguntas, límites y requisitos vigentes de la convocatoria; confirmar cierre y track.
2. Comprobar la demo y el historial del pool, y renovar la evidencia testnet si corresponde.
3. Validar requisitos e interés de una wallet; definir el criterio de aceptación del piloto.
4. Leer propuestas SCF completas relevantes usando Raven y fuentes oficiales.
5. Completar las verificaciones de cifras competitivas y sus fechas; usar solo las sustentadas.
6. Costear los hitos, asignar responsables y acordar cronograma, tarifa experimental y monto solicitado.
7. Redactar las respuestas finales en inglés cuando el formulario lo requiera y revisarlas con el equipo.

No se enviaron postulaciones, mensajes a terceros, commits, pushes ni despliegues durante este trabajo.

## 9. Referencias de Lendara y evidencia técnica

**Equipo según AGENTS.md:** Anouk (Costa Rica), Karu y Elisa (Argentina). Construido para HackMeridian 2026. El repositorio indica Genesis; una entrega antigua usa “Stellar Composed”. Confirmar el nombre final del track antes de reutilizarlo.

**Tecnología:** SPP de Nethermind, Soroban, Groth16, SEP-1/10/24 y SEP-53; API Rust/Axum, Postgres, CLI Rust del pool, contrato OpenAPI, tipos TypeScript generados y app Next.js en inglés y español. Versión SPP fijada en el repositorio: `sdk-v0.2.1`.

- [Demo en inglés](https://lendara-shield.duckdns.org/en) y [en español](https://lendara-shield.duckdns.org/es), solo testnet.
- [Documentación para desarrolladores](https://lendara-shield.duckdns.org/en/docs) y [referencia API](https://lendara-shield.duckdns.org/api-reference.html).
- [Repositorio público](https://github.com/Lendara-Org/lendara-shield); ubicación local: `/home/elisa/lendara-shield`.
- [README](../README.md): transacciones Ramp y Direct del 26 de octubre, enlaces al explorador, contratos y arquitectura.
- [Verificación del 28 de octubre](testing/2026-10-28-testnet-verification.md): evidencia histórica, no garantía de disponibilidad actual.
- [Modelo de amenazas](threat-model.md), [borrador del protocolo](protocol/shield-protocol.md), [OpenAPI](../openapi.yaml) e [instrucciones del repositorio](../AGENTS.md).

El nivel 1 es la integración como middleware; el nivel 2 de integración nativa con anclas es un borrador. No presentarlos como igualmente implementados.

El README advierte que el pool necesita todo su historial y el RPC público conserva aproximadamente siete días. Estimaba que este despliegue salía de esa ventana alrededor del 2 de noviembre de 2026. Revisar bootnode/historial o necesidad de redespliegue antes de otra demo; no se modificó el despliegue aquí.

El sandbox del ancla está en `/home/elisa/Lendara-monorepo`, un proyecto separado. Material anterior conservado: `/home/elisa/Downloads/submission-lendara-shield.md`, `/home/elisa/Downloads/lendara-shield-pitch-deck.pptx` y `/home/elisa/Downloads/lendara-shield-v2-voz-30fps.mp4`.

### Fuentes para la comparación internacional

- [Privacy Cash: documentación enlazada desde su sitio oficial](https://privacycash.mintlify.app/)
- [Privacy Cash: SDK en la organización oficial](https://github.com/Privacy-Cash/privacy-cash-sdk)
- [Hush: descripción de la wallet](https://hush.so/docs)
- [Hush: costos y uso de Privacy Cash](https://hush.so/docs/fees)
- [Nullmask: producto y redes documentadas](https://nullmask.io/docs)
- [Nullmask: comisiones anunciadas en analíticas](https://app.nullmask.io/analytics)
- [RAILGUN: costos y comisiones](https://docs.railgun.org/community-faqs/readme/costs-and-fees)
- [Privacy Pools: ASP, SDK y modelo de divulgación](https://privacypools.com/)
- [Privacy Pools v2: quickstart y acceso al SDK](https://privacy-pools-v2-docs.vercel.app/introduction/quickstart)
- [DefiLlama: métricas de Privacy Cash](https://defillama.com/protocol/privacy-cash)


## 10. Instrucción para retomar el trabajo

> Leé `/home/elisa/lendara-shield/docs/scf-context.md` y `AGENTS.md`. Continuá la preparación de la postulación SCF usando Raven y fuentes oficiales. El objetivo es validar Shield como infraestructura sostenible de pagos privados integrada en una wallet existente. Usá los antecedentes de otras blockchains para fundamentar producto, distribución e ingresos; distinguí el prototipo implementado, el trabajo propuesto y las cifras aún no verificadas. Mantené este único documento en español, sin duplicados. Empezá por los requisitos de la convocatoria y la definición del piloto.
