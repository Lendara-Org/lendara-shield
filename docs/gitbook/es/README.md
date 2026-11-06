---
description: KYC una vez, paga en privado, divulga de forma selectiva.
---

# Lendara Shield

Lendara Shield le da a cualquier producto en Stellar pagos privados con cumplimiento a través de una sola API. Wallets, anchors, apps de pago y usuarios individuales obtienen saldos privados, un Association Set que ellos controlan y divulgación solo para el auditor, sobre [Stellar Private Payments](https://developers.stellar.org/docs/build/apps/privacy). El dinero que llega desde una fuente externa, como cualquier anchor SEP-24, puede llegar a un saldo privado sin que la dirección del usuario aparezca nunca on-chain.

Es la implementación de referencia del **Shield Protocol**, un borrador abierto que cualquier wallet o anchor puede adoptar.

{% hint style="warning" %}
**Solo testnet.** Stellar Private Payments es una developer preview con contratos sin auditar. No lo uses con fondos reales.
{% endhint %}

<table data-view="cards"><thead><tr><th></th><th></th><th data-hidden data-card-target data-type="content-ref"></th></tr></thead><tbody>
<tr><td><strong>Arquitectura técnica</strong></td><td>Componentes, integración con Stellar, flujos, modelo de seguridad y tramos.</td><td><a href="technical-architecture.md">technical-architecture.md</a></td></tr>
<tr><td><strong>Shield Protocol</strong></td><td>Nivel 1 (implementado) y la extensión de SEP-24 para depósitos privados nativos.</td><td><a href="shield-protocol.md">shield-protocol.md</a></td></tr>
<tr><td><strong>Modelo de amenazas</strong></td><td>Qué es privado, frente a quién, y los límites honestos.</td><td><a href="threat-model.md">threat-model.md</a></td></tr>
<tr><td><strong>Roadmap</strong></td><td>Qué está hecho y qué construye cada tramo.</td><td><a href="roadmap.md">roadmap.md</a></td></tr>
</tbody></table>

## El problema

Stellar ya tiene herramientas de privacidad en developer preview, pero dos cosas todavía impiden que los productos las usen:

* **Un pool con cumplimiento necesita un operador.** Los privacy pools con allow-list necesitan a alguien que haga el KYC, mantenga la lista (el ASP), guarde la view key del auditor y pague las comisiones. Hoy cada app tendría que construir eso por su cuenta.
* **El fondeo expone al usuario.** Un depósito desde una rampa le paga a la dirección propia del usuario, con un monto y una hora. Cualquiera puede seguir esa dirección hacia dentro de un privacy pool y hacia fuera.

## Cómo funciona

Shield hace el trabajo de cumplimiento una sola vez, como ASP del pool. Los integradores deciden qué usuarios son elegibles; Shield escribe la allow-list y la block-list. En modo rampa, Shield recibe el pago de la fuente de fondeo en su **cuenta ómnibus**, deposita en el pool desde ahí y luego **transfiere en privado a las llaves del usuario**. El explorador solo ve "la fuente le pagó a Shield" y "Shield usó el pool". La view key del auditor muestra quién recibió qué.

| | **Rampa** | **Directo** |
| --- | --- | --- |
| Para | Personas que fondean desde una fuente externa, como un anchor | Personas que ya tienen USDC |
| El dinero viene de | Un anchor SEP-24 → cuenta ómnibus de Shield → pool | La wallet propia del usuario → pool |
| Dirección del usuario on-chain | **Nunca** | Visible al depositar, y como firmante de sus propias transferencias (hasta el relayer) |
| Qué hace Shield | ASP, recibir, depositar, transferir, auditar | ASP, auditar |

Ambos modos comparten el ASP, el cliente del pool, el auditor y la máquina de estados de los intents. El modo rampa habla con la fuente de fondeo **solo a través de los SEPs 1, 10 y 24**, así que funciona con cualquier anchor SEP-24.

## En vivo en testnet

| | |
| --- | --- |
| App web (inglés / español) | [lendara-shield.duckdns.org/en](https://lendara-shield.duckdns.org/en) · [/es](https://lendara-shield.duckdns.org/es) |
| Documentación para developers | [lendara-shield.duckdns.org/es/docs](https://lendara-shield.duckdns.org/es/docs) |
| Referencia de la API (desde `openapi.yaml`) | [api-reference.html](https://lendara-shield.duckdns.org/api-reference.html) |
| Repositorio | [github.com/Lendara-Org/lendara-shield](https://github.com/Lendara-Org/lendara-shield) |

Las transacciones reales en testnet de ambos modos están en [Arquitectura técnica, sección 9](technical-architecture.md#9.-lo-que-existe-hoy-testnet-verificable).

## Equipo

[Lendara](https://github.com/Lendara-Org):

* **Anouk Rímola** — [LinkedIn](https://www.linkedin.com/in/anouk-r%C3%ADmola-b9915a223/)
* **María Elisa Araya** — [LinkedIn](https://www.linkedin.com/in/arayamariaelisa/)
* **Karen Giannetto** — [LinkedIn](https://www.linkedin.com/in/karen-giannetto/)

Con licencia [Apache-2.0](https://github.com/Lendara-Org/lendara-shield/blob/main/LICENSE).
