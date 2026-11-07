---
description: Cómo se prueba Lendara Shield, desde los tests unitarios hasta las corridas reales en testnet.
---

# Plan de pruebas

Cada cambio llega con tests de lo que agrega, incluidos sus casos de error. Los cambios que tocan la red (pool, ASP, pagos) llegan además con transacciones reales en testnet.

## Niveles de prueba

| Nivel | Qué cubre | Cómo |
| --- | --- | --- |
| Unitario | Dominio puro: transiciones de los intents, reglas del ASP, tipos validados, montos, emparejamiento de memos, validación del challenge SEP-10, firmas SEP-53 | `#[test]` de Rust junto al código |
| Base de datos | Almacenamiento, restricciones y transacciones | `#[sqlx::test]`, una base de PostgreSQL aislada por test |
| HTTP | Cada endpoint: éxito, 400, 401, 404, 409 y 503 | El router real con puertos falsos |
| Adaptadores | Horizon, Soroban RPC, el anchor (SEP-1/10/24) y `pool-cli` | `wiremock` con respuestas públicas grabadas de testnet; un CLI falso |
| Worker | Idempotencia ante fallas: envíos ambiguos, transacciones perdidas y rechazadas, atraso del RPC y una mezcla caótica | Una cadena falsa que aplica la regla de secuencia de Stellar; cada test exige exactamente un depósito y una transferencia |
| Web | Lógica, componentes y llamadas a la API | Vitest, Testing Library y MSW; un smoke test de Playwright en un navegador real |
| Testnet | Cuentas, trustline, contrato del activo, configuración del pool, verificación de SPP, bootnode, anchor sandbox | `make test-testnet` y tests de Rust ignorados que corren contra testnet |

Metas de cobertura: al menos 90 % en los módulos de dominio, intents y ASP, y al menos 75 % en total.

## Integración continua

Cada pull request corre formato, lint, chequeo de tipos, todas las suites de tests, chequeos de licencias y vulnerabilidades de las dependencias de Rust (cargo deny), el build, el smoke test de Playwright, un chequeo de que el contrato OpenAPI y los tipos generados coinciden, un escaneo de secretos (gitleaks) y un lint de workflows y scripts (actionlint, shellcheck).

## Corridas en testnet

Además de las suites automáticas, el equipo corre el producto de punta a punta en testnet y publica los resultados con hashes de transacciones:

| Fecha | Reporte | Resultado |
| --- | --- | --- |
| 2026-10-28 | [Verificación en testnet](https://github.com/Lendara-Org/lendara-shield/blob/develop/docs/testing/2026-10-28-testnet-verification.md) | Circuito de punta a punta, configuración, errores y recuperación; 2 hallazgos |
| 2026-11-06 | [Verificación en testnet, sección 6](testnet-verification.md) | Todos los flujos en el pool redesplegado, con hashes |
| 2026-11-06 | [Latencia del RPC](https://github.com/Lendara-Org/lendara-shield/blob/develop/docs/testing/2026-11-06-rpc-latency.md) | La lentitud del shielding, rastreada a la latencia del RPC público |
| 2026-11-07 | [Auditoría exhaustiva](https://github.com/Lendara-Org/lendara-shield/blob/develop/docs/testing/2026-11-07-exhaustive-audit.md) | Auditoría completa del producto y de caja negra (299 tests de Rust, 143 de web y 13 de Playwright en verde; cobertura 90,6 %); 7 hallazgos, todos corregidos con tests de regresión |

## En cada tramo del SCF

Un entregable está completo solo cuando tiene tests de sus casos de éxito y de error, el CI está en verde y, si toca la red, se publican transacciones reales en testnet (o en mainnet). El tramo 2 amplía el modelo de amenazas a un análisis al estilo STRIDE con un plan de monitoreo; el tramo 3 repite la corrida de punta a punta en mainnet.
