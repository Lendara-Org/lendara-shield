---
description: Corre Lendara Shield en tu máquina contra la testnet de Stellar.
---

# Ejecutar en local

Requisitos: Node 24, pnpm 12, Rust stable, Docker, el [`stellar` CLI](https://developers.stellar.org/docs/tools/cli) y [Freighter](https://www.freighter.app/) configurado en **Testnet**.

```bash
cp .env.example .env
make install
make db-up
make setup-testnet        # funds Shield's accounts, deploys the pool and initialises the intake
                          # (idempotent); copy the variables from
                          # ~/.local/share/lendara-shield/testnet/testnet.env into .env
make build-pool-cli
make dev                  # API on :8080, web on http://localhost:3000
```

Luego define `ADMIN_TOKEN` y `AUDITOR_TOKEN` en `.env` (`openssl rand -hex 32`). Por defecto, el modo rampa usa el anchor público del sandbox de Lendara (`RAMP_ANCHOR_HOME_DOMAIN`); déjalo vacío para correr solo con el modo directo.

Revisiones: `make lint`, `make test` (necesita Postgres), `make test-web-e2e`, y `make test-testnet` para revisiones de solo lectura contra testnet.

## Demo en 5 minutos

{% stepper %}
{% step %}
### Registro

Abre `http://localhost:3000/en/onboard` (o `/es/…` en español) y conecta Freighter (Testnet). Elige el escenario de elegibilidad **Aprobado** y haz clic en *Derivar llaves y registrarme*. Freighter firma dos veces: una para derivar tus llaves del pool en el navegador y otra para probar que la cuenta es tuya. Luego Shield agrega tu hoja a la allow-list.
{% endstep %}

{% step %}
### Deposita con rampa

En `/en/deposit`, elige *Con rampa*, ingresa `CRC 5000` y continúa. En la ventana del anchor, paga con la tarjeta de prueba **4242 4242 4242 4242** (cualquier fecha futura y cualquier CVC). De vuelta en Shield, la línea de tiempo se completa sola: el anchor le pagó a Shield → Shield depositó en el pool → transferencia privada a tus llaves.
{% endstep %}

{% step %}
### Revisa el explorador

Abre los tres enlaces: tu dirección no está en ninguno. El depósito muestra `ext_amount > 0` desde la cuenta de Shield. La transferencia no muestra nada más que la cuenta de Shield.
{% endstep %}

{% step %}
### Mira tu saldo privado

En `/en/wallet`, los USDC están ahí. Solo tú, y el auditor, pueden verlos.
{% endstep %}

{% step %}
### Auditoría

En `/en/audit`, pega `AUDITOR_TOKEN`. El explorador ve *"firmado por Shield, monto oculto, destinatario no visible"*; el auditor ve *"9.30 USDC para ti"*.
{% endstep %}
{% endstepper %}

Para probar el modo directo, elige *Desde mi wallet* en `/en/deposit`. Tu cuenta necesita USDC de testnet del sandbox y su trustline. Todos los resultados de elegibilidad y de fondeo están en [Escenarios](scenarios.md).
