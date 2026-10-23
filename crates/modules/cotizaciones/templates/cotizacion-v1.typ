#let q = json("data.json")
#let ink = rgb("263849")
#let muted = rgb("687384")
#let money(value) = "L " + value
#set document(title: "Cotización " + q.number, author: q.company.name)
#set page(paper: "us-letter", margin: 0.45in, numbering: "1 / 1", number-align: right)
#set text(font: ("Arial", "Liberation Sans", "Libertinus Serif"), size: 9pt, fill: ink, lang: "es")
#set par(leading: 0.5em)
#line(length: 100%, stroke: 3pt + ink)
#v(10pt)
#grid(columns: (155pt, 1fr), gutter: 14pt,
  if q.logo { image("logo.png", width: 145pt, height: 65pt, fit: "contain") } else { text(size: 22pt, weight: "bold", q.company.name) },
  [#strong[RTN:] #q.company.rtn \
   #strong[Correo:] #q.company.email \
   #strong[Contacto:] #q.contact_email \
   #strong[Teléfono:] #q.company.phone \
   #strong[Dirección:] #q.company.address],
)
#v(10pt)
#line(length: 100%, stroke: 1.3pt + ink)
#v(12pt)
#grid(columns: (1fr, 220pt), gutter: 18pt,
  [#text(size: 13pt, weight: "bold", q.client_name)
   #if q.client_rtn != "" [\ #strong[RTN:] #q.client_rtn]
   #if q.order_number != "" [\ #strong[Referencia:] #q.order_number]],
  table(columns: (auto, 1fr), stroke: none, inset: (y: 3pt), align: (left, right),
    [Número], strong(q.number), [Fecha], q.date, [Válida hasta], q.valid_until, [Total], strong(money(q.totals.total))),
)
#v(12pt)
#set table(inset: 6pt, stroke: (left: none, right: none, top: none, bottom: 0.4pt + rgb("edf1f5")))
#table(columns: (2.8fr, 1fr, 0.65fr, 1fr, 0.8fr, 1fr), align: (left, right, right, right, right, right),
  table.header(repeat: true, ..("Concepto / descripción", "Coste unitario", "Cantidad", "Descuento", "Impuesto", "Total").map(label => table.cell(fill: rgb("eef3f7"), text(weight: "bold", label)))),
  ..q.items.map(item => (item.description, money(item.unit_price), item.quantity, money(item.discount_amount), item.tax_label, money(item.total))).flatten(),
)
#v(14pt)
#block(breakable: false)[
 #grid(columns: (1fr, 285pt), gutter: 18pt, align: top,
  if q.signature { image("signature.png", width: 175pt, height: 150pt, fit: "contain") },
  [
  #table(columns: (1fr, auto), stroke: none, inset: (y: 3pt), align: (left, right),
    [Importe Exonerado], money(q.totals.importe_exonerado),
    [Importe Exento], money(q.totals.importe_exento),
    [Importe Gravado 15%], money(q.totals.importe_gravado_15),
    [Importe Gravado 18%], money(q.totals.importe_gravado_18),
    [Descuentos y Rebajas], money(q.totals.descuentos_rebajas),
    [I.S.V. 15%], money(q.totals.isv_15),
    [I.S.V. 18%], money(q.totals.isv_18),
    table.cell(colspan: 2, line(length: 100%, stroke: 1.3pt + ink)),
    strong[ TOTAL A PAGAR ], strong(money(q.totals.total)),
  )
  ],
 )
]
#if q.amount_words != "" [#v(12pt) #strong[VALOR EN LETRAS: #q.amount_words]]
#if q.terms != "" [
  #v(16pt) #line(length: 100%, stroke: 0.5pt + rgb("d9dee5"))
  #v(5pt) #strong[Términos de cotización]
  #par(q.terms)
]
#v(24pt)
#block(breakable: false)[#line(length: 230pt, stroke: 0.5pt + muted) \
    #strong(q.prepared_by_name) \
    #text(fill: muted)[Acompañando en cada proyecto]]
