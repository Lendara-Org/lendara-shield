# Assets
- lendara-logo-master.svg: SVG de entrega obtenido del activo maestro de Foundations en Paper el 26/10/2026. Contornos vectoriales; fondo transparente.
- lendara-logo-dark.svg: firma con texto oscuro, para fondo claro. Copia del paquete vectorial previo; mismo viewBox y colores que el maestro actual.
- lendara-logo-light.svg: firma con texto blanco, para fondo oscuro. Copia del paquete vectorial previo.
- lendara-symbol.svg: símbolo original crimson, sin wordmark. Copia del paquete previo.
- arrow-right, copy, external-link, menu, chevron-down, wallet, check, alert-circle: geometría exportada del tablero Icons actual; atributos JSX convertidos a SVG y stroke normalizado a currentColor para heredar color semántico.

El maestro usa texto #161616 en sus contornos, tal como el activo en Paper; no recolorear automáticamente a tinta. La UI usa --color-text-primary.
Las variantes con contornos no dependen de Sora instalada. Usar alt="Lendara" al incluir la firma como imagen y alt vacío cuando sea redundante con un nombre accesible existente.
No confundir los nombres dark/light: describen el texto del asset, no el fondo.

