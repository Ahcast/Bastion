import { useEffect,useId,useLayoutEffect,useRef,useState } from 'react';
import { createPortal } from 'react-dom';
import { useCore,useLoad } from '../ui.js';
type Supplier={id:number;supplier_name:string};
type Page={items:Supplier[];has_more:boolean};
export function OrderSupplierPicker({index,selected,canSearch,readOnly,change}:{index:number;selected:Supplier|null;canSearch:boolean;readOnly:boolean;change:(value:Supplier|null)=>void}){
 const {api}=useCore(),id=useId(),root=useRef<HTMLDivElement>(null),popup=useRef<HTMLDivElement>(null);
 const [open,setOpen]=useState(false),[text,setText]=useState(''),[query,setQuery]=useState(''),[page,setPage]=useState(1),[active,setActive]=useState(-1);
 const [position,setPosition]=useState({left:0,top:0,width:260});
 const pending=text.trim().slice(0,200)!==query;
 useEffect(()=>{if(!open)return;const timer=setTimeout(()=>{setQuery(text.trim().slice(0,200));setPage(1);setActive(-1);},200);return()=>clearTimeout(timer);},[text,open]);
 const result=useLoad(signal=>open&&canSearch&&!readOnly&&!pending?api.request<Page>('/api/v1/proveedores/catalogo?search='+encodeURIComponent(query)+'&page='+page+'&page_size=25',{signal}):Promise.resolve(null),[open,canSearch,readOnly,pending,query,page]);
 const choices=pending?[]:result.data?.items??[];
 const choose=(value:Supplier|null)=>{change(value);setOpen(false);setText('');setActive(-1);};
 useLayoutEffect(()=>{
  if(!open)return;
  const place=()=>{const r=root.current!.getBoundingClientRect();const width=Math.min(320,window.innerWidth-16);setPosition({left:Math.max(8,Math.min(r.left,window.innerWidth-width-8)),top:Math.max(8,Math.min(r.bottom+4,window.innerHeight-310)),width});};
  place();const outside=(e:PointerEvent)=>{if(!root.current?.contains(e.target as Node)&&!popup.current?.contains(e.target as Node))setOpen(false);};
  const scroll=()=>setOpen(false);
  document.addEventListener('pointerdown',outside);window.addEventListener('resize',place);window.addEventListener('scroll',scroll);
  return()=>{document.removeEventListener('pointerdown',outside);window.removeEventListener('resize',place);window.removeEventListener('scroll',scroll);};
 },[open]);
 const label='Proveedor del producto '+(index+1);
 return <div className="order-supplier" ref={root} onBlur={e=>{if(!root.current?.contains(e.relatedTarget as Node)&&!popup.current?.contains(e.relatedTarget as Node))setOpen(false);}}>
  <label><span className="quote-sr-only">{label}</span><input role="combobox" aria-label={label} autoComplete="off" aria-expanded={open} aria-controls={open?id:undefined} aria-autocomplete="list" aria-activedescendant={open&&active>=0?id+'-'+active:undefined}
   value={open?text:selected?.supplier_name??''} readOnly={readOnly||!canSearch} title={selected?.supplier_name||(!canSearch?'Sin acceso al catálogo de Proveedores':'Seleccionar proveedor')} placeholder={canSearch?'Seleccionar proveedor…':'Sin proveedor'}
   onFocus={()=>{if(!readOnly&&canSearch){setText('');setQuery('');setPage(1);setActive(-1);setOpen(true);}}}
   onChange={e=>{setText(e.target.value);setActive(-1);}}
   onKeyDown={e=>{
    if(e.key==='Escape'||e.key==='Tab'){setOpen(false);setText('');}
    if(e.key==='Enter'){e.preventDefault();if(open&&active>=0)choose(active===0?null:choices[active-1]??null);}
    if(canSearch&&!readOnly&&['ArrowDown','ArrowUp'].includes(e.key)){e.preventDefault();setOpen(true);setActive(v=>e.key==='ArrowDown'?(v+1)%(choices.length+1):v<=0?choices.length:v-1);}
   }}/></label>
  {!readOnly&&selected&&<button type="button" className="order-supplier-clear" aria-label={'Quitar proveedor del producto '+(index+1)} onClick={()=>choose(null)}>×</button>}
  {open&&canSearch&&!readOnly&&createPortal(<div ref={popup} className="order-supplier-popup" style={{position:'fixed',...position}} onMouseDown={e=>e.preventDefault()}>
   <div id={id} role="listbox" aria-label={'Proveedores del producto '+(index+1)}>
    <button type="button" role="option" id={id+'-0'} aria-selected={active===0} onClick={()=>choose(null)}>Sin proveedor</button>
    {choices.map((s,i)=><button key={s.id} type="button" role="option" id={id+'-'+(i+1)} aria-selected={active===i+1||(active<0&&selected?.id===s.id)} onClick={()=>choose(s)}>{s.supplier_name}</button>)}
   </div>
   {pending||(!result.data&&!result.error)?<p role="status">Buscando proveedores…</p>:result.error?<><p role="alert">{result.error}</p><button type="button" onClick={result.reload}>Reintentar proveedores</button></>:!choices.length&&<p role="status">Sin coincidencias</p>}
   {result.data&&!pending&&(page>1||result.data.has_more)&&<div className="actions"><button type="button" disabled={page===1} onClick={()=>{setPage(p=>p-1);setActive(-1);}}>Anteriores</button><span>Pág. {page}</span><button type="button" disabled={!result.data.has_more} onClick={()=>{setPage(p=>p+1);setActive(-1);}}>Más proveedores</button></div>}
  </div>,document.body)}
 </div>;
}
