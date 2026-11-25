# La ventana de resultados: barra de herramientas, esquema y menú de una tabla o un gráfico.

output-none-yet = Todavía no hay resultados
output-copy = Copiar
output-copy-hint = Copiar las tablas seleccionadas con su formato (Ctrl+C)
output-copy-as-text = Copiar como texto
output-copy-as-text-hint = Texto separado por tabuladores, para una hoja de cálculo
output-delete = Eliminar
output-delete-hint = Eliminar las tablas seleccionadas (Supr)
output-export-hint = Exportar las tablas seleccionadas
output-select-all = Seleccionar todo
output-clear = Vaciar resultados
output-selected =
    { $count ->
        [one] 1 tabla seleccionada
       *[other] { $count } tablas seleccionadas
    }
output-show-element = Mostrar en los resultados; Editar > Configuración muestra u oculta tipos completos
output-heading = Encabezado
output-title = Título
output-notes = Notas
output-run-again = Ejecutar de nuevo
output-syntax-copied = sintaxis copiada
output-selected-hidden = las tablas seleccionadas están ocultas: no se copió nada
output-chart-copied = gráfico copiado: se pega como un gráfico editable
output-copied =
    { $count ->
        [one] 1 tabla copiada
       *[other] { $count } tablas copiadas
    }
output-copied-as-text =
    { $count ->
        [one] 1 tabla copiada como texto
       *[other] { $count } tablas copiadas como texto
    }
output-copied-no-rich-text =
    { $count ->
        [one] 1 tabla copiada como texto: el portapapeles no aceptó texto enriquecido
       *[other] { $count } tablas copiadas como texto: el portapapeles no aceptó texto enriquecido
    }
output-show-chart = Mostrar gráfico
output-show-data = Mostrar datos
output-save-png = Guardar gráfico como PNG...
output-save-svg = Guardar gráfico como SVG...
output-open-in-powerpoint = Abrir en PowerPoint
output-open-in-excel = Abrir en Excel
output-open-in-word = Abrir en Word
