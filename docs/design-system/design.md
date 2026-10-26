# Lendara · Aplicación visual al frontend existente
Entrega: 26 octubre 2026. Fuente: archivo Lendara en Paper, páginas «Actual». Tokens: hash `1624c15b`.
Documento en español; nombres de componentes y tokens en inglés para facilitar implementación.

## 1. Objetivo y alcance
Aplicar el design system de Lendara al frontend que la dev ya construyó:
[Frontend existente](https://64-226-124-221.sslip.io/es).

**El sitio existente es la base de producto. Paper es la referencia de estilo, no un pedido de implementar sus pantallas.**

Conservar las pantallas, secciones, orden del contenido, navegación, rutas, textos, formularios, flujos, lógica y conexiones actuales. El trabajo es adaptar colores, fuentes, jerarquía tipográfica, bordes, radios, espaciado local, iconos y estados visuales de los componentes existentes. No añadir funcionalidades ni replicar composiciones de Paper.

Fuentes del sistema visual:
- [Foundations / Actual](https://app.paper.design/file/01M5EDT8K1V1BYVBPFXCDD4M1W/p-8-0): tokens, tipografía, color, iconos y reglas visuales.
- [Brand Guidelines / Actual](https://app.paper.design/file/01M5EDT8K1V1BYVBPFXCDD4M1W/p-F-0): marca y logo.
- [Biblioteca de componentes / Actual](https://app.paper.design/file/01M5EDT8K1V1BYVBPFXCDD4M1W/p-P-0): apariencia y estados, solo para componentes que ya existan en el frontend.

No tomar landing, dashboard, checkout, Docs ni Shield de Paper como pantallas a reconstruir. Si una regla exige cambiar la composición o el comportamiento del sitio, dejarla fuera de esta aplicación visual y consultarla por separado.
El sitio no pudo inspeccionarse con la herramienta web en esta entrega; este documento define el alcance y no afirma haber auditado su implementación.

## 2. Archivos e integración
1. Importar `tokens.css` una vez, antes de los estilos de componentes.
2. Cargar Sora 600; Inter 400/500/600; Geist Mono 400/500 con el mecanismo de fuentes del proyecto. Los binarios no están incluidos. Usar fallbacks sans-serif/monospace y evitar pesos sintéticos.
3. Aplicar `components.css` como referencia optativa o trasladar sus recetas al sistema de variantes del proyecto. No es una librería React lista para producción.
4. Reutilizar comportamiento existente (formularios, auth, enlaces, SDK); cambiar presentación sin reconstruir lógica funcional.
5. Si ya se usa shadcn/ui, aplicar el mapeo de abajo. No hace falta migrar todo ni instalarlo para usar estos tokens.

`tokens.json` conserva nombres, tipos, descripciones y aliases exportados de Paper; no es un archivo W3C DTCG ni un tema shadcn listo para importar.
No se incluyen exportaciones JSX de pantallas: el frontend existente conserva su estructura y componentes. Las recetas CSS son orientativas y deben adaptarse a las clases actuales, sin reemplazar su markup ni su lógica.

## 3. Identidad y color
Dirección luminosa: blanco dominante, azul tinta para lectura, crimson para acciones. El naranja y los gradientes aportan expresión gráfica; no sustituyen estados semánticos.

| Rol | Token | Valor |
|---|---|---|
| Texto principal / superficies oscuras | --color-text-primary | #20344D |
| Texto secundario | --color-text-secondary | #51647A |
| Superficie principal | --color-surface-primary | #FFFFFF |
| Superficie secundaria | --color-surface-subtle | #F5F8FC |
| Separadores | --color-border-default | #D9E2EC |
| Borde de controles | --color-border-control | alias de texto secundario |
| Marca crimson | --color-brand-primary | #E22366 |
| CTA | --color-action-primary | #C21852 |
| CTA hover / pressed | --color-action-primary-hover / -pressed | #B4184D / #A61848 |
| Acento gráfico | --color-brand-secondary | #FF8A3D |
| Selección activa | --color-selection-fill | alias de texto principal |

**No confundir marca con acción:** el botón con texto blanco usa crimson oscuro. El borde claro sirve para separadores; los campos interactivos usan borde de control.
Secundario: blanco → fondo sutil en hover → fondo código en pressed, texto tinta. Selección activa: tinta → #2D4866 en hover → #16263B en pressed, texto blanco.
Disabled: fondo #E3E8EF, texto #51647A. No bajar la opacidad de todo el control: `--opacity-disabled` es solo para elementos no textuales.

| Estado | Texto/icono | Fondo badge | Franja alert |
|---|---|---|---|
| Success | #2C4A12 | #BBF87E | #75B52E |
| Info | #24586D | #96E1FA | #279EC5 |
| Warning | #544211 | #FEE55A | #D0A400 |
| Error | #731C15 | #FBD0CB | #D94B3D |

Badges: combinar primer plano y fondo del mismo estado, siempre con etiqueta explícita.
Alerts: superficie blanca y acento del estado; texto/icono oscuros. No introducir paletas distintas entre dashboard, checkout y Shield.

## 4. Tipografía
| Uso | Familia/peso | Tamaño / interlineado |
|---|---|---|
| Display general desktop / mobile | Sora 600 | 64/68 · 40/46 |
| Sección desktop / mobile | Sora 600 | 44/50 · 32/40 |
| Título producto desktop / mobile | Sora 600 | 36/44 · 30/38 |
| Bloque H3 | Sora 600 | 24/32 |
| Lectura | Inter 400 | 16/24–28 |
| UI | Inter 400–600 | 14–16/20–24 |
| Ayuda de campo | Inter 400 | 14/22 |
| Código landing / producto | Geist Mono 400–500 | 14/23 · 13/23 |
| Importes | Inter 400–600 | según jerarquía, tabular-nums |

Titulares con tracking cerrado (tokens -0.025em y -0.045em); texto y código a 0.
Los tamaños de la tabla son roles visuales; elegirlos según la jerarquía del sitio existente. No trasladar tamaños o anchos particulares del hero de Paper.
El H1 producto mobile 30/38 está documentado en Foundations, pero no tiene token propio: usar una receta local, sin alterar el token de 32px.
Para importes usar `font-variant-numeric: tabular-nums`, moneda explícita y formato consistente con el idioma. Identificadores en mono, seleccionables; acortar visualmente sin truncar el valor que se copia.

## 5. Espaciado, superficies y responsive
Base de 4px. Escala exportada: 4, 8, 12, 16, 20, 24, 32, 40, 48, 56, 64, 80, 96.
- Dentro de controles: 8–16px. Entre grupos: 24–32px. Entre secciones: 56–96px.
- Controles: radio 10px; algunas variantes 12px. Cards producto: radio 16px, padding 24px desktop / 20px mobile.
- Marketing: cards 24px, visuales 32px, CTA pill 999px. No convertir todos los botones de producto en pills.
- Los tokens de contenedor son referencias de diseño, no instrucciones para reemplazar los anchos actuales del sitio.
- 375/390 y 1440 son viewports de referencia, no anchos obligatorios del sitio.
- Breakpoints del sistema: 640 / 768 / 1024px. El límite 520px pertenece al loader/widget.
- Conservar los cambios de layout responsive actuales. Los umbrales de Foundations son orientativos; no reestructurar columnas ni navegación para imitar Paper. Ajustar espaciado local sin provocar desbordes.
- Tablas y código pueden tener scroll horizontal propio; evitar scroll horizontal de toda la página. Importes alineados a la derecha; iconos y acciones en slots de ancho fijo.
- No copiar alturas fijas de artboards como alturas de páginas. Contenido y traducciones deben poder crecer.

## 6. Componentes y estados
Las medidas siguientes salen de la biblioteca vigente. Aplicar las variantes solo a componentes existentes. Esta tabla no es un backlog de componentes o estados nuevos. Las recomendaciones de accesibilidad sirven para preservar la interacción al cambiar estilos; no autorizan reescribir flujos.

| Componente | Base / comportamiento | Estados requeridos |
|---|---|---|
| Button primary | 48px alto, padding horizontal 20px, radio 10px, Inter 16/24 600 | default, hover, pressed, focus-visible, loading, disabled |
| Button secondary / ghost | 48px; secondary con borde de control, ghost sin caja dominante | mismos 6 estados |
| Field + Input | 48px, padding horizontal 12px, radio 10px, borde 1px; label visible | default, hover si aplica, focus, error, error+focus, disabled, read-only, valid |
| NativeSelect | conservar select nativo y su interacción de teclado | default, hover, focus, error, disabled, open ilustrativo |
| RadioGroup | selección única; tinta para seleccionado, no crimson de CTA | selected/unselected, hover, pressed, focus, disabled, error |
| IconButton | área 48px; icono 20px | default, hover, pressed, focus, loading, disabled |
| CopyButton | 48px habitual; compacto 40px | reposo, éxito real, error, focus |
| StatusBadge | etiqueta + icono y colores semánticos | completada, procesando, requiere acción, fallida, vencida |
| Alert | mensaje claro + siguiente acción disponible | info, warning, error; success según contexto |
| Card | borde sutil, radio 16px, padding 24/20px | no crear sombra decorativa por defecto |
| QuoteSummary | importe, comisión, tasa y resultado sin ambigüedad | ready, loading, error |
| Transactions | tabla desktop; lista mobile; paginación | ready, empty, loading, error, filtros sin resultados |
| Navigation | header, proyecto, menú mobile | actual, hover, focus, abierto/cerrado |

**Foco:** outline 2px tinta y offset 3px. El error conserva el anillo de foco; borde rojo y foco son señales distintas.
**Formularios:** label real; ayuda y error asociados con aria-describedby; aria-invalid al fallar. En submit inválido, enfocar el primer campo con error. Read-only sigue siendo seleccionable y accesible; disabled no.
**Loading:** conservar espacio de etiqueta, impedir envío duplicado y anunciar estado. Mostrar éxito solo tras respuesta confirmada.
**Copy:** anunciar con aria-live polite después del éxito real, durante 2 segundos. Si falla, permitir selección/copia manual.
**Alert:** role=alert para error que necesita atención; role=status/polite para información no urgente. Mantener datos previos si solo falla su actualización. Mostrar “Volver a la app” únicamente si existe cierre o destino válido.
**Tablas:** encabezados semánticos, acciones con nombres accesibles, paginación operable con teclado. No inventar status o fechas si falta información.

## 7. Componentes de privacidad existentes
Si el frontend ya contiene badges o controles de privacidad, aplicarles los mismos tokens, tipografía, radios y estados visuales del resto del sitio. Conservar etiquetas, permisos y comportamiento actuales.
No crear app shell, vistas de auditoría, reveals, pasos de depósito, selectores de escenarios ni diálogos nuevos a partir de los tableros Shield de Paper. Tampoco cambiar el contrato de la API.

## 8. Mapeo a shadcn/ui (propuesta de implementación)
| Lendara | Base sugerida | Adaptación |
|---|---|---|
| Button | Button | variantes primary/secondary/ghost; alturas y foco Lendara |
| Field/Input | Label + Input + mensaje | ayuda/error y estados read-only |
| País | select HTML nativo | conservar nativo; no migrarlo a custom sin necesidad |
| Moneda | RadioGroup | opciones con selección única, no Tabs |
| Card, Alert, Badge | equivalentes | aplicar semántica Lendara; no colores default |
| CopyButton | Button + Clipboard API | resultado real y anuncio accesible |
| Dialog | Dialog | validación de negocio y foco |
| Actividad existente | Table + Pagination | estilos; conservar layout y comportamiento actuales |
| Loading | Skeleton / indicador | respetar reduced motion; no simular progreso financiero |

Mapear --background, --foreground, --card, --primary, --primary-foreground, --muted, --muted-foreground, --border, --input, --ring y --destructive a los roles Lendara. Revisar si el tema instalado acepta colores completos o exige canales HSL; no pegar hex dentro de una función hsl().
No redefinir globalmente `--spacing` ni borrar la escala Tailwind. Los breakpoints CSS var() no funcionan dentro de media queries nativas: escribir 640/768/1024px o configurarlos en el build.
Este mapeo es una recomendación; no se verificó la versión actual de dependencias de la dev.

## 9. Logo, iconos y movimiento
Logo: usar assets vectoriales. Lockup maestro 160.3704 × 35, símbolo 24 × 35, gap 10px, Sora 600 con tracking 0. Área libre: media altura del símbolo por lado. Ancho mínimo recomendado 128px; debajo usar símbolo y revisar legibilidad. No deformar ni añadir sombras, contornos o gradientes al logo.
Los SVG con contornos evitan depender de una fuente para renderizar la firma.
Iconos: set outline, viewBox 24×24, stroke 2px, extremos redondeados. 20px dentro de controles y área interactiva de 48px. Preferir SVG currentColor; decorativos aria-hidden; acciones sin texto con nombre accesible. No emojis ni mezclar familias.
Motion de Foundations: hover/foco 150–200ms; entradas 300–450ms. Reduced motion: estado inmediato o fondo estático.
Gradientes gráficos: Impulso crimson → coral → apricot; Calma rosa suave → peach → fondo sutil. El shader documentado usa speed 1, scale 1, distortion .8, swirl .1. Es una opción gráfica, no una obligación en cada pantalla ni feedback de pago.

## 10. Criterios de revisión para la dev
- Comparar el antes y después del frontend existente: misma estructura, copy y flujos; nueva apariencia consistente con Foundations.
- Comprobar 375, 390, 768, 1024 y 1440px y anchos intermedios; zoom 200%; contenido largo.
- Recorrer teclado, foco visible, mensajes de error, lectura de labels y apertura/cierre de dialogs.
- Revisar los estados existentes de botones/campos y de carga/error de datos después de aplicar estilos.
- Comprobar contraste de pares usados, especialmente texto pequeño y controles. No afirmar conformidad completa por tener tokens.
- Verificar que links, copies, auth y transacciones existentes sigan funcionando tras aplicar estilos.
- Conservar las etiquetas de red y avisos funcionales existentes.
- No sustituir copy ni snippets por ejemplos de Paper; mantener los del frontend existente.

## 11. Alcance y mantenimiento
Entrega para aplicar identidad visual al frontend existente. No incluye reconstrucción de pantallas, cambios de contenido, nueva funcionalidad ni auditoría de accesibilidad.
Tokens exportados sin renombrar ni modificar valores; las recetas CSS son una traducción utilizable de la biblioteca y están separadas del export.
Si un componente necesita un rol nuevo, documentarlo y sumarlo al origen en Paper antes de duplicar colores o crear variantes ad hoc.

