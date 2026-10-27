import { useEffect, useRef, useState } from 'react';
import { ApiError, message } from '../api.js';
import { Heading, Loading, Notice, useCore } from '../ui.js';
import { moduleCan } from '../types.js';
import { useNavigationGuard, useUnsavedChanges } from '../navigationGuard.js';
import { downloadFile, encodeAttachment, fileSize } from '../quoteAttachments.js';
import type { QuoteFile } from '../quoteAttachments.js';
import type { PurchaseOrder } from '../orders.js';

interface Document { id:string;line_id:string;filename:string;mime:string;byte_size:number }
interface Documents {items:Document[];order_version:number}
interface Pending {line:string;file:File;key:string;version:number}
const base='/api/v1/ordenes-compra';
export function OrderDocuments({ id, close }:{id:number;close:()=>void}){
  const {api,access,notify}=useCore(),{navigate}=useNavigationGuard();
  const [order,setOrder]=useState<PurchaseOrder|null>(null),[files,setFiles]=useState<Documents|null>(null);
  const [storage,setStorage]=useState(false),[pending,setPending]=useState<Pending|null>(null);
  const [busy,setBusy]=useState(false),[error,setError]=useState(''),[conflict,setConflict]=useState(false);
  const [ambiguous,setAmbiguous]=useState(false);
  const running=useRef(false), heading=useRef<HTMLHeadingElement>(null);
  const editable=moduleCan(access,'ordenes_compra','documentos','editar'), removable=moduleCan(access,'ordenes_compra','documentos','eliminar');
  useUnsavedChanges(!!pending||ambiguous,busy);
  const reload=async()=>{
    const [current,documents,ready]=await Promise.all([
      api.request<PurchaseOrder>(base+'/'+id),api.request<Documents>(base+'/'+id+'/documents'),api.request<{storage:boolean}>(base+'/document-readiness')
    ]);
    if(current.version!==documents.order_version)throw new Error('La orden cambió durante la consulta. Reintenta cargar documentos.');
    setOrder(current);setFiles(documents);setStorage(ready.storage);setConflict(false);
  };
  useEffect(()=>{heading.current?.focus();void reload().catch(reason=>setError(message(reason)));},[id,api]);
  const run=async(action:()=>Promise<void>)=>{
    if(running.current)return;running.current=true;setBusy(true);setError('');
    try{await action();}catch(reason){setError(message(reason));if(reason instanceof ApiError&&reason.status===412)setConflict(true);}
    finally{running.current=false;setBusy(false);}
  };
  return <><Heading title="Documentos de la orden" description={order?.data.n_orders??'Archivos privados por producto'} action={<button disabled={busy} onClick={()=>void navigate(close)}>Volver</button>}/>
    <h2 tabIndex={-1} ref={heading}>Archivos por producto</h2><p>Un archivo por producto, hasta 10 MiB. Clonar una orden no copia estos archivos.</p><Notice>{error}</Notice>
    <button disabled={busy} onClick={()=>void navigate(()=>{setPending(null);setAmbiguous(false);void run(reload);})}>Cargar versión actual</button>
    {order&&files?<>{!storage&&<Notice>El almacenamiento privado no está disponible. Solicita al administrador revisar su configuración.</Notice>}
      {conflict&&<Notice>La orden cambió. Tu archivo seleccionado se conserva; carga la versión actual antes de continuar.</Notice>}
      {pending&&<section className="card" aria-label="Archivo seleccionado"><p>{pending.file.name} · {fileSize(pending.file.size)}</p>
        <button className="primary" disabled={busy||conflict||!storage} onClick={()=>void run(async()=>{
          const content_base64=await encodeAttachment(pending.file);
          let result:{attachment:Document;order_version:number};
          try{
            result=await api.request(base+'/'+id+'/documents/'+pending.line,{method:'POST',ifMatch:pending.version,idempotencyKey:pending.key,body:{filename:pending.file.name,content_base64}});
          }catch(reason){if(!(reason instanceof ApiError)||reason.status>=500||reason.status===408)setAmbiguous(true);throw reason;}
          setFiles(previous=>previous?{items:[...previous.items.filter(f=>f.line_id!==pending.line),result.attachment],order_version:result.order_version}:previous);
          setPending(null);setAmbiguous(false);notify('Archivo guardado.');await reload();
        })}>{ambiguous?'Reintentar guardar el mismo archivo':'Guardar archivo'}</button>
        <button disabled={busy} onClick={()=>void navigate(()=>{setPending(null);setAmbiguous(false);})}>Descartar selección</button>
      </section>}
      <div className="order-documents">{order.data.products.map((product,index)=>{
        const file=files.items.find(f=>f.line_id===product.line_id);
        return <section className="card" key={product.line_id??index}><h3>Producto {index+1} · {product.description||'Sin descripción'}</h3>
          {file?<><p>{file.filename} · {fileSize(file.byte_size)}</p><button disabled={busy} onClick={()=>void run(async()=>downloadFile(await api.request<QuoteFile>(base+'/'+id+'/documents/'+file.id+'/content')))}>Descargar archivo de producto {index+1}</button>
            {removable&&<button disabled={busy||!!pending||conflict} onClick={()=>{if(window.confirm('¿Retirar el archivo de este producto?'))void run(async()=>{
              await api.request(base+'/'+id+'/documents/'+file.id,{method:'DELETE',ifMatch:files.order_version});notify('Archivo retirado.');await reload();
            });}}>Retirar archivo de producto {index+1}</button>}
          </>:<p>Sin archivo</p>}
          {editable&&(!file||removable)&&<label>{file?'Sustituir archivo':'Seleccionar archivo'} del producto {index+1}<input type="file" disabled={busy||!!pending||conflict||!storage} onChange={event=>{
            const selected=event.target.files?.[0];event.target.value='';if(!selected)return;
            if(!selected.size||selected.size>10*1024*1024){setError('Cada archivo debe tener contenido y medir hasta 10 MiB.');return;}
            setError('');setPending({line:product.line_id!,file:selected,key:crypto.randomUUID(),version:files.order_version});
          }}/></label>}
          {file&&editable&&!removable&&<p>Necesitas permiso de eliminación para sustituir el archivo existente.</p>}
        </section>;
      })}</div>{!order.data.products.length&&<p>Esta orden no tiene productos. Agrega y guarda sus productos para asociar archivos.</p>}
    </>:!error&&<Loading/>}
  </>;
}
