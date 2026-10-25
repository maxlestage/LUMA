# LÜMA frente a Lightroom Classic: la paridad, función por función

LÜMA reproduce el **flujo de trabajo** de Lightroom Classic: los mismos módulos, los mismos
paneles, los mismos reguladores con los mismos nombres, rangos y valores predeterminados, y la
lectura de los archivos XMP de Lightroom. Un fotógrafo encuentra en él sus referencias y sus
ajustes existentes.

## Qué significa «idéntico» y qué no puede significar

| Se conserva tal cual | Diferente a propósito |
|---|---|
| Organización en módulos (Biblioteca, Revelar, Exportar…) y en paneles | El aspecto: LÜMA tiene su propia identidad visual, no la de Adobe |
| Nombre, rango, valor predeterminado y sentido de cada regulador | El motor de renderizado: el de Adobe es secreto. LÜMA tiene el suyo, escrito en Rust; un mismo valor tiene el mismo significado y la misma amplitud, pero no da exactamente los mismos píxeles |
| Ajustes intercambiados en formato XMP `crs:` (lectura y escritura) | Los perfiles de Adobe («Adobe Color»…), protegidos: LÜMA ofrece sus equivalentes (Color, Neutro, Vivo, Monocromo) |
| Temperatura y matiz en la escala DNG (el mismo «5500 K, +10») | Las marcas y los logotipos |
| Atajos de teclado (`\`, J, Z, copiar y pegar, deshacer…) | |

Leyenda: ✅ hecho y probado · 🟡 parcial · ⏳ pendiente

## Módulo Revelar

| Panel | Función | Estado |
|---|---|---|
| Perfil | Color, Neutro, Vivo, Monocromo; lectura de `crs:CameraProfile` | ✅ |
| | LUT 3D `.cube` (como los perfiles creativos), con cantidad ajustable, también en la GPU | ✅ |
| | Perfiles del fabricante («Camera Standard»…) | ⏳ |
| Básicos | Balance de blancos «Como se tomó», temperatura y matiz (escala DNG) | ✅ |
| | Balance de blancos automático, ajustes preestablecidos (luz de día, nublado, sombra, tungsteno, fluorescente, flash), cuentagotas (W) | ✅ |
| | Exposición, Contraste | ✅ |
| | Altas luces, Sombras (adaptativas a la imagen, como en el proceso 2012) | ✅ |
| | Blancos, Negros | 🟡 fijos, todavía no adaptativos a la imagen |
| | Textura, Claridad, Neblina | ✅ |
| | Intensidad, Saturación | ✅ |
| Curva de tonos | Paramétrica (4 regiones, 3 divisiones) | ✅ |
| | Curvas de puntos: luminosidad, rojo, verde, azul | ✅ |
| Mezclador de color | Tono, saturación y luminancia de 8 colores | ✅ |
| | Mezcla de blanco y negro (±3 EV, `crs:GrayMixer*`) | ✅ |
| | Herramienta de ajuste selectivo | ⏳ |
| Gradación de color | Sombras, medios tonos, altas luces, global; fusión, equilibrio | ✅ |
| Detalle | Enfoque: cantidad, radio, detalle, máscara | ✅ |
| | Reducción de ruido de luminancia y de color | ✅ |
| | Reducción de ruido por IA, superresolución | ⏳ |
| Correcciones de lente | Aberración cromática lateral estimada en la foto, distorsión, viñeteado (cantidad, punto medio), eliminación de halos púrpuras y verdes; nombres XMP de Lightroom | ✅ |
| | Perfiles de objetivos (base de datos), aberración longitudinal | ⏳ |
| Transformar | Perspectiva vertical y horizontal, rotación, aspecto, escala (homografía, centro fijo) | ✅ |
| | Upright automático y guiado | ⏳ |
| Efectos | Viñeta tras el recorte (cantidad, punto medio, redondez, desvanecimiento, altas luces) | ✅ |
| | Grano (cantidad, tamaño, aspereza), idéntico en todos los tamaños | ✅ |
| Calibración | Matiz de las sombras, tono y saturación de los primarios | ✅ |
| Herramientas | Recorte y enderezado (ángulo libre), herramienta en pantalla con proporciones bloqueadas (R) | ✅ |
| | Máscaras: degradado lineal (M), degradado radial (Mayús+M), pincel con borrador (K); invertir, cantidad, mostrar superposición (O); exposición, contraste, altas luces, sombras, blancos, negros, temperatura, matiz, saturación, claridad | ✅ |
| | Máscaras por IA (sujeto, cielo), rango de luminancia o de color; lectura de las máscaras de los XMP de Lightroom | ⏳ |
| | Corrección, clonar, ojos rojos, eliminar | ⏳ |
| | Historial, deshacer y rehacer, copiar y pegar ajustes | ✅ en LÜMA Studio |
| | Instantáneas, sincronización en varias fotos | ⏳ |
| | Histograma, indicador de recorte (J), antes/después (`\`), zoom 1:1 | ✅ |
| | Panorámica (perspectiva, cilíndrica, esférica), recorte automático reversible, DNG | ✅ |
| | Apilamientos: media, mediana, máximo, cielo estrellado (más allá de Lightroom) | ✅ |
| | Combinación HDR, deformación de los límites de la panorámica | ⏳ |
| | 100 ajustes preestablecidos con cantidad: películas en color, cine, blanco y negro, instantáneas; todos medidos sobre una carta de prueba en cada ejecución de las pruebas (distintos, sin imágenes empastadas, renderizados idénticos en la GPU) | ✅ |
| | Ajustes preestablecidos del usuario (guardar, importar `.xmp` de Lightroom), exportación uno a uno o por carpeta en formato Lightroom | ✅ |
| | Foto editada señalada bajo la foto y en las miniaturas (película, cantidad, paneles ajustados) | ✅ |
| | Medición de una película real: un escaneo comparado con la misma escena en digital, alineación automática, LUT 3D (más allá de Lightroom) | ✅ |
| | Películas apiladas mediante máscaras, cada una en su zona y con su cantidad, incluidas altas luces, sombras y claridad (más allá de Lightroom) | ✅ |

## Formatos y color

| Función | Estado |
|---|---|
| RAW Bayer: Sony ARW, Canon CR3, Nikon NEF (y los demás formatos que lee rawler) | ✅ 3 cámaras verificadas de forma continua |
| RAW X‑Trans (Fujifilm) | ✅ interpolación cromática específica |
| DNG lineal, Apple ProRAW (con `BaselineExposure`) | ✅ |
| Color DNG: dos matrices interpoladas según la temperatura | ✅ |
| Pipeline en coma flotante de 32 bits, espacio de trabajo Rec. 2020 lineal | ✅ |
| Altas luces saturadas en el sensor reproducidas neutras (sin cielos rosados) | ✅ |
| Orientación EXIF | ✅ |

## Exportación

| Función | Estado |
|---|---|
| JPEG (calidad), PNG y TIFF de 8 o 16 bits | ✅ |
| sRGB, Display P3, Rec. 2020, ProPhoto, con perfil ICC incrustado | ✅ |
| Cambio de tamaño (lado largo) | ✅ |
| Ventana Exportar: formato, calidad, profundidad de bits, espacio de color, tamaño, carpeta; exportación de todas las fotos del filtro | ✅ |
| Enfoque de salida, marca de agua, copia de los metadatos EXIF, ajustes preestablecidos de exportación | ⏳ |

## Biblioteca e interoperabilidad

| Función | Estado |
|---|---|
| Lectura de los XMP de Lightroom (archivo sidecar `IMG_0001.xmp`), escritura en el mismo formato | ✅ guardado automático en la app |
| Tira de película de la carpeta con miniaturas, navegación ← → | ✅ |
| Cuadrícula, valoraciones (0 a 5), banderas (P, X, U), filtro por valoración y bandera, en el XMP | ✅ |
| Importación, catálogo, colecciones, palabras clave, búsqueda | ⏳ |
| Importación de un catálogo de Lightroom (`.lrcat`) | ⏳ |

## Lo que ya es mejor, o lo será

- **No se envía nada a ninguna parte**: sin cuenta, sin nube obligatoria.
- **100 % Rust, verificado**: `scripts/audit-rust.sh` comprueba el grafo de dependencias en
  cada compilación (170 crates, ningún enlace nativo ni compilador de C).
- **Medido, no prometido**: `luma bench` y la CI publican los tiempos de renderizado reales.
