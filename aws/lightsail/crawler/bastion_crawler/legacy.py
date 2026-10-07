from datetime import datetime

def extraer_formulario(sopa):
    form = sopa.find("form")
    if form is None:
        raise RuntimeError("No se encontro el formulario ASP.NET en la pagina.")

    datos = {}

    for campo in form.find_all(["input", "select", "textarea"]):
        nombre = campo.get("name")
        if not nombre:
            continue

        if campo.name == "select":
            opcion = campo.find("option", selected=True) or campo.find("option")
            datos[nombre] = opcion.get("value", "") if opcion else ""
            continue

        if campo.name == "textarea":
            datos[nombre] = campo.get_text()
            continue

        tipo = (campo.get("type") or "text").lower()

        if tipo in ("submit", "button", "image", "file", "reset"):
            continue

        if tipo in ("checkbox", "radio") and not campo.has_attr("checked"):
            continue

        datos[nombre] = campo.get("value", "")

    return datos

def texto_limpio(elemento):
    return elemento.get_text(" ", strip=True).replace("\xa0", " ")

def convertir_fecha(fecha_texto):
    try:
        fecha_normalizada = (
            fecha_texto.strip()
            .lower()
            .replace("a.m.", "AM")
            .replace("p.m.", "PM")
            .replace("a. m.", "AM")
            .replace("p. m.", "PM")
        )
        return datetime.strptime(fecha_normalizada, "%d/%m/%Y %I:%M:%S %p")
    except (ValueError, AttributeError):
        return None

