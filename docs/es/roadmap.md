# LÜMA — Hoja de ruta (Apple primero)

macOS e iOS/iPadOS son las plataformas prioritarias. Windows, Linux y Android se
compilan en CI desde el primer mes (el motor no debe presuponer nada de Apple), pero sus
aplicaciones no se publican hasta la fase 6.

Estimación: de 24 a 36 meses para una versión 1.0 fotográfica profesional con un equipo experimentado.
Es una estimación, no una garantía.

| Fase | Meses | Apple (prioritario) | Otras plataformas |
|---|---|---|---|
| 1. Viabilidad | M0–M3 | App mínima para macOS y app mínima para iOS en Rust (`winit` + `objc2`), renderizado `wgpu`/Metal, decodificación con `rawler` sobre un corpus (Sony, Canon, Nikon, Fujifilm, ProRAW), firma y TestFlight automatizados | Compilación en CI del motor para Windows, Linux, Android |
| 2. Motor | M4–M6 | Pipeline f32, Display P3, EDR/HDR en pantallas XDR, memoria unificada sin copias, grafo de revelado, proyecto versionado | Pruebas del motor en CPU (alternativa) |
| 3. Alfa | M7–M12 | Mac: fototeca, revelado, exportación. iPhone/iPad: importación desde PhotoKit y Archivos, máscaras con el Apple Pencil, gestos. TestFlight público | — |
| 4. Beta | M13–M18 | Máscaras por rangos de color y de luminancia, reducción de ruido avanzada (sin IA), correcciones de objetivo leídas en los RAW, búsqueda, recuperación tras un cierre inesperado | Primera adaptación del renderizado a Vulkan/DX12 |
| 5. 1.0 | M19–M24 | App Store (iPhone y iPad), descarga directa (Mac), accesibilidad, migración, documentación | — |
| 6. Expansión | M25–M36 | Vídeo (timeline, etalonaje), captura conectada (tethering), impresión | Aplicaciones para Windows, Linux y Android sobre el mismo motor |

## Avance

**M0 – M3, viabilidad: el motor de revelado existe.** Hecho:

- workspace de Cargo (`crates/`), regla «100 % Rust» verificada en CI sobre el grafo de
  dependencias real;
- lectura de RAW Bayer, X‑Trans y DNG lineal (ProRAW), color en el sentido DNG,
  interpolación cromática Malvar‑He‑Cutler y X‑Trans;
- módulo Revelar completo, panel por panel (véase
  [`fonctions.md`](fonctions.md)), lectura y escritura de XMP `crs:`;
- exportación JPEG, PNG, TIFF de 8/16 bits en sRGB o Display P3 con perfil ICC;
- 5 cámaras reveladas en cada push (Sony, Canon, Nikon, Fujifilm, iPhone), mediciones
  de capacidad de respuesta publicadas;
- LÜMA Studio, la app para Mac (`apps/studio`): módulo Revelar completo, historial,
  XMP automático, tira de película, atajos de teclado, Biblioteca (valoraciones, banderas,
  filtro), herramienta Recortar, ventana Exportar; acabado en la GPU (Metal), imagen
  base en caché; máscaras lineal, radial y pincel;
  correcciones de lente y transformación; `LÜMA.app` universal generada por
  la CI en un runner con Apple Silicon.

Próximos pasos, por orden:

1. renderizado en GPU, continuación: el acabado por píxel está hecho (`luma-gpu`, idéntico al
   procesador); quedan las etapas de vecindad (ruido, tono local, enfoque)
   y la visualización sin relectura (textura compartida con la interfaz);
2. en la app: catálogo (varias carpetas, colecciones, palabras clave), ajustes preestablecidos;
3. máscaras por rangos de color y de luminancia, correcciones de objetivo leídas en los RAW, enderezamiento automático (sin IA);
4. iOS: la app ya funciona (simulador, CI); quedan PhotoKit, los gestos (pellizcar para
   hacer zoom), el Apple Pencil para las máscaras, la firma y TestFlight.

## Hitos de validación

- **M3**: un RAW se abre, se ajusta y se exporta en Mac y en iPhone, sin ninguna dependencia prohibida.
- **M6**: renderizado idéntico Mac/iPhone con tolerancia ΔE documentada, vista previa P95 < 100 ms en Mac.
- **M12**: un fotógrafo realiza un flujo completo (importación → selección → revelado → exportación) en Mac o iPad.
- **M18**: las herramientas avanzadas superan las pruebas de calidad, compatibilidad y rendimiento.
- **M24**: versión estable publicada en el App Store (iPhone y iPad) y en descarga directa (Mac), matriz de compatibilidad pública.

## Riesgos principales

| Riesgo | Nivel | Respuesta | Situación actual |
|---|---|---|---|
| Cobertura RAW | Muy alto | Corpus real, matriz por cámara publicada y ampliada en cada versión | **Hecho.** [`boitiers.md`](boitiers.md): 735 cámaras de 20 marcas, 714 con color calibrado, generada a partir de la base de datos del decodificador (`luma cameras --markdown`) y verificada por la CI en cada push. 5 RAW reales revelados en cada prueba (Sony, Canon, Nikon, Fujifilm, iPhone ProRAW); se lee cualquier DNG. Por ampliar: un RAW real más por versión (Canon CR2, Olympus, Panasonic, Pentax). |
| Vídeo sin bibliotecas de terceros | Muy alto | Decodificación mediante los frameworks del sistema de Apple, o alcance reducido; auditoría desde M0 | **Decidido: alcance reducido.** La v1 es un editor de fotos, sin vídeo. La auditoría: decodificar H.264/HEVC/ProRes en Rust puro no es realista; AVFoundation (del sistema, admitido) requiere enlaces con Objective‑C y, por tanto, código `unsafe`, hoy prohibido en LÜMA. Si el vídeo vuelve, será en una capa `luma-platform` aislada, la única autorizada a usar `unsafe`, auditada por separado. |
| Integración de iOS en Rust (UIKit, Pencil, PhotoKit) | Alto | Prototipo desde M0, capa `luma-platform` fina y probada | **Prototipo hecho**: la app funciona en iPhone y iPad (simulador lanzado en cada push, captura de pantalla verificada), Metal, RAW mediante la app Archivos. **Queda**: PhotoKit (fototeca), Apple Pencil (presión para el pincel), compartir. Todo ello requiere enlaces con Objective‑C: la misma regla que para el vídeo, una capa `luma-platform` aislada. |
| Color no lo bastante fiel | Alto | Pruebas colorimétricas, renderizados de referencia | **Pruebas implantadas** (`crates/luma-render/tests/color.rs`): carta ColorChecker vista por un sensor real (Sony A7 III), ΔE 2000 < 1 con luz de día y bajo una lámpara de 3200 K (interpolación de las matrices), tonos preservados a lo largo del revelado completo; 100 películas medidas sobre una carta de prueba (`luma-image/tests/presets.rs`), GPU idéntica al procesador. **Medición con una cámara real**: `luma colorchecker photo.RAW` (foto de una carta real, recortada) da el ΔE 2000 de cada parche. **Queda**: fotos de cartas reales en el corpus, y perfiles que vayan más allá de las matrices (tablas de tono, perfiles DCP). |
| Memoria en iPhone | Alto | Teselas, caché limitada, procesamiento progresivo | **Cachés limitadas en bytes**: 600 MB en iPhone y iPad, 3 GB en el resto (`luma_render::memory_budget`), las entradas más antiguas salen primero, probado. Vistas previas al tamaño de la pantalla. **Queda**: la exportación a resolución completa por teselas (hoy la imagen entera está en memoria: unos 600 MB para 48 Mpx). |
| Revisión de la App Store | Medio | Cumplimiento de las normas desde la alfa, TestFlight continuo | **Listo por parte de la app**: manifiesto de privacidad (`PrivacyInfo.xcprivacy`: sin seguimiento, sin datos recopilados), sin cifrado que declarar, Info.plist completo. **Queda**: una cuenta de Apple Developer (firma, TestFlight), y la suscripción en iPhone y iPad mediante compras dentro de la app (norma de la App Store, véase [`premium.md`](premium.md)). |
