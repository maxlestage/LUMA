# Novedades

## 0.3.0 — próximamente

La fototeca profesional, paso a paso.

- **Más rápido**: la reducción de ruido de color (activa por defecto) es tres veces
  más rápida sin cambiar un solo píxel; la exportación a resolución completa tarda
  entre un 25 y un 30 % menos, y la vista previa con ajustes intensos responde un 25 %
  más rápido. Después los desenfoques: dos búferes en lugar de seis imágenes nuevas, y
  el enfoque a resolución completa casi el doble de rápido (NEF 0,64 → 0,36 s), sin
  cambiar un píxel. Después la escritura de los JPEG, en todos los núcleos: de 2,3 a
  2,6 veces más rápida, archivos idénticos byte a byte; una exportación a resolución
  completa de 24 Mpx en 1,8 s en lugar de 2,4.

- **Catálogo local**: `luma catalog` añade carpetas (subcarpetas incluidas) a la
  fototeca de LÜMA; fecha de captura, cámara, objetivo, ISO, apertura, velocidad y
  focal se leen en la cabecera de los RAW, sin decodificar la imagen (5 RAW en
  94 ms), y después solo se vuelven a leer los archivos modificados (menos de un
  milisegundo). Fotos ordenadas por fecha de captura.
- **Palabras clave**: leídas y escritas en el XMP (`dc:subject`), como hacen todas las
  aplicaciones de fotografía.
- **Búsqueda y filtros**: `luma catalog --search "mar rating:3 date:2024-06 iso:..800
  camera:sony"`; palabras (nombre, cámara, objetivo, etiqueta, palabras clave, sin
  tener en cuenta los acentos) y filtros (valoración, bandera, etiqueta, fechas, ISO,
  focal, cámara, objetivo, palabra clave). 100 000 fotos exploradas en 4 a 25 ms.
  `--keywords` cuenta palabras clave, cámaras y objetivos.
- **Colecciones** manuales (fotos elegidas, de cualquier carpeta) e inteligentes (una
  regla escrita como una búsqueda, «rating:4 date:2024 camera:sony», que sigue al
  catálogo): `luma catalog --smart`, `--add`, `--collections`, `--collection`.
- **Casilla «Correcciones de la cámara»** (Geometría): marcada por defecto; desmarcada,
  la foto vuelve a ser la que dio el objetivo, y los controles de distorsión y viñeteo
  toman el relevo. Guardada en el XMP.
- **Correcciones de objetivo de las OM System, Olympus y Panasonic**: la distorsión (y,
  en Olympus y OM System, la aberración cromática lateral) que la cámara registra en
  cada ORF o RW2 se aplica automáticamente. El corpus de prueba cuenta ahora con 7 RAW
  reales, entre ellos una OM‑5 Mark II y una Panasonic G9.
- **Correcciones de objetivo de las Fujifilm**: las que la cámara registra en cada RAF
  (distorsión, aberración cromática lateral, viñeteo), modo recortado incluido, se
  aplican automáticamente; por ejemplo el barrilete del XF 35 mm F2.
- **Correcciones de objetivo de las Sony**: la distorsión, la aberración cromática
  lateral y el viñeteo que la cámara registra en cada ARW se aplican automáticamente,
  con la ampliación justa para no dejar ningún borde vacío.
- **Correcciones de objetivo de los DNG**: la distorsión, la aberración cromática
  lateral y el viñeteo que la cámara o el programa escribieron en un DNG (opcodes
  `WarpRectilinear` y `FixVignetteRadial`) se aplican automáticamente, en la vista
  previa como en la exportación.
- **Fototeca más rápida en discos externos**: para archivar una foto, LÜMA ya solo
  lee el principio del archivo (0,5 a 2 MB en lugar de 25 a 56 MB). Cinco RAW del
  corpus: 34 MB leídos en lugar de 189; un NEF se archiva en 1,5 ms en lugar de 17 a
  23, un RAF en 0,6 ms en lugar de 22 a 25 (lecturas en frío).
- **Copias virtuales**: varios revelados de una misma foto sin duplicar el archivo;
  cada copia tiene sus ajustes, su valoración, sus palabras clave y su miniatura, junto
  a su original, y se exporta como `IMG_0001-2`. Eliminarla nunca toca el original.
- **La fototeca en la app** (Biblioteca): cada carpeta abierta entra en ella en
  segundo plano; todas las fotos, las colecciones y las carpetas, cada una con su
  número de fotos, y una búsqueda («mar rating:3 date:2024»); estas vistas se ordenan
  por fecha de captura, y la tira, la cuadrícula y las flechas las siguen.
- **Palabras clave en la app** (Biblioteca): se escriben (varias a la vez, separadas
  por comas) y van a todas las fotos seleccionadas (⌘‑clic, Mayús‑clic); una cruz
  quita una. La lista de palabras clave de la carpeta, con su número, filtra la
  cuadrícula con un clic (sin distinguir mayúsculas ni acentos).
- **Catálogo ligero**: un formato binario compacto en el que cada carpeta, cámara,
  objetivo y palabra clave se escribe una sola vez. Para 100 000 fotos: 7,4 MB en
  lugar de 33,6, leído en 68 ms en lugar de 166, guardado en 121 ms en lugar de 332.
  El catálogo anterior se lee y se convierte solo.
- **Ajustes preestablecidos de exportación**: formato, calidad, profundidad, espacio,
  tamaño y carpeta guardados con un nombre; cuatro de serie (web, resolución completa,
  pantalla P3, impresión) y los suyos, compartidos por la app y la línea de comandos
  (`luma export-preset`, `luma develop --preset Web`).
- **Nombres de los archivos exportados**: una plantilla como `{fecha}_{secuencia}` o
  `{año}/{mes}/{cámara}_{nombre}`, con la fecha de captura, la cámara, el objetivo, el
  ISO, la valoración, la etiqueta y el orden en la exportación; en la ventana Exportar,
  los ajustes preestablecidos y `luma develop --name`. Dos fotos con el mismo nombre no
  se sobrescriben (`-2`, `-3`…).
- **Dimensiones de exportación**: lado largo, lado corto, ancho × alto (la imagen cabe
  en el marco) o megapíxeles, sin ampliar nunca; en la ventana Exportar, los ajustes
  preestablecidos y `luma develop --resize short:1080`.
- **Enfoque de salida**: para pantalla, papel mate o papel brillante, bajo, estándar o
  alto, aplicado a la imagen en su tamaño final (en papel, el radio sigue el tamaño de
  la copia); las zonas lisas siguen lisas.
- **Marca de agua de texto**: «© su nombre» dibujado con la tipografía de LÜMA,
  suavizado al píxel, en una de nueve posiciones, con tamaño, opacidad, blanco o
  negro; en la ventana Exportar, los ajustes preestablecidos y `luma develop --watermark`.
- **Marca de agua con logotipo**: su logotipo en PNG, con su transparencia y sus
  colores reales, en una de nueve posiciones, con tamaño y opacidad; solo o con el
  texto; en la ventana Exportar, los ajustes preestablecidos y
  `luma develop --logo firma.png`.
- **Metadatos de exportación**: el EXIF de la toma (cámara, objetivo, ajustes, fecha)
  copiado en los JPEG y PNG, su nombre y su copyright, palabras clave, valoración y
  etiqueta; la ubicación GPS se quita por defecto, para que una foto compartida no diga
  dónde vive (`--keep-gps` la conserva). Los TIFF también los reciben (EXIF, XMP), con
  exactamente los mismos píxeles que antes.
- **Exportación a varios ajustes preestablecidos a la vez**: marque «Web» e
  «Impresión» en la ventana Exportar y cada foto va a los dos de una vez; guarde sus
  ajustes como preestablecido; `luma export *.ARW --preset Web --preset Impression`.
- **Máscaras por rango**: la zona de una máscara (degradado, pincel, imagen entera)
  puede quedarse solo con algunos tonos (rango de luminancia: mínimo, máximo,
  suavizado) o algunos colores (rango de color: tono, anchura, suavizado; los grises
  quedan fuera). Medidos en OKLab sobre la foto, antes de los ajustes de las máscaras;
  la zona se muestra en rojo (O).
- **Pincel que sigue los bordes**: marque «Seguir los bordes» y el pincel solo pinta
  los tonos cercanos al centro de cada toque; la zona se detiene en el contorno de un
  cielo, una pared o una cara, sin desbordar.
- **Combinaciones de máscaras**: «Combinar» añade, resta o interseca otra forma
  (degradado lineal o radial, pincel, cada una invertible); un radial al que se le
  resta otro se convierte en un anillo, un pincel restado protege una cara bajo un
  degradado.
- **Perfiles ICC leídos** (un paso hacia la prueba en pantalla): perfiles de
  pantallas (matriz y curvas) y de impresoras y papeles (tablas lut8, lut16, A→B y
  B→A, versiones 2 y 4), en ambos sentidos y para las cuatro intenciones de
  reproducción, en Rust puro; un perfil truncado o dañado se rechaza sin daños.
  `luma icc` describe un perfil: clase, espacio, intenciones, blanco del soporte, y
  el blanco, el negro y los primarios del dispositivo.
- **Prueba en pantalla** (bajo el histograma): elija el perfil ICC de una impresora y
  un papel, y la vista previa muestra la foto tal como saldrá, con la intención
  perceptual o colorimétrica relativa; el histograma la sigue. Una vista, no un
  ajuste: no se escribe nada en la foto.
- **Aviso de gama** (en la prueba en pantalla): los colores que la impresora no podrá
  reproducir se vuelven azules en la vista previa, con o sin la simulación de la
  impresión.
- **Papel y tinta negra** (en la prueba en pantalla): «Color del papel» muestra el
  tono y la luminosidad del papel, «Tinta negra» el negro real de la tinta, más claro
  que el de la pantalla; sin ellas, el blanco del papel y el negro de la tinta se
  llevan al blanco y al negro de la pantalla.

## 0.2.0 — octubre de 2026

Todos los formatos de imagen, tres idiomas y la tienda.

- **Todos los formatos de imagen**: JPEG, PNG, TIFF, WebP, GIF, BMP, OpenEXR, HDR, JPEG XL, PSD,
  JPEG 2000, SVG… y HEIC, HEIF, AVIF del iPhone (Mac, iPhone y iPad); el perfil «Original»
  abre la imagen tal cual, y todas las herramientas se aplican.
- **Tres idiomas**: francés, inglés y español (app y web).
- **LÜMA Premium con Stripe**: la clave de licencia aparece justo después del pago,
  la app la verifica sin conexión y se renueva cada día.
- **App de Mac lista para la firma Developer ID** y la notarización de Apple.

## 0.1.0 — octubre de 2026

La primera versión para descargar: 14 días de prueba y, después, LÜMA Premium.

- **Revelado RAW**: un módulo Revelar completo, panel por
  panel, motor en coma flotante de 32 bits, acabado en la GPU (Metal), ajustes en XMP.
- **735 cámaras** reconocidas, 714 de ellas con color calibrado; cualquier DNG, incluido el iPhone
  ProRAW.
- **100 películas** verificadas por la máquina (películas en color, cine, blanco y negro,
  instantáneas); **tus propias películas medidas** a partir de un escaneo, como LUT 3D; importación de
  LUT `.cube`.
- **Máscaras** lineal, radial y pincel; **películas apiladas** mediante máscaras, completas.
- **Apilamientos** (ruido, mediana, estelas, cielo estrellado) y **panorámicas**, en DNG.
- **Ajustes preestablecidos**: los tuyos, importados o exportados como ajustes
  preestablecidos XMP.
- **Mac** (Apple Silicon e Intel), **iPhone y iPad** (en preparación para la App
  Store), línea de comandos `luma`.
