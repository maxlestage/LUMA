# Funciones

Lo que LÜMA sabe hacer, y lo que viene, área por área: el revelado, los formatos y el
color, la exportación, la biblioteca. Cada función marcada como «hecha» está cubierta
por las pruebas.

## Principios

- **No destructivo**: un ajuste es una lista de parámetros, que se vuelve a aplicar en
  cada renderizado; el original nunca se modifica.
- **Reguladores precisos**: cada uno con su nombre, su rango, su valor
  predeterminado y su sentido; temperatura y matiz en la escala DNG («5500 K, +10»).
- **Un motor propio**: escrito en Rust, en coma flotante de 32 bits, en la GPU, con sus
  propios perfiles de renderizado (Color, Neutro, Vivo, Monocromo).
- **El formato XMP**: ajustes leídos y escritos en XMP `crs:`, en un archivo
  junto al RAW o como ajustes preestablecidos.
- **Una identidad propia**: interfaz, renderizado y atajos de teclado (`\`, J, Z,
  copiar y pegar, deshacer…) pensados para el trabajo diario.

Leyenda: ✅ hecho y probado · 🟡 parcial · ⏳ pendiente

## Módulo Revelar

| Panel | Función | Estado |
|---|---|---|
| Perfil | Color, Neutro, Vivo, Monocromo; lectura de `crs:CameraProfile` | ✅ |
| | LUT 3D `.cube` (como los perfiles creativos), con cantidad ajustable, también en la GPU | ✅ |
| | Perfiles del fabricante | ⏳ |
| Básicos | Balance de blancos «Como se tomó», temperatura y matiz (escala DNG) | ✅ |
| | Balance de blancos automático, ajustes preestablecidos (luz de día, nublado, sombra, tungsteno, fluorescente, flash), cuentagotas (W) | ✅ |
| | Exposición, Contraste | ✅ |
| | Altas luces, Sombras (adaptativas a la imagen) | ✅ |
| | Blancos, Negros (adaptativos: llevan los extremos reales de la foto al blanco y al negro) | ✅ |
| | Textura, Claridad, Neblina | ✅ |
| | Intensidad, Saturación | ✅ |
| Curva de tonos | Paramétrica (4 regiones, 3 divisiones) | ✅ |
| | Curvas de puntos: luminosidad, rojo, verde, azul | ✅ |
| Mezclador de color | Tono, saturación y luminancia de 8 colores | ✅ |
| | Mezcla de blanco y negro (±3 EV, `crs:GrayMixer*`) | ✅ |
| | Herramienta de ajuste selectivo (T): arrastrar sobre un color de la foto ajusta su tono, saturación o luminancia | ✅ |
| Gradación de color | Sombras, medios tonos, altas luces, global; fusión, equilibrio | ✅ |
| Detalle | Enfoque: cantidad, radio, detalle, máscara | ✅ |
| | Reducción de ruido de luminancia y de color | ✅ |
| | Reducción de ruido avanzada (medias no locales, ondículas), sin IA | ⏳ |
| Correcciones de lente | Aberración cromática lateral estimada en la foto, distorsión, viñeteado (cantidad, punto medio), eliminación de halos púrpuras y verdes | ✅ |
| | Correcciones de objetivo escritas en los DNG (opcodes `WarpRectilinear` y `FixVignetteRadial`): distorsión, aberración cromática lateral y viñeteo, aplicadas automáticamente en todos los tamaños | ✅ |
| | Correcciones de objetivo de los ARW de Sony (distorsión, aberración cromática lateral, viñeteo, registradas por la cámara), aplicadas automáticamente sin borde vacío | ✅ |
| | Correcciones de objetivo de los RAF de Fujifilm (distorsión, aberración cromática lateral, viñeteo, registradas por la cámara, modo recortado incluido), aplicadas automáticamente | ✅ |
| | Correcciones de objetivo de los ORF de Olympus y OM System (distorsión, aberración cromática lateral) y de los RW2 de Panasonic (distorsión), registradas por la cámara, aplicadas automáticamente | ✅ |
| | Casilla «Correcciones de la cámara»: marcada por defecto, guardada en el XMP (`crs:LensProfileEnable`); desmarcada, la foto vuelve a ser la que dio el objetivo, y los controles de distorsión y viñeteo toman el relevo | ✅ |
| | Aberración cromática longitudinal, correcciones de otras marcas (Canon, Nikon, Pentax…) | ⏳ |
| Transformar | Perspectiva vertical y horizontal, rotación, aspecto, escala (homografía, centro fijo) | ✅ |
| | Enderezamiento automático y guiado | ⏳ |
| Efectos | Viñeta tras el recorte (cantidad, punto medio, redondez, desvanecimiento, altas luces) | ✅ |
| | Grano (cantidad, tamaño, aspereza), idéntico en todos los tamaños | ✅ |
| Calibración | Matiz de las sombras, tono y saturación de los primarios | ✅ |
| Herramientas | Recorte y enderezado (ángulo libre), herramienta en pantalla con proporciones bloqueadas (R) | ✅ |
| | Máscaras: degradado lineal (M), degradado radial (Mayús+M), pincel con borrador (K); invertir, cantidad, mostrar superposición (O); exposición, contraste, altas luces, sombras, blancos, negros, temperatura, matiz, saturación, claridad | ✅ |
| | Máscaras restringidas a un rango de luminancia (mínimo, máximo, suavizado) o de color (tono, anchura, suavizado; grises excluidos), leído sobre la foto; zona visible (O) | ✅ |
| | Pincel que sigue los bordes: cada toque solo pinta los tonos cercanos a su centro, la zona se detiene en los contornos de la foto | ✅ |
| | Combinaciones de máscaras; lectura de las máscaras `crs:MaskGroupBasedCorrections` | ⏳ |
| | Corrección, clonar, ojos rojos, eliminar | ⏳ |
| | Historial, deshacer y rehacer, copiar y pegar ajustes | ✅ en LÜMA Studio |
| | Instantáneas, sincronización en varias fotos | ⏳ |
| | iPhone y iPad: vista previa reducida durante un gesto (12 veces más rápida), memoria vigilada, cachés liberadas antes de que el sistema cierre la app | ✅ |
| | Histograma, indicador de recorte (J), antes/después (`\`), zoom 1:1 | ✅ |
| | Panorámica (perspectiva, cilíndrica, esférica), recorte automático reversible, DNG | ✅ |
| | Apilamientos: media, mediana, máximo, cielo estrellado | ✅ |
| | Combinación HDR, deformación de los límites de la panorámica | ⏳ |
| | 100 ajustes preestablecidos con cantidad: películas en color, cine, blanco y negro, instantáneas; todos medidos sobre una carta de prueba en cada ejecución de las pruebas (distintos, sin imágenes empastadas, renderizados idénticos en la GPU) | ✅ |
| | Ajustes preestablecidos del usuario (guardar, importar `.xmp`), exportación uno a uno o por carpeta en formato XMP | ✅ |
| | Foto editada señalada bajo la foto y en las miniaturas (película, cantidad, paneles ajustados) | ✅ |
| | Medición de una película real: un escaneo comparado con la misma escena en digital, alineación automática, LUT 3D | ✅ |
| | Películas apiladas mediante máscaras, cada una en su zona y con su cantidad, incluidas altas luces, sombras y claridad | ✅ |

## Formatos y color

| Función | Estado |
|---|---|
| RAW Bayer: Sony ARW, Canon CR3, Nikon NEF (y los demás formatos RAW habituales) | ✅ 3 cámaras verificadas de forma continua |
| RAW X‑Trans (Fujifilm) | ✅ interpolación cromática específica |
| DNG lineal, Apple ProRAW (con `BaselineExposure`) | ✅ |
| Imágenes reveladas: JPEG, PNG, TIFF, WebP, GIF, BMP, TGA, QOI, OpenEXR, HDR, PNM, ICO, DDS, JPEG XL, PSD, JPEG 2000, SVG; HEIC, HEIF, AVIF (ImageIO de Apple); extensión desconocida leída por su contenido | ✅ perfil «Original»: idéntica al abrir |
| Color DNG: dos matrices interpoladas según la temperatura | ✅ |
| Pipeline en coma flotante de 32 bits, espacio de trabajo Rec. 2020 lineal | ✅ |
| Altas luces saturadas en el sensor reproducidas neutras (sin cielos rosados) | ✅ |
| Orientación EXIF | ✅ |
| Perfiles ICC leídos (versiones 2 y 4): matriz y curvas, tablas lut8, lut16, A→B y B→A de impresoras y papeles, cuatro intenciones de reproducción (`luma icc`) | ✅ |
| Prueba en pantalla: la vista previa tal como se imprimirá con un perfil de impresora, intención perceptual o colorimétrica relativa (histograma incluido) | ✅ |

## Exportación

| Función | Estado |
|---|---|
| JPEG (calidad), PNG y TIFF de 8 o 16 bits | ✅ |
| sRGB, Display P3, Rec. 2020, ProPhoto, con perfil ICC incrustado | ✅ |
| Cambio de tamaño: lado largo, lado corto, ancho × alto, megapíxeles, sin ampliar nunca (`--resize`) | ✅ |
| Ventana Exportar: formato, calidad, profundidad de bits, espacio de color, tamaño, carpeta; exportación de todas las fotos del filtro | ✅ |
| Exportación a varios ajustes preestablecidos a la vez (ventana Exportar: casillas, usar, guardar; `luma export --preset A --preset B`) | ✅ |
| Ajustes preestablecidos de exportación (formato, calidad, profundidad, espacio, tamaño, carpeta): web, resolución completa, pantalla P3, impresión, y los suyos; `luma export-preset`, `luma develop --preset` | ✅ |
| Plantillas de nombre de archivo: `{nom}` (o `{nombre}`), `{date}` (`{fecha}`), `{séquence}` (`{secuencia}`), `{boîtier}` (`{cámara}`), `{objectif}` (`{objetivo}`), `{iso}`, `{note}` (`{valoración}`), `{libellé}` (`{etiqueta}`)…, subcarpetas con «/»; en la ventana Exportar, los ajustes preestablecidos y `luma develop --name` | ✅ |
| Enfoque de salida: pantalla, papel mate, papel brillante; bajo, estándar, alto; al tamaño final (`--output-sharpen`) | ✅ |
| Marca de agua de texto (tipografía de LÜMA, suavizada): nueve posiciones, tamaño, opacidad, blanco o negro (`--watermark`) | ✅ |
| Marca de agua con logotipo: un PNG con su transparencia, colores fieles, nueve posiciones, tamaño, opacidad (`--logo`) | ✅ |
| Metadatos de exportación: EXIF de la toma copiado (cámara, objetivo, ajustes, fecha), autor y copyright, palabras clave, valoración y etiqueta en XMP, ubicación GPS quitada por defecto (`--author`, `--keep-gps`) | ✅ |
| Metadatos también en los TIFF: EXIF, perfil ICC y XMP, TIFF escrito por LÜMA (mismos píxeles, mismo tamaño) | ✅ |

## Biblioteca e interoperabilidad

| Función | Estado |
|---|---|
| Lectura de los XMP (archivo sidecar `IMG_0001.xmp`), escritura en el mismo formato | ✅ guardado automático en la app |
| Tira de película de la carpeta con miniaturas, navegación ← → | ✅ |
| Cuadrícula, valoraciones (0 a 5), banderas (P, X, U), filtro por valoración y bandera, en el XMP | ✅ |
| Catálogo local (`luma catalog`): carpetas y subcarpetas, fecha de captura, cámara, objetivo, ISO leídos en la cabecera sin decodificar la imagen, actualización incremental, orden por fecha | ✅ |
| Miniaturas instantáneas (la vista previa integrada en el RAW, en 15 ms), luego renderizadas con los ajustes y guardadas sin pérdida para las visitas siguientes | ✅ |
| Copia de trabajo a resolución completa: idéntica al original, más rápida de abrir, dos veces y media menos memoria; «Vaciar la caché» en un clic | ✅ |
| Palabras clave en el XMP (`dc:subject`), leídas y escritas | ✅ |
| Búsqueda y filtros (`luma catalog --search`): palabras sin tener en cuenta los acentos; valoración, bandera, etiqueta, fechas, ISO, focal, cámara, objetivo, palabra clave; recuento de palabras clave, cámaras y objetivos | ✅ |
| Colecciones manuales e inteligentes (regla: valoración, fecha, cámara, objetivo, ISO, palabras clave…), guardadas en el catálogo | ✅ |
| Palabras clave en la app: escritura (varias a la vez, separadas por comas), añadidas y quitadas en todas las fotos seleccionadas, lista de palabras clave de la carpeta con su número, filtro de la cuadrícula | ✅ |
| Fototeca en la app: cada carpeta abierta entra en ella (en segundo plano); todas las fotos, las colecciones y las carpetas, cada una con su número de fotos; búsqueda y filtros; vistas ordenadas por fecha de captura | ✅ |
| Copias virtuales: varios revelados de una foto sin duplicar el archivo (un XMP por copia), cada una con su valoración, sus palabras clave y su miniatura, junto a su original; exportada como `IMG_0001-2`; eliminada sin tocar el original | ✅ |
| Importación de catálogos existentes | ⏳ |

## Lo que distingue a LÜMA

- **No se envía nada a ninguna parte**: sin cuenta, sin nube obligatoria.
- **100 % Rust, verificado**: `scripts/audit-rust.sh` comprueba el grafo de dependencias en
  cada compilación (170 crates, ningún enlace nativo ni compilador de C).
- **Medido, no prometido**: `luma bench` y la CI publican los tiempos de renderizado reales.
