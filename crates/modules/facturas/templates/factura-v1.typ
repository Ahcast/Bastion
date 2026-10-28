#let d = json("data.json")
#let money(value) = "L " + value
#let title = if d.kind == "delivery" { "Nota de entrega" } else if d.kind == "receipt" { "Recibo" } else if not d.issued { "Factura Proforma" } else { "Factura" }
#set document(title: title + " " + d.number, author: d.company.name)
#set page(paper: "us-letter", margin: 0.5in, numbering: "1 / 1")
#set text(font: ("Arial", "Liberation Sans", "Libertinus Serif"), size: 9pt, lang: "es")
#line(length: 100%, stroke: 2pt)
#grid(columns: (150pt, 1fr), gutter: 18pt,
 if d.logo {image("logo.png",width:140pt,height:65pt,fit:"contain")} else {text(size:18pt,weight:"bold",d.company.name)},
 [#strong(d.company.name) \ RTN: #d.company.rtn \ #d.company.address \ #d.company.phone · #d.company.email])
#v(12pt)
#if d.kind == "receipt" {
 align(center, text(size:18pt, weight:"bold", title))
 v(35pt)
 set par(justify:true)
 [Hemos recibido de #d.client_name la cantidad de #money(d.totals.total) por concepto de pago de la Factura #d.number.]
 v(18pt)
 [#strong[Total en letras:] #d.total_words]
 v(45pt)
 [Firma autorizada: #line(length:180pt,stroke:0.5pt)]
} else { [
#text(size:18pt,weight:"bold",title)
#v(8pt)
#grid(columns:(1fr,1fr),gutter:16pt,
 [#strong[Cliente:] #d.client_name \ #strong[RTN:] #d.client_rtn \ #strong[Referencia:] #d.order_number],
 [#strong[Número:] #d.number \ #strong[Fecha:] #d.date \ #strong[Estado:] #if not d.issued and d.kind == "invoice" { "Proforma" } else { d.status }])
#if d.kind == "invoice" {
 let term = if d.payment_term_type == "credito" { "Crédito (" + str(d.credit_days) + " días)" } else { "Contado" }
 let discount_type = if d.discount_type == "percent" { "Porcentaje" } else { "Valor (HNL)" }
 let discount = if d.discount_type == "percent" { d.discount_value + "%" } else { money(d.discount_value) }
 let display(value) = if value == "" { "—" } else if value.len() <= 35 { value } else { value.clusters().join("\u{200b}") }
 [#v(8pt) #grid(columns:(1fr,1fr),gutter:16pt,
  [#strong[Términos:] #term \ #strong[Descuento:] #discount_type \ #strong[Valor descuento:] #discount],
  [#strong[Orden de Compra / Exenta No.:] #display(d.purchase_order_exempt_number) \ #strong[Constancia de Registro No.:] #display(d.registration_certificate_number) \ #strong[Registro SAG No.:] #display(d.sag_registration_number)])]
}
#v(12pt)
#set table(inset:6pt,stroke:(bottom:0.4pt),align:(left,right))
#if d.kind == "delivery" {
 table(columns:(4fr,1fr),table.header(repeat:true,strong("Descripción"),strong("Cantidad")),..d.items.map(i=>(i.description,i.quantity)).flatten())
} else {
 table(columns:(3fr,1fr,1fr,1fr,1fr),table.header(repeat:true,..("Descripción","Cantidad","Precio unitario","Descuento","Importe").map(x=>strong(x))),..d.items.map(i=>(i.description,i.quantity,money(i.unit_price),money(i.discount),money(i.total))).flatten())
}
#v(12pt)
#if d.kind != "delivery" {
 block(width:100%,breakable:false)[
 #align(right)[#box(width:265pt)[#table(columns:(180pt,85pt),stroke:none,inset:2pt,align:left,
 [Subtotal],money(d.totals.subtotal),
 [Importe exonerado],money(d.totals.importe_exonerado),
 [Importe exento],money(d.totals.importe_exento),
 [Importe gravado 15%],money(d.totals.importe_gravado_15),
 [Importe gravado 18%],money(d.totals.importe_gravado_18),
 [Descuentos y rebajas],money(d.totals.descuentos_rebajas),
 [ISV 15%],money(d.totals.isv_15),[ISV 18%],money(d.totals.isv_18),
 strong("Total de factura"),strong(money(d.totals.total)),
 [Total pagado],money(d.paid_total),[Saldo],money(d.balance))]]
 ]
}
#if d.public_notes != "" { [#v(12pt) #strong[Notas:] \ #d.public_notes] }
#if d.sar != none and d.kind == "invoice" {
 [#v(16pt) #line(length:100%,stroke:0.5pt) \ #d.sar.sar_text \ #strong[Rango autorizado:] #d.sar.limit_range \ #strong[Fecha límite de emisión:] #d.sar.limit_date]
}
]
}
