import { A, C, Callout, CodeBlock, H2, H3, List, P, Table } from "@/components/docs/prose";
import { type DocChrome, type DocPage, type DocSlug, docHref } from "@/lib/docs";

import { CODE, REPO } from "./shared";

export const chrome: DocChrome = {
  docs: "Docs",
  previous: "Anterior",
  next: "Siguiente",
  menu: "Menú de docs",
  copy: ["Copiar", "Copiado"],
};

const code = (value: string, title?: string) => <CodeBlock code={value} copy={chrome.copy} {...(title ? { title } : {})} />;

export const pages: Record<DocSlug, DocPage> = {
  "": {
    title: "Introducción",
    description: "Una capa de privacidad con cumplimiento que lleva el dinero de cualquier anchor SEP-24 directo a un pool privado.",
    body: () => (
      <>
        <Callout title="Solo testnet" tone="warning">
          Shield corre en la testnet de Stellar sobre Stellar Private Payments, un developer preview con contratos sin
          auditar. No se mueve dinero real.
        </Callout>

        <H2 id="problema">El problema</H2>
        <P>
          Stellar tiene herramientas de privacidad, pero la privacidad se rompe en el borde fiat. Un depósito por rampa
          le paga a la dirección del usuario, con un monto y una hora, y cualquiera puede seguirlo hasta un pool privado
          y de vuelta. Además, un pool con cumplimiento necesita a alguien que haga el KYC, administre la allow-list (el
          ASP), guarde la llave de vista del auditor y pague las comisiones.
        </P>

        <H2 id="como">Cómo funciona Shield</H2>
        <List ordered>
          <li>La wallet del usuario deriva sus llaves del pool. Shield recibe solo llaves públicas, firmadas por la cuenta.</li>
          <li>Shield hace el KYC y, como ASP del pool, agrega al usuario a la allow-list on-chain.</li>
          <li>
            Shield abre un depósito SEP-24 estándar con el anchor, pagado a la <strong>cuenta ómnibus de Shield</strong>{" "}
            con un memo.
          </li>
          <li>Cuando el anchor confirma su pago, Shield deposita en el pool desde su propia cuenta.</li>
          <li>Shield transfiere el monto en privado a las llaves del usuario.</li>
        </List>
        <P>
          En el explorador solo se ve &quot;el anchor le pagó a Shield&quot; y &quot;Shield usó el pool&quot;. La
          dirección del usuario no aparece en ningún lado. El auditor, con la llave de vista, ve quién recibió qué.
        </P>

        <H2 id="modos">Dos modos</H2>
        <Table
          head={["", "Rampa", "Directo"]}
          rows={[
            ["De dónde viene el dinero", "Moneda local a través de un anchor SEP-24", "USDC en la wallet del usuario"],
            ["Dirección del usuario on-chain", "Nunca", "Al depositar, y como firmante de sus transferencias"],
            ["Necesita un anchor", "Sí, cualquier anchor SEP-24", "No"],
          ]}
        />

        <H2 id="para-quien">Para quién es</H2>
        <List>
          <li>
            <strong>Wallets y apps</strong> que quieren saldos privados para sus usuarios: integran el API (ver{" "}
            <A href={docHref("es", "quickstart")}>Inicio rápido</A>).
          </li>
          <li>
            <strong>Anchors</strong> que quieren ofrecer depósitos privados: el nivel 1 no les pide ningún cambio, y pueden
            adoptar el <A href={docHref("es", "protocol")}>Shield Protocol</A> de forma nativa.
          </li>
          <li>
            <strong>Auditores y reguladores</strong>: cada nota es visible con la llave de vista del pool (ver{" "}
            <A href={docHref("es", "security")}>Seguridad</A>).
          </li>
        </List>
      </>
    ),
  },

  protocol: {
    title: "Shield Protocol",
    description: "El nivel 1 funciona hoy con cualquier anchor SEP-24; el nivel 2 permite que los anchors depositen en el pool de forma nativa.",
    body: () => (
      <>
        <P>
          El Shield Protocol se apoya en <A href="https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0024.md">SEP-24</A>,{" "}
          <A href="https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0010.md">SEP-10</A>,{" "}
          <A href="https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0053.md">SEP-53</A> y un pool
          de <A href="https://developers.stellar.org/docs/build/apps/privacy">Stellar Private Payments</A> con allow-list,
          block-list y Global View Key. El nombre no lleva marca para que cualquier anchor lo pueda adoptar.
        </P>

        <H2 id="roles">Roles</H2>
        <Table
          head={["Rol", "Qué hace"]}
          rows={[
            ["Usuario", "Deriva sus llaves del pool en la wallet; las llaves de gasto nunca salen de ahí"],
            ["Anchor", "Rampa SEP-24: KYC, recibe el pago en fiat, paga el activo"],
            ["ASP", "Mantiene la allow-list y la block-list del pool"],
            ["Auditor", "Tiene la Global View Key y puede descifrar cada nota"],
            ["Shield (nivel 1)", "ASP + cuenta ómnibus + cliente SEP-24 frente a cualquier anchor"],
          ]}
        />

        <H2 id="nivel-1">Nivel 1: Shield como middleware (implementado)</H2>
        <H3>Registro</H3>
        <List ordered>
          <li>
            La wallet deriva la llave de nota, la de cifrado y la hoja del ASP. El usuario <strong>no</strong> se registra
            en el registro público de llaves del pool, porque esa transacción publicaría su dirección.
          </li>
          <li>La cuenta firma las llaves (SEP-53) y las envía al ASP.</li>
          <li>Tras el KYC, el ASP inserta la hoja en la allow-list. Bloquear es insertar la llave de nota en la block-list.</li>
        </List>
        <H3>Depósito</H3>
        <List ordered>
          <li>Shield se autentica con el anchor (SEP-10) como su cuenta ómnibus, con un memo <C>id</C> del usuario.</li>
          <li>
            Shield pide un depósito interactivo (SEP-24) a <strong>su cuenta ómnibus</strong>, con un memo <C>id</C>{" "}
            aleatorio del intent.
          </li>
          <li>El usuario paga en el widget del anchor; el anchor le paga a la cuenta ómnibus.</li>
          <li>
            Shield fondea el intent <strong>solo con la transacción que reporta el anchor</strong> (
            <C>stellar_transaction_id</C>), nunca solo porque un pago traiga el memo.
          </li>
          <li>Shield deposita en el pool (<C>ext_amount &gt; 0</C>) y luego transfiere en privado al usuario (<C>ext_amount = 0</C>).</li>
        </List>
        <Callout title="La idempotencia es parte del protocolo">
          Cada transacción del pool se prueba, se firma y su hash se guarda <em>antes</em> de enviarla. Un reintento
          revisa el hash guardado y reenvía el mismo sobre. Solo se prepara un reemplazo cuando la original ya no puede
          entrar.
        </Callout>

        <H2 id="nivel-2">Nivel 2: depósitos privados nativos (borrador)</H2>
        <P>El anchor deposita directo en el pool a las llaves del usuario, lo que elimina la custodia de Shield.</P>
        {code(`SHIELD_PROTOCOL_VERSION = "0.1"
SHIELD_POOL_CONTRACT = "C…"
SHIELD_ASP_ENDPOINT = "https://…"`, "stellar.toml")}
        {code(`{ "features": { "shielded_deposit": true } }`, "GET /info (SEP-24)")}
        {code(`POST /transactions/deposit/interactive
asset_code=USDC
destination_type=shielded
shielded_recipient=<note_public_key>:<encryption_public_key>`, "Depósito interactivo SEP-24")}
        <P>
          El borrador completo, con la liquidación, los retiros, las consideraciones de seguridad y las preguntas
          abiertas, está en <A href={`${REPO}/docs/protocol/shield-protocol.md`}>docs/protocol/shield-protocol.md</A>{" "}
          (en inglés).
        </P>
      </>
    ),
  },

  quickstart: {
    title: "Inicio rápido",
    description: "Dale a tus usuarios un saldo privado fondeado en moneda local, con unas pocas llamadas HTTP.",
    body: () => (
      <>
        <P>
          Es el mismo flujo que sigue la web de Shield. Necesitas el SDK de navegador{" "}
          <A href="https://www.npmjs.com/package/stellar-private-payments">stellar-private-payments</A> (0.2.1) para las
          llaves, y Freighter o cualquier wallet SEP-43 para las firmas.
        </P>

        <H2 id="info">1. Lee el despliegue</H2>
        {code(`const info = await fetch(\`\${SHIELD}/v1/info\`).then((r) => r.json());
// info.network_passphrase, info.pool.contract_id, info.modes.ramp.enabled`)}

        <H2 id="llaves">2. Deriva las llaves del pool del usuario</H2>
        <P>La wallet firma una vez para derivarlas. Se quedan en el navegador.</P>
        {code(CODE.keys)}

        <H2 id="registro">3. Registra al usuario con una firma</H2>
        <P>
          La cuenta firma exactamente este texto (SEP-53, <C>signMessage</C> de Freighter), con las llaves en hex en
          minúsculas. Sin la firma, el API responde <C>400 invalid_signature</C>.
        </P>
        {code(CODE.onboard)}
        <P>
          Consulta <C>GET /v1/users/{"{account}"}</C> mientras <C>asp_status</C> sea <C>pending</C>, hasta que quede{" "}
          <C>allowed</C>.
        </P>

        <H2 id="intent">4. Crea un intent de rampa</H2>
        {code(CODE.intent)}

        <H2 id="seguimiento">5. Síguelo hasta completed</H2>
        <P>Consulta cada 2 o 3 segundos. Los tres hashes aparecen a medida que cada paso entra on-chain.</P>
        {code(CODE.follow)}

        <H2 id="saldo">6. Muestra el saldo privado</H2>
        {code(CODE.balance)}
        <Callout title="Pruébalo sin código">
          Las páginas <A href="/es/onboard">Registro</A>, <A href="/es/deposit">Depositar</A> y{" "}
          <A href="/es/wallet">Wallet</A> de este sitio ejecutan exactamente este flujo contra testnet.
        </Callout>
      </>
    ),
  },

  api: {
    title: "API",
    description: "REST y JSON, definida en openapi.yaml. Solo testnet.",
    body: () => (
      <>
        <P>
          El contrato es <A href={`${REPO}/openapi.yaml`}>openapi.yaml</A>.{" "}
          <A href="/api-reference.html">Abrir la referencia completa de la API (en inglés) →</A>
        </P>

        <H2 id="endpoints">Endpoints</H2>
        <Table
          head={["Endpoint", "Autenticación", "Qué hace"]}
          rows={[
            [<C key="c">GET /v1/info</C>, "—", "Red, pool, activo y modos habilitados"],
            [<C key="c">POST /v1/onboarding</C>, "Firma SEP-53", "Registra las llaves, corre el KYC simulado y pide la allow-list"],
            [<C key="c">GET /v1/users/{"{account}"}</C>, "—", "Estado del KYC y de la allow-list"],
            [<C key="c">POST /v1/intents</C>, "—", "Crea un intent de rampa y devuelve la URL interactiva del anchor"],
            [<C key="c">GET /v1/intents/{"{id}"}</C>, "—", "Estado y hashes on-chain"],
            [<C key="c">POST /v1/asp/block</C>, <C key="t">ADMIN_TOKEN</C>, "Bloquea una llave (block-list)"],
            [<C key="c">POST /v1/asp/unblock</C>, <C key="t">ADMIN_TOKEN</C>, "Desbloquea una llave"],
            [<C key="c">GET /v1/audit/transactions</C>, <C key="t">AUDITOR_TOKEN</C>, "Actividad del pool descifrada con la llave de vista"],
            [<C key="c">GET /v1/audit/public/{"{tx_hash}"}</C>, "—", "Lo que cualquiera ve de una transacción"],
          ]}
        />

        <H2 id="errores">Errores</H2>
        <P>Cada error tiene un código estable:</P>
        {code(`{ "error": "user_not_allowed", "message": "…" }`)}
        <Table
          head={["Estado", "Códigos frecuentes"]}
          rows={[
            ["400", <><C>invalid_request</C>, <C>invalid_signature</C>, <C>invalid_key</C>, <C>invalid_limit</C></>],
            ["401", <C key="c">unauthorized</C>],
            ["404", <C key="c">not_found</C>],
            ["409", <><C>account_already_onboarded</C>, <C>pool_key_already_registered</C>, <C>user_not_allowed</C>, <C>ramp_mode_disabled</C></>],
            ["502 / 503", <><C>anchor_unavailable</C>, <C>service_unavailable</C></>],
          ]}
        />

        <H2 id="estados">Estados de un intent</H2>
        <Table
          head={["Estado", "Significado"]}
          rows={[
            [<C key="c">awaiting_anchor</C>, "El usuario tiene que pagar en el anchor"],
            [<C key="c">funds_received</C>, "El pago del anchor está confirmado y en custodia de Shield"],
            [<C key="c">shielding</C>, "Se está enviando el depósito al pool"],
            [<C key="c">transferring</C>, "Se está enviando la transferencia privada"],
            [<C key="c">completed</C>, "El saldo privado es del usuario"],
            [<><C>rejected</C>, <C>expired</C>, <C>error</C></>, "Final; ver message"],
          ]}
        />
      </>
    ),
  },

  scenarios: {
    title: "Escenarios",
    description: "Fuerza cada resultado en testnet: el KYC de Shield y las tarjetas de prueba del anchor.",
    body: () => (
      <>
        <H2 id="kyc">El KYC de Shield (sandbox)</H2>
        <P>
          Se elige con <C>scenario</C> en <C>POST /v1/onboarding</C>, o en el selector de la página de Registro.
        </P>
        <Table
          head={["Escenario", "Resultado"]}
          rows={[
            [<C key="c">approved</C>, "La hoja entra a la allow-list: el usuario puede operar"],
            [<C key="c">rejected</C>, "Nunca se inserta: el pool rechaza sus pruebas y los intents responden 409"],
            [<C key="c">blocked</C>, "Se inserta y luego se bloquea: queda congelado hasta que el ASP lo desbloquee"],
          ]}
        />

        <H2 id="tarjetas">Tarjetas de prueba del anchor (sandbox de Lendara)</H2>
        <P>Sirve cualquier fecha futura y cualquier CVC de 3 dígitos.</P>
        <Table
          head={["Tarjeta", "Resultado", "Intent"]}
          rows={[
            [<C key="c">4242 4242 4242 4242</C>, "Aprobada", <C key="s">completed</C>],
            [<C key="c">4000 0000 0000 0002</C>, "KYC rechazado", <C key="s">error</C>],
            [<C key="c">4000 0000 0000 9995</C>, "Pago no recibido", <C key="s">expired</C>],
            [<C key="c">4000 0000 0000 0077</C>, "Aprobada a los 30 s", <C key="s">completed</C>],
          ]}
        />

        <H2 id="dinero">Casos límite con dinero</H2>
        <List>
          <li>Un pago con el memo de otra persona que el anchor no reporta nunca fondea nada: queda en revisión.</li>
          <li>Los pagos tardíos o duplicados se guardan para revisión; nunca se aplican solos.</li>
          <li>Los timeouts, las transacciones perdidas y el retraso del RPC nunca pagan dos veces: primero se revisa el hash guardado.</li>
        </List>
      </>
    ),
  },

  security: {
    title: "Seguridad",
    description: "Qué es privado, frente a quién, y los límites que conocemos.",
    body: () => (
      <>
        <H2 id="quien-ve">Quién ve qué (modo rampa)</H2>
        <Table
          head={["Observador", "Ve"]}
          rows={[
            ["El público", "El anchor le pagó a Shield; Shield usó el pool. Ni la dirección del usuario ni quién recibió qué"],
            ["El anchor", "Al usuario y el monto (hace el KYC)"],
            ["Shield", "Qué cuenta es dueña de qué llaves (es el ASP), y cada intent"],
            ["El auditor", "Cada nota: dueño y monto"],
          ]}
        />

        <H2 id="protecciones">Protecciones</H2>
        <List>
          <li>Las llaves de gasto nunca salen del navegador del usuario.</li>
          <li>El registro exige la firma SEP-53 de la cuenta sobre las llaves exactas.</li>
          <li>Solo el pago que reporta el anchor fondea un intent; los memos de los intents son aleatorios.</li>
          <li>La llave de vista solo llega al proceso del auditor, nunca a un navegador.</li>
          <li>Operador, cuenta ómnibus y auditor son tres llaves distintas.</li>
          <li>Los logs nunca juntan la dirección de un usuario con un monto.</li>
        </List>

        <H2 id="limites">Límites conocidos</H2>
        <List>
          <li>El pago del anchor y el depósito de Shield comparten monto y hora, así que se pueden vincular (lo siguiente es agruparlos).</li>
          <li>En modo directo, el usuario firma sus propias transferencias, así que su cuenta aparece como usuaria del pool (lo resuelve un relayer).</li>
          <li><C>GET /v1/users/{"{account}"}</C> es público: dice si una cuenta usa Shield.</li>
          <li>El emisor del activo podría activar el clawback más adelante; no es <C>AUTH_IMMUTABLE</C>.</li>
          <li>Contratos de developer preview sin auditar; el RPC público guarda unos 7 días de historial.</li>
        </List>
        <P>
          Threat model completo (en inglés): <A href={`${REPO}/docs/threat-model.md`}>docs/threat-model.md</A>.
        </P>
      </>
    ),
  },
};
