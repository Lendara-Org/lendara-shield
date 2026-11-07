# Lendara · Design system para el sitio existente
Entrega ajustada: 26 octubre 2026.

## Alcance
Aplicar la identidad visual de Paper sobre https://64-226-124-221.sslip.io/es.
Conservar las pantallas, composición, navegación, copy, flujos y lógica que la dev ya construyó.
**No implementar ni replicar las pantallas diseñadas en Paper.**

## Cómo usar el paquete
1. Leer [design.md](design.md), especialmente el alcance.
2. Integrar [tokens.css](tokens.css): 105 variables originales de Paper.
3. Adaptar [components.css](components.css) a los componentes que ya existen; recetas optativas, no un reemplazo del frontend.
4. Usar [assets/](assets/) para marca e iconos y [tokens.json](tokens.json) para consultar roles.

No se requiere migrar a shadcn/ui ni crear componentes nuevos. No incluye exportaciones de pantallas.
No se modificó el sitio. No incluye fuentes binarias ni dependencias. El sitio no pudo inspeccionarse con la herramienta web; no es una auditoría de su código.
