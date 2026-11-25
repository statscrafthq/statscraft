# Las partes comunes de todas las ventanas: botones de los cuadros de diálogo, diálogos de
# archivo, barra de estado, registro, preguntas antes de cerrar, Acerca de.

button-ok = Aceptar
button-paste = Pegar
button-reset = Restablecer
button-cancel = Cancelar
button-close = Cerrar
button-save = Guardar
button-dont-save = No guardar
dialog-syntax = Sintaxis

filter-data-file = Archivo de datos
filter-compressed-data-file = Archivo de datos comprimido
filter-excel-workbook = Libro de Excel
filter-web-page = Página web
filter-pdf-document = Documento PDF
filter-png-image = Imagen PNG
filter-svg-image = Imagen SVG

window-data = Datos
window-untitled = Sin título
window-active = { $title } (activo)
window-drop-files = Suelte archivos de datos, libros, texto, .sps o .scout para abrirlos
window-one-data-one-syntax = se abre un archivo de datos y uno de sintaxis cada vez
window-opens = Statscraft abre archivos .sav, .zsav, .xlsx, .xlsm, .xls, .csv, .txt, .tab, .dat, .sps y .scout
window-not-opened = no se abrieron: { $files } ({ $reasons })

status-ready = Listo
status-saved-syntax = sintaxis guardada en { $file }
status-saved-chart = gráfico guardado en { $file }
status-exported = resultados exportados a { $file }
status-chart-no-picture = este tipo de gráfico todavía no tiene imagen
status-undone = deshecho: { $syntax }
status-redone = rehecho: { $syntax }
status-wait-to-delete-output = espere a que termine la ejecución para eliminar resultados
status-opening = Abriendo { $file }
status-running = Ejecutando { $what }
status-waiting = { $count } en espera
status-warnings =
    { $count ->
        [one] 1 advertencia, vea el registro
       *[other] { $count } advertencias, vea el registro
    }
status-split-layered = Segmentado por { $variables } (por capas)
status-split-separate = Segmentado por { $variables } (por separado)
status-filter = Filtro: { $name }
status-weight = Ponderación: { $name }

log-title = Registro
log-copy-syntax = Copiar sintaxis
log-undone = { $summary }; deshecho
log-warning = advertencia: { $warning }

unsaved-quit = ¿Desea guardar los cambios antes de cerrar?
unsaved-syntax = ¿Desea guardar antes los cambios de la sintaxis?
unsaved-output = ¿Desea guardar antes los resultados?
unsaved-data = ¿Desea guardar antes los cambios de los datos?
unsaved-data-item = Datos: { $name }
unsaved-dataset-no-file = { $name }, todavía sin guardar en un archivo
unsaved-no-file = todavía sin guardar en un archivo
unsaved-syntax-item = Sintaxis: { $name }
unsaved-output-file = Resultados: { $name } ({ $file })
unsaved-output-no-file = Resultados: { $name }, todavía sin guardar en un archivo

about-title = Acerca de Statscraft
about-version = Versión { $version }
about-what = Estadística con un lenguaje de comandos, sobre archivos de datos .sav.
about-license = Licencia: MIT
about-source = Código fuente
about-themes = Temas de color
about-fonts = Fuentes

import-sheet = Hoja
import-names-first-row = Leer los nombres de las variables de la primera fila
import-range = Rango
import-whole-sheet = toda la hoja
import-delimiter = Delimitador
import-comma = Coma
import-semicolon = Punto y coma
import-tab = Tabulador
import-names-first-line = Leer los nombres de las variables de la primera línea
import-first-line = Primera línea

# La barra de actualizaciones bajo el menú.
update-checking = Buscando actualizaciones
update-up-to-date = Statscraft { $version } está actualizado.
update-no-release = Todavía no se ha publicado ninguna versión de Statscraft.
update-check-failed = La búsqueda de actualizaciones falló: { $error }
update-failed = La actualización falló: { $error }
update-available = Statscraft { $version } está disponible (usted tiene la { $current }).
update-release-notes = Notas de la versión
update-unmanaged = Esta copia se ejecuta desde { $folder }, que la actualización no puede sustituir: instale la versión nueva de la misma forma que instaló esta, o use el .deb o el archivo tar de las notas de la versión.
update-download = Descargar
update-mac-replace = Después sustituya Statscraft.app por la de la descarga.
update-now = Actualizar ahora
update-windows-restart = La ventana se cierra y se vuelve a abrir: guarde antes su trabajo.
update-later = Más tarde
update-downloading = Descargando y comprobando Statscraft { $version }
update-installed = Statscraft { $version } está instalado. Reinicie la ventana para usarlo; guarde antes su trabajo.
update-restart-now = Reiniciar ahora
update-saved = El paquete nuevo está comprobado y guardado como { $file }. Instálelo en una terminal con
update-copy-command = Copiar comando
update-hint-windows = Descarga el instalador y lo comprueba con las sumas de verificación de la versión. Después la ventana se cierra, el instalador actualiza Statscraft y vuelve a abrir la ventana.
update-hint-programs = Descarga los programas nuevos, los comprueba con las sumas de verificación de la versión y sustituye los de { $folder }.
update-hint-deb = Descarga el paquete a su carpeta de descargas, lo comprueba con las sumas de verificación de la versión y lo instala; el sistema le pide su contraseña.
