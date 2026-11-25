# Statscraft

[English](README.md) · Español · [简体中文](README.zh-CN.md)

Esta traducción sigue el README.md en inglés de la versión 0.48.0. Si los dos textos no coinciden, vale el inglés.

Statscraft es un programa de estadística libre y de código abierto para las ciencias sociales, pensado desde la primera línea para trabajar con agentes de IA. Es una alternativa a PSPP, Stata y SAS: lee archivos .sav y ejecuta la sintaxis que ya tiene, en Windows, macOS y Linux. Dos programas escritos en Rust comparten un mismo motor: la línea de comandos `statscraft` y la ventana de escritorio `statscraft-gui`.

[Descargar](https://statscraft.org/download.html) · [Documentación](https://docs.statscraft.org) · [Sitio web](https://statscraft.org)

## Qué hace

- **Archivos de datos.** Lee y escribe archivos .sav y .zsav con sus etiquetas, valores perdidos y conjuntos de respuestas múltiples. Abre libros de Excel y archivos de texto, y escribe archivos de Excel, CSV y Stata.
- **Preparación de datos.** Calcular, recodificar, seleccionar, ponderar, agregar, fusionar y reestructurar, con macros y el lenguaje matricial.
- **Tablas personalizadas.** Recuentos, porcentajes y medias por grupos, con totales, subtotales, conjuntos de respuestas múltiples y pruebas de significación marcadas con letras.
- **Estadística.** Frecuencias, tablas cruzadas, medias, pruebas t y análisis de varianza; regresión lineal, logística y muchas otras; modelos lineales generales, generalizados y mixtos; análisis factorial, de conglomerados y de árboles; análisis de supervivencia, previsión, muestras complejas, pruebas bayesianas y análisis de potencia.
- **Resultados.** Tablas y gráficos a Excel, Word, PowerPoint, PDF y páginas web. Los gráficos llegan a los programas de oficina como gráficos que se pueden editar.
- **Agentes de IA.** Un servidor MCP, salida en JSON, sesiones que conservan los datos entre ejecuciones y errores con línea y columna, para que un agente haga el análisis mientras usted lo sigue en la ventana.

Las [páginas de comandos](docs/commands/README.md) (en inglés) describen cada comando.

Statscraft es un proyecto independiente. No está afiliado ni respaldado por los fabricantes de otros programas de estadística ni por la suite ArtCraft. Los nombres de productos son marcas de sus propietarios.

Estado: en desarrollo temprano. Licencia MIT.

## Documentación

La documentación está en [docs.statscraft.org](https://docs.statscraft.org) y las mismas páginas están en `docs/`. Por ahora está en inglés.

- [Primeros pasos](docs/getting-started.md): de la instalación a una tabla personalizada en Excel, con un archivo de ejemplo.
- [Instalar y actualizar](docs/install.md): cada paquete, la desinstalación y las actualizaciones.
- [La ventana](docs/window.md): el editor de datos, el editor de sintaxis, los resultados, los menús y las tablas personalizadas.
- [La línea de comandos](docs/command-line.md): cada subcomando y opción, las sesiones, la salida en JSON, los códigos de salida, la exportación a Excel y la sesión en vivo de la ventana.
- [Bases de datos](docs/databases.md): guardar conjuntos de datos en SQLite, PostgreSQL, MySQL, MariaDB o Dolt, y recodificarlos allí.
- [Agentes de IA](docs/ai-agents.md): cómo debe trabajar un agente con Statscraft e instrucciones para pegar en el suyo.
- [Conectar su agente de IA](docs/connect.md): añadir Statscraft a los agentes de IA de su ordenador.
- [El servidor MCP](docs/mcp.md): las herramientas que recibe una aplicación de agentes, las sesiones y la seguridad.
- [Complementos](docs/plugins.md): programas que colorean y comentan las celdas de las tablas personalizadas.
- [Comandos](docs/commands/README.md): una página por comando.
- [Registro de cambios](CHANGELOG.md): qué trae cada versión.

## Contribuir

[CONTRIBUTING.md](CONTRIBUTING.md) explica cómo compilar desde el código fuente, ejecutar las pruebas y publicar una versión. Las [notas para desarrolladores](docs/developers/README.md) y [Escribir un complemento](docs/developers/plugins.md) contienen los detalles que hay detrás de las guías de usuario.
