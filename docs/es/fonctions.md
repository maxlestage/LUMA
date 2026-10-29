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
| | Tonos automáticos («Tonos automáticos», `--auto-tone`): exposición, contraste, altas luces y sombras leídos en el histograma de la foto, sin IA | ✅ |
| | Exposición, Contraste | ✅ |
| | Altas luces, Sombras (adaptativas a la imagen) | ✅ |
| | Blancos, Negros (adaptativos: llevan los extremos reales de la foto al blanco y al negro) | ✅ |
| | Textura, Claridad, Neblina | ✅ |
| | Intensidad, Saturación | ✅ |
| Curva de tonos | Paramétrica (4 regiones, 3 divisiones) | ✅ |
| | Curvas de puntos: luminosidad, rojo, verde, azul | ✅ |
| | Editor de curvas de puntos en la app (Luz, Curva de tonos): un clic añade un punto, se arrastra, un doble clic lo quita; la curva paramétrica en tenue debajo; se puede deshacer | ✅ |
| Mezclador de color | Tono, saturación y luminancia de 8 colores | ✅ |
| | Mezcla de blanco y negro (±3 EV, `crs:GrayMixer*`) | ✅ |
| | Herramienta de ajuste selectivo (T): arrastrar sobre un color de la foto ajusta su tono, saturación o luminancia | ✅ |
| Gradación de color | Sombras, medios tonos, altas luces, global; fusión, equilibrio | ✅ |
| | Virado parcial (sombras y altas luces, equilibrio; `crs:SplitToning*` leído y escrito) | ✅ |
| | Mezclador de canales: cada canal de salida hecho de los tres canales de la foto (−200 a +200 %) | ✅ |
| | Perfil de cámara medido en una carta ColorChecker («Perfil desde una carta», `luma colorchecker -o perfil.xmp`), como ajuste preestablecido | ✅ |
| | Colores de dos cámaras igualados a partir de la misma escena («Igualar con la foto elegida», `luma match-colors`), como ajuste preestablecido | ✅ |
| Detalle | Enfoque: cantidad, radio, detalle, máscara | ✅ |
| | Enfoque por deconvolución (casilla «Deconvolución», Richardson-Lucy), sin IA | ✅ |
| | Reducción de ruido de luminancia y de color | ✅ |
| | Reducción de ruido avanzada (ondículas, casilla «Ondículas»), sin IA | ✅ |
| Correcciones de lente | Aberración cromática lateral estimada en la foto, distorsión, viñeteado (cantidad, punto medio), eliminación de halos púrpuras y verdes | ✅ |
| | Correcciones de objetivo escritas en los DNG (opcodes `WarpRectilinear` y `FixVignetteRadial`): distorsión, aberración cromática lateral y viñeteo, aplicadas automáticamente en todos los tamaños | ✅ |
| | Correcciones de objetivo de los ARW de Sony (distorsión, aberración cromática lateral, viñeteo, registradas por la cámara), aplicadas automáticamente sin borde vacío | ✅ |
| | Correcciones de objetivo de los RAF de Fujifilm (distorsión, aberración cromática lateral, viñeteo, registradas por la cámara, modo recortado incluido), aplicadas automáticamente | ✅ |
| | Correcciones de objetivo de los ORF de Olympus y OM System (distorsión, aberración cromática lateral) y de los RW2 de Panasonic (distorsión), registradas por la cámara, aplicadas automáticamente | ✅ |
| | Casilla «Correcciones de la cámara»: marcada por defecto, guardada en el XMP (`crs:LensProfileEnable`); desmarcada, la foto vuelve a ser la que dio el objetivo, y los controles de distorsión y viñeteo toman el relevo | ✅ |
| | Aberración cromática longitudinal, correcciones de otras marcas (Canon, Nikon, Pentax…) | ⏳ |
| Transformar | Perspectiva vertical y horizontal, rotación, aspecto, escala (homografía, centro fijo) | ✅ |
| | Enderezamiento automático (Mayús+R, Mayús+V) y guiado: de dos a cuatro líneas trazadas en la foto ajustan la perspectiva y la rotación | ✅ |
| | Bordes vacíos recortados: el mayor encuadre centrado, con las proporciones de la foto, sin bordes vacíos tras la Transformación o la Distorsión | ✅ |
| Efectos | Viñeta tras el recorte (cantidad, punto medio, redondez, desvanecimiento, altas luces) | ✅ |
| | Grano (cantidad, tamaño, aspereza), idéntico en todos los tamaños | ✅ |
| Calibración | Matiz de las sombras, tono y saturación de los primarios | ✅ |
| Herramientas | Recorte y enderezado (ángulo libre), herramienta en pantalla con proporciones bloqueadas (R) | ✅ |
| | Guías de recorte: tercios, proporción áurea, diagonales, triángulos áureos, espiral áurea, cuadrícula (O para cambiar, Mayús+O para girar) | ✅ |
| | Enderezado automático (Mayús+R): horizonte y verticales leídos con una transformada de Hough; nada en caso de duda (primer plano, líneas de fuga) | ✅ |
| | Verticales automáticas (Mayús+V): se mide el punto de fuga de las verticales y se ajusta la Vertical de la Transformación para volverlas paralelas | ✅ |
| | Máscaras: degradado lineal (M), degradado radial (Mayús+M), pincel con borrador (K); invertir, cantidad, mostrar superposición (O); exposición, contraste, altas luces, sombras, blancos, negros, temperatura, matiz, saturación, claridad, textura, nitidez (realzada o suavizada), reducción de ruido, neblina | ✅ |
| | Máscaras restringidas a un rango de luminancia (mínimo, máximo, suavizado) o de color (tono, anchura, suavizado; grises excluidos), leído sobre la foto; zona visible (O) | ✅ |
| | Máscara «Cielo» («Seleccionar el cielo»): cielo y nubes encontrados en la foto, bordes seguidos, reflejos en el agua excluidos; sin IA | ✅ |
| | Pincel que sigue los bordes: cada toque solo pinta los tonos cercanos a su centro, la zona se detiene en los contornos de la foto | ✅ |
| | Combinaciones de máscaras: añadir, restar, intersecar otras formas (cada una invertible) | ✅ |
| | Duplicar una máscara, o duplicarla invertida (ajustes a cero): el suelo después del cielo, sin volver a trazar la zona | ✅ |
| | Zona de la máscara mostrada (O) en rojo, verde, azul, blanco o blanco sobre negro, opacidad del 10 al 100 % | ✅ |
| | Lectura de las máscaras de un XMP de otra aplicación (`crs:MaskGroupBasedCorrections`, y las listas de degradados antiguas): degradados lineales y radiales, intensidad, inversión, ajustes locales, llevados al recorte | ✅ |
| | Máscaras de pincel, de sujeto y de rango de un XMP de otra aplicación | ⏳ |
| | Corrector y tampón (Q): un clic, la fuente encontrada sola, zona y fuente desplazables; tamaño, difuminado, opacidad | ✅ |
| | Manchas del sensor: vista de manchas (el polvo como puntos oscuros), «Quitar el polvo» con un clic | ✅ |
| | Ojos rojos: un círculo en el ojo, la pupila roja vuelve a ser negra, la piel se queda | ✅ |
| | Muaré y desflecado (aberración cromática local) con el pincel, en cada máscara | ✅ |
| | Eliminar: un círculo sobre lo que molesta (cable, transeúnte, basura), rellenado con trozos de la foto de alrededor, textura incluida, sin IA | ✅ |
| | Eliminación trazada: un trazo a lo largo de un cable, una raya o un alambre, borrado de un solo gesto; sin IA | ✅ |
| | Corrector y tampón trazados: un trazo a lo largo de una arruga o una raya, la fuente con el mismo desplazamiento en todo el trazo | ✅ |
| | Licuar (Retoque): empujar, inflar, desinflar con un pincel (tamaño, fuerza), trazos guardados en el XMP, deshacibles; sin IA | ✅ |
| | Historial, deshacer y rehacer, copiar y pegar ajustes | ✅ en LÜMA Studio |
| | Copiar y pegar selectivo de ajustes («Pegar…», Alt+Mayús+Cmd+V): catorce grupos a elegir, recorte y máscaras propios de cada foto por defecto | ✅ |
| | Sincronizar una selección (Mayús+Cmd+S): los ajustes de la foto abierta a las demás fotos elegidas, grupo por grupo; balance automático en lote, cada foto el suyo | ✅ |
| | Ajustes predeterminados por cámara y a partir de un ISO (gana el más preciso), punto de partida de las fotos sin XMP; película de importación para las fotos que llegan (carpeta vigilada, cámara conectada) | ✅ |
| | Instantáneas con nombre, guardadas en el XMP, recuperadas con un clic (un paso del historial); sincronización en varias fotos | ✅ |
| | iPhone y iPad: vista previa reducida durante un gesto (12 veces más rápida), memoria vigilada, cachés liberadas antes de que el sistema cierre la app | ✅ |
| | Histograma, indicador de recorte (J), antes/después (`\`), zoom 1:1 | ✅ |
| | Forma de onda, desfile RGB, vectorscopio (Mayús+J) | ✅ |
| | Medición puntual (RGB, Lab), recorte por canal | ✅ |
| | Panorámica (perspectiva, cilíndrica, esférica), recorte automático reversible, DNG | ✅ |
| | Apilamientos: media, mediana, máximo, cielo estrellado | ✅ |
| | Fusión HDR: tomas en horquillado alineadas, sin fantasmas, en DNG (`luma merge --stack hdr`) | ✅ |
| | Apilado de enfoque: cada toma alineada con su vecina, lo más nítido de cada una (`luma merge --stack focus`) | ✅ |
| | Deformación de los bordes del panorama: los bordes irregulares se estiran para llenar el rectángulo en lugar de recortarse («Deformar los bordes», `luma merge --warp`) | ✅ |
| | 275 ajustes preestablecidos con cantidad: películas en color, cine, blanco y negro, instantáneas; todos medidos sobre una carta de prueba en cada ejecución de las pruebas (distintos, sin imágenes empastadas, renderizados idénticos en la GPU) | ✅ |
| | Ajustes preestablecidos del usuario (guardar, importar `.xmp`), exportación uno a uno o por carpeta en formato XMP | ✅ |
| | Foto editada señalada bajo la foto y en las miniaturas (película, cantidad, paneles ajustados) | ✅ |
| | Medición de una película real: un escaneo comparado con la misma escena en digital, alineación automática, LUT 3D | ✅ |
| | Películas apiladas mediante máscaras, cada una en su zona y con su cantidad, incluidas altas luces, sombras y claridad | ✅ |

## Formatos y color

| Función | Estado |
|---|---|
| RAW Bayer: Sony ARW, Canon CR3, Nikon NEF (y los demás formatos RAW habituales) | ✅ 3 cámaras verificadas de forma continua |
| RAW X‑Trans (Fujifilm) | ✅ interpolación cromática específica |
| Interpolación cromática Bayer direccional: el verde sigue los contornos (sin efecto cremallera ni punteado en los detalles finos) | ✅ |
| Interpolación cromática X‑Trans direccional: el verde sigue los contornos, también en las sombras con ruido | ✅ |
| DNG lineal, Apple ProRAW (con `BaselineExposure`) | ✅ |
| Imágenes reveladas: JPEG, PNG, TIFF, WebP, GIF, BMP, TGA, QOI, OpenEXR, HDR, PNM, ICO, DDS, JPEG XL, PSD, JPEG 2000, SVG; HEIC, HEIF, AVIF (ImageIO de Apple); extensión desconocida leída por su contenido | ✅ perfil «Original»: idéntica al abrir |
| Color DNG: dos matrices interpoladas según la temperatura | ✅ |
| Pipeline en coma flotante de 32 bits, espacio de trabajo Rec. 2020 lineal | ✅ |
| Altas luces saturadas en el sensor reproducidas neutras (sin cielos rosados) | ✅ |
| Altas luces reconstruidas: un canal saturado toma el color de sus vecinos (casilla «Reconstruir las altas luces», `--set highlights.reconstruction=on`) | ✅ |
| Orientación EXIF | ✅ |
| Perfiles ICC leídos (versiones 2 y 4): matriz y curvas, tablas lut8, lut16, A→B y B→A de impresoras y papeles, cuatro intenciones de reproducción (`luma icc`) | ✅ |
| Prueba en pantalla: la vista previa tal como se imprimirá con un perfil de impresora, intención perceptual o colorimétrica relativa (histograma incluido) | ✅ |
| Aviso de gama: los colores que la impresora no podrá reproducir, en azul sobre la vista previa (ida y vuelta en colorimétrico relativo, diferencia superior a ΔE 3) | ✅ |
| Simulación del papel (tono y luminosidad, colorimétrico absoluto) y de la tinta negra (su negro real); sin ellas, compensación del punto negro | ✅ |
| Impresión («Imprimir…», `luma print`): PDF al tamaño exacto del papel (A4, A3, A3+, A5, Carta EE. UU., de 10 × 15 a 20 × 30 cm, o a medida), vertical u horizontal, márgenes, una foto por página u hoja de contactos (cuadrícula, separación, nombre bajo cada foto), fotos enteras o llenando su celda, resolución, enfoque para papel mate o brillante; páginas convertidas al perfil RGB o CMYK de la impresora (el de la prueba en pantalla, con compensación del punto negro; en CMYK, un PDF en CMYK, sus tintas sin pérdida) ; marcas de corte en las esquinas de las fotos, una línea de texto al pie de la página (`--crop-marks`, `--text`); el PDF abierto con un clic en el lector del sistema, para imprimir («Abrir …», Mac, Linux, Windows), o enviado tal cual a una impresora del sistema, con el número de copias («Enviar a la impresora», Mac y Linux); o las páginas en JPEG, una imagen por página, para un servicio de impresión (`-o hoja.jpg`, libro de fotos incluido) | ✅ |
| Libro de fotos en PDF («Libro, presentación, galería…», `luma book`): portada (primera foto, título, subtítulo), páginas de una, dos o cuatro fotos según su orientación, formatos 20 × 20, 30 × 30, A4, 21 × 28, A3, una página de texto tras la portada y otra al final (`--opening`, `--closing`), el número de fotos por página a elegir («1,2,4», `--pages`), las páginas vistas en pequeño y el orden de las fotos cambiado arrastrando una sobre otra (intercambio) o sobre el fondo de una página (desplazamiento) y el número de fotos de una página elegido («Páginas del libro…», `--layout`); presentación a pantalla completa en la app (avance automático, pausa, flechas, las mismas transiciones) y presentación exportada (`luma slideshow`): una carpeta autónoma, index.html y JPEG, fundido, deslizamiento o fundido con un zoom lento (`--transition`), música en bucle (`--music`, M la silencia), orden aleatorio (`--shuffle`, también en la app), teclado, deslizamiento del dedo | ✅ |
| Galería web estática (`luma gallery`, y desde la app): cuadrícula de miniaturas cargadas al llegar, foto en grande con un clic (flechas, deslizamiento del dedo, Esc), título, descripción, pies de foto; JPEG optimizado para la web (crominancia 4:2:0 y tablas de Huffman calculadas para la imagen: de un 9 a un 29 % más ligero), en el ajuste Web y `--web` | ✅ |
| Vídeo básico (`luma video`): una película de LÜMA, un ajuste XMP o unos ajustes aplicados a cada imagen de un vídeo AVI en Motion JPEG (el formato de muchas cámaras), guardado de nuevo en el mismo formato, mismo tamaño y misma cadencia, el sonido copiado tal cual; sin dependencia nativa | ✅ |

## Exportación

| Función | Estado |
|---|---|
| JPEG (calidad), PNG y TIFF de 8 o 16 bits | ✅ |
| sRGB, Display P3, Rec. 2020, ProPhoto, con perfil ICC incrustado | ✅ |
| Cambio de tamaño: lado largo, lado corto, ancho × alto, megapíxeles, sin ampliar por defecto (`--resize`) | ✅ |
| Ampliación si es necesario («Ampliar si es necesario», `--enlarge`): Lanczos sin halos, hasta cuatro veces cada lado y 200 MP, para copias grandes; sin IA | ✅ |
| Tamaño de copia («Copia impresa (cm)», `--resize 30x40cm@300`): el marco en centímetros y la resolución en puntos por pulgada, escrita en el JPEG, el PNG y el TIFF | ✅ |
| Peso máximo de un JPEG («Peso máximo», `--max-kb 500`): la mejor calidad que cabe en el peso, metadatos incluidos, para un sitio o un formulario que limita el tamaño de los archivos | ✅ |
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
| Selección a pantalla completa con el teclado (F): valoraciones, banderas, etiquetas de color (6 a 9), foto siguiente por sí sola (A), Esc para salir; etiquetas con los nombres del formato en el XMP, encontradas en francés, inglés y español | ✅ |
| Comparación de 2 a 4 fotos (C): lado a lado y luego en cuadrícula, cada una revelada con sus ajustes; zoom y desplazamiento sincronizados (rueda, doble clic, arrastrar), lupa ×4 en el mismo punto de cada foto (L), un clic abre la foto | ✅ |
| Pilas: ráfagas y series de exposición reconocidas (misma cámara, menos de dos segundos de diferencia; la exposición que cambia hace la serie), plegadas tras la primera foto (S para desplegar), «Fusionar la serie» hacia la herramienta Fusión | ✅ |
| Duplicados por huella de imagen (la misma foto copiada, exportada a otro tamaño o recomprimida), agrupados uno al lado del otro; valoraciones, banderas y etiquetas en lote en las fotos elegidas | ✅ |
| Catálogo local (`luma catalog`): carpetas y subcarpetas, fecha de captura, cámara, objetivo, ISO leídos en la cabecera sin decodificar la imagen, actualización incremental, orden por fecha | ✅ |
| Renombrado por lotes («Renombrar…», `luma rename`): los marcadores de la exportación (`{date}_{séquence}`…), el RAW y el JPEG de una toma juntos, el XMP y las copias virtuales los acompañan, `-2` cuando un nombre está ocupado, vuelta atrás ante cualquier fallo; la fototeca y sus colecciones lo siguen | ✅ |
| Copias de seguridad del catálogo (una por semana en la app, se guardan las diez últimas; `luma catalog --backup`, `--restore`), fotos que faltan listadas, carpeta movida recuperada con sus colecciones (`--missing`, `--relocate`); en la app, la ventana «Mantenimiento…»: fotos que faltan por carpeta, localizadas (las carpetas vecinas la siguen) o quitadas, copia a petición y restauración tras confirmar | ✅ |
| Duplicados del catálogo (`luma catalog --duplicates`): misma cámara, mismo instante, archivos del mismo tamaño y tipo (una tarjeta importada dos veces, una copia en otra carpeta), listados con el espacio que ocupan; el RAW y el JPEG de una misma toma no son duplicados; no se borra nada | ✅ |
| Miniaturas instantáneas (la vista previa integrada en el RAW, en 15 ms), luego renderizadas con los ajustes y guardadas sin pérdida para las visitas siguientes | ✅ |
| Copia de trabajo a resolución completa: idéntica al original, más rápida de abrir, dos veces y media menos memoria; «Vaciar la caché» en un clic | ✅ |
| Palabras clave en el XMP (`dc:subject`), leídas y escritas | ✅ |
| El XMP que el archivo lleva dentro (JPEG, TIFF, DNG, PNG, RAW) leído sin XMP al lado: valoración, etiqueta, palabras clave puestas por otra aplicación o por la cámara (en atributos o en elementos), en la fototeca y en la app; y los ajustes de revelado que un RAW o un DNG lleva dentro (nunca los de un JPEG, ya en sus píxeles) | ✅ |
| Información IPTC en el XMP: título, leyenda, autor, derechos, lugar (lugar, ciudad, región, país); palabras clave jerárquicas («Lugares > Francia > París», cada nivel también en `dc:subject`); plantillas de metadatos para las fotos elegidas; en los JPEG exportados | ✅ |
| Mapa de las fotos geolocalizadas (posición GPS del EXIF): grupos de fotos cercanas, arrastrar, hacer zoom, tocar un grupo para ver sus fotos; mapa base de mosaicos libres guardados en el dispositivo, utilizable sin conexión; una foto sin posición se sitúa a mano (la mira en el centro, «Situar aquí», escrita en su XMP), toda una selección de una vez | ✅ |
| Fotos situadas a partir de un track GPX (`luma geotag --gpx`): la posición del track en el instante de cada toma (entre los dos puntos que la rodean), desfase del reloj de la cámara (`--offset +02:00`, o adivinado: `auto`), sin cruzar los huecos del track, y en la app («Seguir un track GPX…» en el mapa, Mac, Linux, Windows, y iPhone e iPad mediante la app Archivos), escrita en el XMP (`exif:GPSLatitude`); la fototeca y el mapa la leen, antes que la del archivo | ✅ |
| EXIF de las imágenes reveladas (JPEG, HEIC, AVIF, PNG, WebP, TIFF): fecha de la toma, cámara, objetivo, ajustes y posición GPS leídos como en un RAW; las fotos de un teléfono se ordenan por su fecha y aparecen en el mapa | ✅ |
| Búsqueda y filtros (`luma catalog --search`): palabras sin tener en cuenta los acentos; valoración, bandera, etiqueta, fechas, ISO, focal, cámara, objetivo, palabra clave; recuento de palabras clave, cámaras y objetivos | ✅ |
| Colecciones manuales e inteligentes (regla: valoración, fecha, cámara, objetivo, ISO, palabras clave…), guardadas en el catálogo | ✅ |
| Palabras clave en la app: escritura (varias a la vez, separadas por comas), añadidas y quitadas en todas las fotos seleccionadas, lista de palabras clave de la carpeta con su número, filtro de la cuadrícula | ✅ |
| Fototeca en la app: cada carpeta abierta entra en ella (en segundo plano); todas las fotos, las colecciones y las carpetas, cada una con su número de fotos; búsqueda y filtros; vistas ordenadas por fecha de captura | ✅ |
| Carpeta vigilada (captura conectada, tarjeta copiada): cada foto completa entra en la fototeca y en la tira, y se abre; `luma watch` | ✅ |
| Importación desde una tarjeta de memoria («Importar una tarjeta…», `luma import`): la tarjeta montada encontrada sola (carpeta DCIM), ordenación por fecha de toma, renombrado por plantilla, copia en un segundo disco, cada copia releída y comparada con el original, el RAW, su JPEG y su XMP con el mismo nombre, fotos ya importadas omitidas, la tarjeta nunca modificada, solo los días elegidos (una casilla por día de la toma, `--days`), y foto a foto sobre sus vistas previas («Elegir foto a foto», `--skip`), la tarjeta expulsada al final en Mac, Linux y Windows («Expulsar la tarjeta después de importar», `--eject`) | ✅ |
| iPhone y iPad: importación desde la app Archivos («Desde Archivos» bajo «Importar fotos», menú «•••»): una tarjeta de memoria en un lector, iCloud Drive, un disco; varias fotos a la vez (RAW, HEIC, JPEG…), copiadas en la carpeta de LÜMA como las de Fotos | ✅ |
| En Mac, cámara conectada (USB, o red si lo permite) mediante el marco del sistema: cada foto tomada llega a la carpeta vigilada, disparo desde LÜMA para las cámaras que lo permiten | ✅ |
| Copias virtuales: varios revelados de una foto sin duplicar el archivo (un XMP por copia), cada una con su valoración, sus palabras clave y su miniatura, junto a su original; exportada como `IMG_0001-2`; eliminada sin tocar el original; también en el catálogo (todas las fotos, búsquedas, colecciones), con la fecha y la cámara de su original | ✅ |
| Importación de catálogos existentes | ⏳ |
| Atajos de teclado personalizables («Atajos de teclado…»: cada comando, sus teclas, una tecla ya usada pasa al nuevo comando, vuelta a las teclas por defecto; «Raccourcis.txt»); Tab oculta los paneles; espacios de trabajo (Selección, Revelado, Retoque, Presentación y los suyos: herramienta y paneles), Cmd+1 a Cmd+9 | ✅ |

## Lo que distingue a LÜMA

- **No se envía nada a ninguna parte**: sin cuenta, sin nube obligatoria.
- **100 % Rust, verificado**: `scripts/audit-rust.sh` comprueba el grafo de dependencias en
  cada compilación (170 crates, ningún enlace nativo ni compilador de C).
- **Medido, no prometido**: `luma bench` y la CI publican los tiempos de renderizado reales.
