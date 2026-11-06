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
| **1 — Testnet lista para developers** | Relayer con fee-bump (los usuarios del modo directo ya no son visibles como firmantes) · API keys por proyecto, permisos acotados y sesiones, incluida la decisión de elegibilidad del integrador a través de su key · SDK de TypeScript generado desde `openapi.yaml` |
| **2 — Preparación para producción** | Llaves de operador, ómnibus y de vista en un KMS/HSM con rotación · divulgación selectiva por nota para los usuarios y un portal de auditor basado en roles con registros de acceso · integración con una segunda fuente de fondeo |
| **3 — Mainnet** | Despliegue en mainnet con RPC de archivo, monitoreo y alertas · refuerzo del anonymity set (agrupación, redondeo, demoras) · primera integración con una wallet de terceros |

Cómo encaja cada ítem en el sistema está en [Arquitectura técnica](technical-architecture.md). Los límites que resuelve cada uno están en el [Modelo de amenazas](threat-model.md).
