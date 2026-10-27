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
  completa de 24 Mpx en 1,8 s en lugar de 2,4. Después la etapa de renderizado,
  calculada de ocho en ocho píxeles: de 1,8 a 1,9 veces más rápida (NEF 160 → 85 ms),
  archivos idénticos byte a byte. Después el color (perfil y Saturación), igual: tres
  veces más rápido (NEF 147 → 48 ms). Después la codificación JPEG, otro 13 a 15 % más
  corta (24 Mpx: 200 → 175 ms), mismos bytes. Después las correcciones de objetivo
  grabadas por la cámara (Sony, Fujifilm, Olympus, Panasonic), ocho píxeles a la vez:
  un 30 % más rápidas (ARW de 24 Mpx: 385 → 270 ms), mismos archivos byte a byte.

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
- **Zona de la máscara a su gusto**: la zona mostrada (O) puede ser roja, verde,
  azul, blanca o blanca sobre negro, con una opacidad del 10 al 100 %.
- **Corrector y tampón** (Q): un clic sobre una mota de polvo o un detalle molesto, y
  LÜMA elige la parte de la foto que lo sustituye. El corrector conserva su textura y
  toma la luz del borde de la zona; el tampón hace una copia exacta. Zona y fuente se
  mueven con el ratón; tamaño, difuminado, opacidad. Guardados en el XMP.
- **Interpolación cromática direccional**: el verde de cada píxel sigue los contornos
  en lugar de cruzarlos; ramas, cabello y texturas finas ya no muestran punteado ni
  efecto cremallera (4 dB mejor en nuestras pruebas). Las copias de trabajo se rehacen
  una vez.
- **Colores igualados entre dos cámaras**: la misma escena fotografiada con tus dos
  cámaras, aunque esté encuadrada algo distinto; elige las dos fotos y luego «Igualar
  con la foto elegida» (Mezclador de canales), o `luma match-colors`. La segunda cámara
  toma los colores de la primera; guardada como ajuste preestablecido, la corrección se
  aplica a todas sus fotos.
- **Perfil de cámara**: fotografía una carta ColorChecker, recórtala al justo y luego
  «Perfil desde una carta» (Mezclador de canales) o
  `luma colorchecker carta.ARW -o perfil.xmp`: LÜMA calcula la corrección que hace
  exactos sus 24 colores (de 3,8 a 0,02 de error en nuestras pruebas), con el blanco
  neutro. Guardada como ajuste preestablecido, se aplica a todas las fotos de esa
  cámara.
- **Apilado de enfoque** (Fusión, «Enfoque», o `luma merge --stack focus`): las tomas
  cuyo enfoque avanza, en macro o en paisaje, se convierten en una sola foto nítida del
  primer plano al fondo. Cada toma se alinea con su vecina, y LÜMA toma lo más nítido de
  cada una.
- **Fusión HDR** (Fusión, «HDR», o `luma merge --stack hdr`): las tomas en
  horquillado, incluso a pulso, se convierten en un único DNG donde las sombras y el
  cielo conservan su detalle. LÜMA mide por sí mismo la diferencia de exposición entre
  tomas, y lo que se movió de una a otra (follaje, transeúntes) no deja fantasmas.
- **Altas luces reconstruidas**: la casilla «Reconstruir las altas luces» (Luz, bajo
  Tono) devuelve a un canal saturado por el sensor el color de sus vecinos; un cielo
  quemado vuelve a ser anaranjado, una toalla blanca recupera su textura. Un píxel
  donde todo se satura queda blanco. También en la línea de comandos
  (`--set highlights.reconstruction=on`).
- **Enfoque por deconvolución**: la casilla «Deconvolución» de Enfoque deshace el
  desenfoque del objetivo en lugar de acentuar los bordes; vuelve el detalle real, sin
  halos (1,6 dB mejor que la máscara de enfoque en nuestras pruebas). Visible al 100 %
  y al exportar; también en la línea de comandos (`--set sharpen.deconvolution=on`).
- **Reducción de ruido por ondículas**: la casilla «Ondículas» de Detalle suaviza la
  luminancia escala por escala, cada una según el ruido medido en la foto; con la misma
  intensidad, conserva más textura y contornos que el filtro habitual (3,7 dB mejor en
  nuestras pruebas), por aproximadamente el doble de tiempo. Sin IA.
- **Verticales automáticas** (Mayús+V, o el botón de la Transformación): un edificio
  fotografiado apuntando hacia arriba recupera sus paredes rectas; LÜMA mide dónde se
  juntan sus verticales y ajusta la Vertical en consecuencia.
- **Enderezado automático** (Mayús+R, o «Enderezar automáticamente» en el recorte):
  LÜMA lee el horizonte y las verticales de la foto y la endereza; en caso de duda (un
  primer plano hecho de líneas de fuga), no toca nada.
- **Guías de recorte**: tercios, proporción áurea, diagonales, triángulos áureos,
  espiral áurea y cuadrícula; O pasa a la guía siguiente durante el recorte, Mayús+O
  gira la espiral y los triángulos.
- **Mezclador de canales** (herramienta Color): cada canal rojo, verde o azul hecho de
  una mezcla de los tres canales de la foto, de −200 a +200 %; guardado en el XMP y
  en los ajustes preestablecidos, sin ralentizar el renderizado.
- **Muaré y desflecado con el pincel**: dos controles más en cada máscara. «Muaré»
  suaviza los colores parásitos de un tejido o una rejilla, «Desflecado» quita los
  bordes coloreados a lo largo de los contornos; la luminosidad no cambia.
- **Ojos rojos**: un círculo en cada ojo, y la pupila roja vuelve a ser negra; la piel
  de alrededor y el reflejo blanco del ojo no cambian.
- **Manchas del sensor**: «Ver las manchas» hace resaltar el polvo del sensor como
  puntos oscuros; «Quitar el polvo» lo encuentra en las zonas uniformes (un cielo,
  una pared) y pone un corrector en cada mancha, sin tocar texturas, cables ni el
  desenfoque del fondo.
- **Perfiles ICC leídos** (un paso hacia la prueba en pantalla): perfiles de
  pantallas (matriz y curvas) y de impresoras y papeles (tablas lut8, lut16, A→B y
  B→A, versiones 2 y 4), en ambos sentidos y para las cuatro intenciones de
  reproducción, en Rust puro; un perfil truncado o dañado se rechaza sin daños.
  `luma icc` describe un perfil: clase, espacio, intenciones, blanco del soporte, y
  el blanco, el negro y los primarios del dispositivo.
- **Más rápido, más ligero**: las correcciones de objetivo grabadas en los RAW (Sony,
  Fujifilm, OM System, Panasonic) ya no copian la imagen, entre un 17 y un 35 % más
  rápidas a resolución completa (ARW 602 → 408 ms, RW2 523 → 339 ms), con píxeles
  idénticos bit a bit; los duplicados de trabajo, entre un 5 y un 9 % más ligeros y
  leídos hasta un 17 % más rápido (CR3 331 → 274 ms), siempre sin pérdida.
- **Prueba en pantalla** (bajo el histograma): elija el perfil ICC de una impresora y
  un papel, y la vista previa muestra la foto tal como saldrá, con la intención
  perceptual o colorimétrica relativa; el histograma la sigue. Una vista, no un
  ajuste: no se escribe nada en la foto.
- **iPhone más sencillo**: una flecha Deshacer en la barra superior y un menú «•••»
  para lo demás (rehacer, copiar, pegar, restablecer, idioma); un botón «Antes» que
  dice lo que muestra; cada herramienta nombrada bajo su icono; en la hoja de
  herramientas, primero los controles.
- **LÜMA Premium en el App Store** (iPhone y iPad): las dos suscripciones a su precio del
  App Store, «Suscribirse» y «Restaurar compras»; cada compra y renovación abre
  Premium. En iPhone y iPad, solo el App Store vende la suscripción; en Mac, solo el
  sitio web (Stripe).
- **Aviso de gama** (en la prueba en pantalla): los colores que la impresora no podrá
  reproducir se vuelven azules en la vista previa, con o sin la simulación de la
  impresión.
- **Papel y tinta negra** (en la prueba en pantalla): «Color del papel» muestra el
  tono y la luminosidad del papel, «Tinta negra» el negro real de la tinta, más claro
  que el de la pantalla; sin ellas, el blanco del papel y el negro de la tinta se
  llevan al blanco y al negro de la pantalla.
- **Carpeta vigilada** (Biblioteca): las fotos que llegan a una carpeta (cámara
  conectada, tarjeta copiada) entran en la fototeca y en la tira en cuanto están
  completas, y la última se abre (o solo se añade, a elegir); `luma watch carpeta`
  hace lo mismo desde la línea de comandos.
- **Cámara conectada, en Mac** (Carpeta vigilada): «Conectar una cámara» abre la
  cámara enchufada (USB, o red si lo permite) mediante el marco del sistema; cada
  foto tomada llega a la carpeta vigilada y se abre, las que ya están en la tarjeta
  se quedan donde están; «Disparar» para las cámaras que se disparan a distancia.
- **Selección a pantalla completa** (F): la foto sola, valorada con el teclado (0 a
  5), con sus banderas (P, X, U) y sus etiquetas de color (6 a 9: rojo, amarillo,
  verde, azul; morado con el ratón); la foto siguiente llega por sí sola (A para
  desactivarlo), Esc para salir. Las etiquetas se escriben en el XMP con los nombres
  del formato, que leen los demás programas de fotos, y se buscan en francés, inglés
  o español.
- **Comparación** (C): 2 a 4 fotos lado a lado o en cuadrícula (las elegidas, si no
  la foto abierta y la siguiente), cada una revelada con sus ajustes; el zoom y el
  desplazamiento son comunes a todas, la lupa (L) amplía el mismo punto de cada una,
  un clic abre la foto.
- **iPhone más simple y más sobrio**: la pantalla de inicio ya no se redibuja dos
  veces por segundo sin motivo (solo durante una importación desde Fotos); la barra
  superior muestra solo lo útil: sin foto, ni «Después» ni exportar; la prueba de impresión ya no ocupa sitio en iPhone y iPad.
- **Ráfagas y series de exposición apiladas**: las fotos de una misma cámara con
  menos de dos segundos de diferencia se pliegan tras la primera («Ráfaga ×5»,
  «Serie ×3»); S las despliega, «Fusionar la serie» abre la herramienta Fusión con
  las fotos de una serie de exposición.
- **Duplicados** («Buscar duplicados»): la misma imagen en varios archivos, copiada,
  exportada a otro tamaño o recomprimida, encontrada por su huella y mostrada grupo
  por grupo. **En lote**: con varias fotos elegidas, la valoración, la bandera y la
  etiqueta van a todas.
- **Apertura dos veces más rápida**: los niveles del sensor (negro, blanco) se
  normalizan directamente desde sus enteros, en todos los núcleos, sin copia previa
  a flotantes; NEF 145 → 67 ms, ARW 143 → 65, RW2 131 → 65, RAF 169 → 73, DNG
  285 → 158, CR3 547 → 399, datos idénticos bit a bit.
- **Sincronizar una selección** (Mayús+Cmd+S, o «Sincronizar…» en cuanto hay dos
  fotos elegidas): los ajustes de la foto abierta pasan a las demás, grupo por grupo
  (recorte y máscaras propios de cada una por defecto). **Balance auto en lote**:
  cada foto elegida recibe su propio balance automático.
- **Ajustes predeterminados por cámara y por ISO** («Ajustes predeterminados…»,
  herramienta Luz): los ajustes de la foto abierta pasan a ser el punto de partida de
  las fotos de esa cámara sin XMP, a cualquier ISO o a partir de su ISO (gana la regla
  más precisa; salvo recorte y máscaras). **Película de importación**: elegida en la
  carpeta vigilada, se da a cada foto que llega.
- **Información IPTC** (Biblioteca, «Información»): título, leyenda, autor, derechos
  y lugar de la toma, para una foto o todas las fotos elegidas, en el XMP y en los JPEG
  exportados; **palabras clave jerárquicas** («Lugares > Francia > París»);
  **plantillas de metadatos** (autor, derechos, lugar, palabras clave) guardadas y
  aplicadas de una vez.
- **Mapa** (Biblioteca, «Mapa»): las fotos de la tira donde se tomaron (posición GPS
  del EXIF, guardada en la fototeca), agrupadas cuando están cerca; arrastrar, hacer
  zoom con la rueda o con dos dedos, tocar un grupo para ver sus fotos. Mapa base de
  mosaicos libres, descargados por el sistema y guardados en el dispositivo; sin
  conexión, los ya guardados.

- **Instrumentos de medida** (encima del histograma, o Mayús+J): **forma de onda**
  (la luminancia, columna por columna), **desfile RGB** (cada canal por separado) y
  **vectorscopio** (tono y saturación, con los objetivos de los primarios al 75 % y la
  línea de tonos de piel), calculados sobre la vista previa en cada render, prueba en
  pantalla incluida.
- **Medición puntual**: bajo el histograma, los valores RGB (sRGB, de 0 a 255) y Lab
  del punto bajo el puntero, leídos sobre los colores reales aunque se muestre el
  recorte. **Recorte por canal**: en la foto y en la esquina del histograma, el color
  indica qué canales están quemados (rojo, amarillo para rojo y verde…, blanco para
  los tres).
- **Renombrado por lotes** («Renombrar…» en las fotos seleccionadas, `luma rename`):
  una plantilla con los marcadores de la exportación (`{date}_{séquence}`,
  `{boîtier}-{nom}`…), con una vista previa de los nuevos nombres antes de confirmar;
  el RAW y el JPEG de una misma toma conservan un mismo nombre, el XMP y las copias
  virtuales los acompañan, un nombre ya ocupado recibe `-2`; ante cualquier fallo, todo
  vuelve a como estaba. La fototeca y sus colecciones lo siguen.
- **Instantáneas** (herramienta Luz, bajo el historial): los ajustes de un momento, con
  un nombre, guardados en el XMP de la foto; un clic vuelve a ellos (un paso del
  historial, que se puede deshacer).
- **Copias de seguridad del catálogo**: una por semana, sola, en «Sauvegardes» junto al
  catálogo (se guardan las diez últimas); `luma catalog --backup`, `--backups`,
  `--restore` (el catálogo reemplazado se guarda antes). **Fotos que faltan**:
  `--missing` las lista, `--relocate antigua nueva` recupera una carpeta movida fuera de
  LÜMA, colecciones incluidas.
- **Impresión** («Imprimir…» en las fotos seleccionadas, `luma print`): un PDF al
  tamaño exacto del papel (A4, A3, A3+, 10 × 15 cm…), vertical u horizontal, márgenes,
  una foto por página o una **hoja de contactos** (cuadrícula, nombre bajo cada foto),
  fotos enteras o llenando su celda, enfoque para papel mate o brillante, vista previa
  de la página; páginas convertidas al **perfil de la impresora** (el de la prueba en
  pantalla, con compensación del punto negro), para imprimir sin la gestión de color
  del controlador.
- **Libro de fotos** («Libro, presentación, galería…», `luma book`): un PDF con su portada (la
  primera foto, el título y el subtítulo) y luego páginas de una, dos o cuatro fotos
  elegidas según su orientación; formatos 20 × 20, 30 × 30, A4, 21 × 28, A3.
- **Presentación**: a pantalla completa en la app (la foto sola, avanza por sí misma,
  Espacio para pausar, Esc para salir) y **exportada** (`luma slideshow`): una carpeta
  autónoma, index.html y las fotos, para abrir en cualquier navegador o publicar
  (fundido, flechas, deslizamiento del dedo, pantalla completa).
- **Galería web** («Libro, presentación, galería…», `luma gallery`): una carpeta
  estática para publicar tal cual, la cuadrícula de miniaturas y cada foto en grande con
  un clic (flechas, deslizamiento del dedo, Esc), con título, descripción y pies de foto.
- **JPEG optimizado para la web**: crominancia 4:2:0 y tablas de Huffman calculadas para
  cada imagen, de un 9 a un 29 % más ligero en las fotos de prueba, con una diferencia
  apenas medible; en el ajuste Web, la ventana de exportación («Optimizado para la web»)
  y `--web`.
- **Vídeo básico** (`luma video`): una película de LÜMA («Portra 400», «Tri-X 400»…),
  un ajuste XMP o unos ajustes aplicados a cada imagen de un vídeo corto en Motion JPEG
  (el formato AVI de muchas cámaras), guardado de nuevo en el mismo formato: mismo
  tamaño, misma cadencia, el sonido copiado tal cual. Leído y escrito en Rust puro.
- **Importación desde una tarjeta** («Importar una tarjeta…» en la Fototeca, `luma
  import`): la tarjeta montada se encuentra sola; las fotos se ordenan por fecha de
  toma, se renombran si se quiere y se copian también en un segundo disco; cada copia
  se relee y se compara con el original, el RAW, su JPEG y su XMP conservan el mismo
  nombre, una foto ya importada se omite y la tarjeta nunca se modifica.
- **Atajos de teclado personalizables** (menú del idioma, «Atajos de teclado…»): cada
  comando y sus teclas; hacer clic y pulsar las nuevas teclas; una tecla ya usada pasa
  al nuevo comando; vuelta a las teclas por defecto, una a una o todas. **Tab** oculta
  los paneles.
- **Espacios de trabajo**: Selección, Revelado, Retoque, Presentación y los que se
  guardan (la herramienta y los paneles), de Cmd+1 a Cmd+9.
- **iPhone: la cuadrícula de fotos realineada**: columnas calculadas según el ancho de
  la pantalla, con los márgenes de las islas, el nombre encima de la foto y ya no sobre
  ella; la barra Cuadrícula, Mapa y Pase de diapositivas cabe en una línea («Libro,
  pase de diapositivas, galería…» pasa a «•••»); solo el histograma en el panel Fotos.
  Tras una actualización de la app, las fotos siguen a la nueva carpeta de la app en
  lugar de aparecer en un segundo «Documents».
- **iPhone y iPad: el toque cae donde debe**: la imagen por fin tiene el tamaño de la
  pantalla (estaba hecha para la zona segura y luego estirada un 11 % en altura); se
  ve el final de los paneles (Instantáneas, bajo el historial), el dedo desplaza el
  panel y toca lo que apunta. La barra superior contiene sus botones de 40 puntos.
- **iPhone y iPad: la exportación va a la fototeca**: cada imagen exportada se añade a
  la app Fotos tal cual (EXIF, IPTC y perfil de color conservados; iOS pide permiso la
  primera vez), y también queda en LÜMA/ (app Archivos). La ventana Exportar cabe en la
  pantalla del iPhone, cada ajuste bajo su título.
- **Películas en familias plegables**: color, blanco y negro, cine, instantáneas, mis
  ajustes, mis películas; cada título indica cuántas películas tiene la familia, y al
  principio solo se abre la de la película aplicada.
- **150 películas**: 50 más. Color (Kodacolor 200, Portra 160VC y 400VC, Ektachrome
  64, Fujicolor 100 Industrial, Superia Venus 800, Sensia 100, Precisa 100, Agfa
  Portrait 160, Konica VX 200, E100VS, Phoenix 200, Lomochrome Purple y Metropolis,
  Aerochrome), cine (EXR 200T, Eterna 400T, F-64D, XTR 250, Super 16, 70 mm, Neón,
  Terror, Romance, Documental, Veranillo de San Martín, Thriller urbano), blanco y negro
  (Delta 400, Kentmere 400, SFX 200, Fomapan 100, RPX 25, Pancro 400, CHS 100 II, T-Max
  P3200, Pan 400, Verichrome Pan), instantáneas (Polaroid i-Type, Spectra y 669, Instax
  Wide, Impossible Project, Instamatic) y creativos (Pastel, Otoño, Invierno, Primavera,
  Bruma, Contraluz, Bicromía). Todas verificadas en la carta como las demás:
  distintas, sin imagen aplastada, blanco y negro neutros, mismos renders en la GPU.

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
