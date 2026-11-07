---
description: Qué está hecho y qué construye cada tramo del SCF.
---

# Roadmap

Ambos modos ya corren de punta a punta en Stellar testnet. Los tres tramos llevan a Shield de un build funcional en testnet a mainnet.

## Hecho

**HackMeridian 2026 (25–26 de octubre):** spike de SPP, contrato OpenAPI, pool en testnet, onboarding y ASP, intents y cliente del anchor, CLI del pool, watcher y worker de shielding, app web en inglés y español, panel de auditoría y documentación. El modo rampa se completa en testnet sin que la dirección del usuario aparezca en ningún lugar on-chain.

**Correcciones de seguridad después del hackathon:**

* Fondeo confirmado por el anchor y onboarding firmado (SEP-53).
* Solicitudes de intent firmadas y de un solo uso; la URL interactiva solo en la respuesta de creación.
* Usuarios bloqueados detenidos a mitad de un intent; una espera acotada para pagos del anchor sin confirmar.
* Loops separados de rampa y de shielding; rate limit por IP; rutas del ASP con su propio timeout.
* Una Content-Security-Policy y verificaciones al arrancar sobre llaves y URLs.

## Tramos del SCF Build Award

| Tramo | Construye |
| --- | --- |
| **1 — Testnet lista para developers** | Hoja del ASP amarrada a la llave registrada (cierra el límite conocido 7) · relayer con su propia cuenta fuente y como sender de la llamada al pool (los usuarios del modo directo ya no son visibles como fuente ni firmantes de sus transferencias privadas) · API keys por proyecto, permisos acotados y sesiones, incluida la decisión de elegibilidad del integrador a través de su key · SDK de TypeScript generado desde `openapi.yaml` |
| **2 — Preparación para producción** | Shield Protocol nivel 2: especificación completa que cualquier anchor SEP-24 puede adoptar, con el anchor de Lendara como primera implementación de referencia (depósitos directos al pool, sin paso de custodia) · llaves de operador y ómnibus en un KMS/HSM con rotación; la llave de vista guardada cifrada y con registro de accesos (su parte pública queda fija por pool) · divulgación selectiva por nota para los usuarios y un portal de auditor basado en roles con registros de acceso · resolución de los fondos de usuarios bloqueados (reanudar, devolver al anchor o retener) con registro de auditoría · integración con una segunda fuente de fondeo · modelo de amenazas (al estilo STRIDE) y plan de monitoreo para el despliegue de producción |
| **3 — Mainnet** | Despliegue en mainnet con RPC de archivo, monitoreo y alertas; el modo rampa solo con el nivel 2, así que Shield no tiene fondos de usuarios en mainnet · refuerzo del anonymity set (agrupación, redondeo, demoras) · primera integración con una wallet de terceros |

## Después de este award: una capa de cumplimiento para las dos primitivas de privacidad

Stellar tiene dos herramientas de privacidad complementarias. **Stellar Private Payments** oculta quién le paga a quién; **Confidential Tokens** oculta montos y saldos, mientras las direcciones siguen visibles. Los tramos del SCF construyen el Shield Protocol primero sobre SPP. Después, la misma capa de cumplimiento (la decisión de elegibilidad del integrador, el rol de ASP y la divulgación solo para el auditor) se extiende a Confidential Tokens, para que wallets y anchors puedan ofrecer contrapartes privadas o montos privados con la misma API. Esto no forma parte del presupuesto solicitado.

Cómo encaja cada ítem en el sistema está en [Arquitectura técnica](technical-architecture.md). Los límites que resuelve cada uno están en el [Modelo de amenazas](threat-model.md).
