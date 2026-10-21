(() => {
  "use strict";

  // Presentation metadata only. Business data and permissions belong to each module.
  const areas = [
    {
      id: "procesos", label: "Procesos activos", title: "Procesos", icon: "procesos",
      hint: "Oportunidades y seguimiento", description: "Oportunidades, entidades y fechas de recepción de ofertas.",
      chart: "Actividad de procesos", breakdown: "Procesos por estado", table: "Procesos recientes",
      columns: ["Expediente", "Entidad", "Resumen", "Recepción de ofertas", "De interés"],
    },
    {
      id: "cotizaciones", label: "Cotizaciones", title: "Cotizaciones", icon: "cotizaciones",
      hint: "Ofertas y propuestas", description: "Cotizaciones, clientes, importes y seguimiento de ofertas.",
      chart: "Monto cotizado en el tiempo", breakdown: "Cotizaciones por estado", table: "Cotizaciones recientes",
      columns: ["Cotización", "Cliente", "Fecha", "Estado", "Total"],
    },
    {
      id: "ordenes", label: "Órdenes de compra", title: "Órdenes de compra", icon: "ordenes",
      hint: "Órdenes y entregas", description: "Órdenes, entregas y seguimiento de compras.",
      chart: "Órdenes en el tiempo", breakdown: "Órdenes por estado", table: "Órdenes recientes",
      columns: ["Orden", "Cliente", "Fecha", "Entrega", "Estado", "Total"],
    },
    {
      id: "facturas", label: "Facturas", title: "Facturas", icon: "facturas",
      hint: "Facturación y cobros", description: "Facturas, pagos recibidos y saldos pendientes.",
      chart: "Facturación en el tiempo", breakdown: "Facturas por estado", table: "Facturas recientes",
      columns: ["Factura", "Cliente", "Fecha", "Estado", "Total", "Saldo"],
    },
    {
      id: "gastos", label: "Gastos", title: "Gastos", icon: "gastos",
      hint: "Registro y control", description: "Gastos registrados, proveedores e importes.",
      chart: "Gastos en el tiempo", breakdown: "Gastos por proveedor", table: "Gastos recientes",
      columns: ["Factura", "Fecha", "Proveedor", "Moneda", "Total"],
    },
  ];
  const paths = {
    procesos: '<rect x="4" y="5" width="16" height="16" rx="0"/><path d="M8 3v4m8-4v4M4 11h16m-11 4h2m2 0h2m-6 3h2"/>',
    cotizaciones: '<path d="M6 3h8l4 4v14H6zM14 3v5h4M9 12h6m-6 4h6"/>',
    ordenes: '<path d="m3 7 9-4 9 4v10l-9 4-9-4zM3 7l9 4 9-4m-9 4v10M7.5 5l9 4"/>',
    facturas: '<path d="M6 3h12v18l-3-2-3 2-3-2-3 2zM9 7h6m-6 4h6m-6 4h3"/>',
    gastos: '<path d="M3 7h18v13H3zM3 7V4h14v3m-1 5h5v4h-5z"/>',
    arrow: '<path d="M5 12h14m-5-5 5 5-5 5"/>',
    activity: '<path d="M3 12h4l3-7 4 14 3-7h4"/>',
    clock: '<circle cx="12" cy="12" r="8"/><path d="M12 7v5l3 2"/>',
    chart: '<path d="M4 4v16h16m-11-4v-5m5 5V8m5 8V5"/>',
  };
  let selected = "procesos";

  function escape(value) {
    return String(value).replace(/[&<>"']/g, character => ({
      "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
    }[character]));
  }

  function icon(name, className = "") {
    return `<svg class="db-icon ${className}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="square" stroke-linejoin="miter" aria-hidden="true" focusable="false">${paths[name]}</svg>`;
  }

  function sectionTitle(id, title, label) {
    return `<div class="db-section-heading"><h3 id="${id}">${escape(title)}</h3><span class="db-meta">${escape(label)}</span></div>`;
  }

  function detail(area) {
    return `
      <div class="db-detail-heading">
        <div><p class="db-overline">Resumen del área</p><h2>${escape(area.title)}</h2><p>${escape(area.description)}</p></div>
        <span class="db-badge">Próximamente</span>
      </div>
      <div class="db-availability">${icon("clock")}<p>Este módulo aún no está disponible. Aquí aparecerán sus estadísticas y registros cuando se incorpore.</p></div>
      <div class="db-insights">
        <section class="db-section" aria-labelledby="db-attention-title">
          ${sectionTitle("db-attention-title", "Requiere atención", area.title)}
          <div class="db-attention-empty">
            ${icon(area.icon, "db-empty-icon")}
            <strong>Tu seguimiento, en un solo lugar</strong>
            <p>Los pendientes de ${escape(area.title.toLowerCase())} aparecerán aquí.</p>
            <span class="db-empty-label">Sin actividad disponible</span>
          </div>
        </section>
        <section class="db-section" aria-labelledby="db-chart-title">
          ${sectionTitle("db-chart-title", area.chart, "Evolución")}
          <div class="db-chart-empty" role="img" aria-label="${escape(area.chart)}: sin datos disponibles">
            <div class="db-chart-grid" aria-hidden="true"><i></i><i></i><i></i><i></i></div>
            <div class="db-chart-message">${icon("chart")}<strong>Sin datos todavía</strong><span>La evolución se mostrará aquí.</span></div>
          </div>
          <p class="db-chart-caption">Las cifras estarán disponibles con el módulo.</p>
        </section>
        <section class="db-section" aria-labelledby="db-breakdown-title">
          ${sectionTitle("db-breakdown-title", area.breakdown, "Distribución")}
          <div class="db-distribution-empty"><div class="db-empty-ring" aria-hidden="true"><span>—</span></div><div><strong>Sin datos todavía</strong><p>La distribución de ${escape(area.title.toLowerCase())} aparecerá aquí.</p></div></div>
        </section>
      </div>
      <section class="db-section db-records" aria-labelledby="db-records-title">
        ${sectionTitle("db-records-title", area.table, "Registros")}
        <div class="db-table-wrap" tabindex="0" role="region" aria-label="${escape(area.table)}. Tabla con desplazamiento horizontal.">
          <table class="db-table"><caption class="sr-only">${escape(area.table)}. El módulo todavía no está disponible.</caption>
            <thead><tr>${area.columns.map(column => `<th scope="col">${escape(column)}</th>`).join("")}</tr></thead>
            <tbody><tr><td colspan="${area.columns.length}"><div class="db-records-empty">${icon(area.icon)}<strong>Aún no hay registros para mostrar</strong><p>Este espacio se actualizará con la actividad de ${escape(area.title.toLowerCase())}.</p></div></td></tr></tbody>
          </table>
        </div>
      </section>`;
  }

  function mount(container) {
    const date = new Intl.DateTimeFormat("es-HN", {
      weekday: "long", day: "numeric", month: "long", year: "numeric", timeZone: "America/Tegucigalpa",
    }).format(new Date());
    container.innerHTML = `
      <div class="dashboard">
        <header class="db-heading"><div><p class="db-overline">${escape(date)}</p><h1>Inicio</h1><p>Tu operación, en una vista. Selecciona un área para explorar su resumen.</p></div><span class="db-core-label">Bastion <span>/</span> Vista general</span></header>
        <div class="db-selector-heading"><h2>Resumen por área</h2><span class="db-meta">5 áreas · Próximamente</span></div>
        <div class="db-cards" role="tablist" aria-label="Área del panel de inicio">
          ${areas.map((area, index) => `<button type="button" class="db-card${area.id === selected ? " is-selected" : ""}" role="tab" id="db-tab-${area.id}" data-dashboard-area="${area.id}" aria-selected="${area.id === selected}" aria-controls="dashboard-content" tabindex="${area.id === selected ? "0" : "-1"}">
            <span class="db-card-top"><span class="db-card-number">0${index + 1}</span>${icon(area.icon)}</span>
            <span class="db-card-label">${escape(area.label)}</span><span class="db-card-value" aria-label="Sin datos">—</span><span class="db-card-hint">${escape(area.hint)}</span>
            <span class="db-card-bottom"><span class="db-card-status">Próximamente</span><span class="db-card-action">${icon("arrow")}</span></span>
          </button>`).join("")}
        </div>
        <p class="db-swipe-hint">Desliza para ver las cinco áreas ${icon("arrow")}</p>
        <p class="sr-only" role="status" aria-live="polite" id="dashboard-announcement"></p>
        <div id="dashboard-content" class="db-detail" role="tabpanel" aria-labelledby="db-tab-${selected}" tabindex="0">${detail(areas.find(area => area.id === selected))}</div>
      </div>`;

    const buttons = Array.from(container.querySelectorAll("[data-dashboard-area]"));
    const strip = container.querySelector(".db-cards");
    const panel = container.querySelector("#dashboard-content");
    const announcement = container.querySelector("#dashboard-announcement");
    function select(index, focus) {
      const area = areas[index];
      selected = area.id;
      buttons.forEach((button, position) => {
        button.classList.toggle("is-selected", position === index);
        button.setAttribute("aria-selected", String(position === index));
        button.tabIndex = position === index ? 0 : -1;
      });
      panel.setAttribute("aria-labelledby", `db-tab-${selected}`);
      panel.innerHTML = detail(area);
      announcement.textContent = `${area.title}: resumen seleccionado. Módulo disponible próximamente.`;
      if (focus) buttons[index].focus();
      if (focus || strip.scrollWidth > strip.clientWidth) buttons[index].scrollIntoView({block: "nearest", inline: "start"});
    }
    buttons.forEach((button, index) => {
      button.addEventListener("click", () => select(index, false));
      button.addEventListener("keydown", event => {
        const targets = {ArrowRight: (index + 1) % areas.length, ArrowLeft: (index + areas.length - 1) % areas.length, Home: 0, End: areas.length - 1};
        if (Object.hasOwn(targets, event.key)) {
          event.preventDefault();
          select(targets[event.key], true);
        }
      });
    });
  }

  window.BastionDashboard = Object.freeze({mount});
})();
