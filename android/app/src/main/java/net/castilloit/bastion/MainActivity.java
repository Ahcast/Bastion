package net.castilloit.bastion;

import android.app.*;
import android.content.*;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.text.InputType;
import android.view.*;
import android.widget.*;
import org.json.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.*;
import java.util.function.*;

/** Android Views only. No browser, WebView, JavaScript or remote UI. */
public final class MainActivity extends Activity {
    private static final int INK=0xff25292d,TEAL=0xff006d77,MUTED=0xff56616b;
    private final ExecutorService io=Executors.newSingleThreadExecutor();
    private BastionApi api; private Contracts contracts; private JSONObject access=new JSONObject();
    private LinearLayout root,body,bar; private TextView progress; private boolean busy;
    private final Deque<Screen> history=new ArrayDeque<>(); private String currentTitle="Bastion";
    private BiConsumer<String,byte[]> filePicked; private byte[] exportBytes; private String exportMime;
    // Package-private injection is accessible only to the instrumentation APK, never via Intents.
    static Function<Context,BastionApi> testFactory;
    private static final class Screen { final LinearLayout content; final String title; Screen(LinearLayout c,String t){content=c;title=t;} }
    interface Work<T>{T run() throws Exception;}
    @Override public void onCreate(Bundle state){
        super.onCreate(state);
        if(android.os.Build.VERSION.SDK_INT>=33)getOnBackInvokedDispatcher().registerOnBackInvokedCallback(android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT,this::back);
        getWindow().setFlags(WindowManager.LayoutParams.FLAG_SECURE,WindowManager.LayoutParams.FLAG_SECURE);
        root=column();root.setBackgroundColor(0xfff5f6f7);setContentView(root);
        root.setOnApplyWindowInsetsListener((view,insets)->{view.setPadding(insets.getSystemWindowInsetLeft(),insets.getSystemWindowInsetTop(),insets.getSystemWindowInsetRight(),insets.getSystemWindowInsetBottom());return insets.consumeSystemWindowInsets();});
        try{contracts=new Contracts(this);api=testFactory==null?new BastionApi(this):testFactory.apply(this);login();
            run(()->{api.refreshCsrf();return api.request("GET","/api/v1/auth/me",null,null,null).json();},v->authenticated(),false);
        }catch(Exception e){page("Bastion",false);text(body,"No se pudo preparar la aplicación.",18);button(body,"Reintentar",()->recreate());}
    }
    int dp(int n){return Math.round(n*getResources().getDisplayMetrics().density);}
    LinearLayout column(){LinearLayout l=new LinearLayout(this);l.setOrientation(LinearLayout.VERTICAL);l.setLayoutParams(new LinearLayout.LayoutParams(-1,-2));return l;}
    LinearLayout card(LinearLayout parent){LinearLayout l=column();l.setPadding(dp(14),dp(10),dp(14),dp(10));GradientDrawable bg=new GradientDrawable();bg.setColor(Color.WHITE);bg.setCornerRadius(dp(12));bg.setStroke(dp(1),0xffdce2e5);l.setBackground(bg);LinearLayout.LayoutParams p=new LinearLayout.LayoutParams(-1,-2);p.setMargins(0,dp(8),0,dp(8));parent.addView(l,p);return l;}
    TextView text(LinearLayout parent,String value,int size){TextView t=new TextView(this);t.setText(value);t.setTextColor(size>=18?INK:MUTED);t.setTextSize(size);t.setPadding(0,dp(6),0,dp(6));t.setTextIsSelectable(true);parent.addView(t);return t;}
    void heading(LinearLayout p,String v,int s){TextView t=text(p,v,s);t.setTypeface(null,android.graphics.Typeface.BOLD);}
    Button button(LinearLayout parent,String title,Runnable action){Button b=new Button(this);b.setText(title);b.setAllCaps(false);b.setTextSize(16);boolean primary=title.equals("Ingresar")||title.equals("Guardar")||title.equals("Verificar");b.setTextColor(primary?Color.WHITE:TEAL);b.setMinHeight(dp(48));b.setPadding(dp(12),dp(8),dp(12),dp(8));GradientDrawable shape=new GradientDrawable();shape.setColor(primary?TEAL:Color.WHITE);shape.setCornerRadius(dp(10));shape.setStroke(dp(1),primary?TEAL:0xffcbd5da);b.setBackground(new android.graphics.drawable.RippleDrawable(android.content.res.ColorStateList.valueOf(0x33006d77),shape,null));b.setOnClickListener(v->{if(!busy)action.run();});LinearLayout.LayoutParams params=new LinearLayout.LayoutParams(-1,-2);params.setMargins(0,dp(4),0,dp(4));parent.addView(b,params);return b;}
    EditText input(String hint,boolean password){EditText e=new EditText(this);e.setHint(hint);e.setTextSize(16);e.setMinHeight(dp(48));e.setSingleLine(true);e.setInputType(InputType.TYPE_CLASS_TEXT|(password?InputType.TYPE_TEXT_VARIATION_PASSWORD:InputType.TYPE_TEXT_FLAG_CAP_SENTENCES));e.setSaveEnabled(false);if(android.os.Build.VERSION.SDK_INT>=26)e.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_NO);return e;}
    private void page(String title,boolean push){if(push&&body!=null){history.push(new Screen(body,currentTitle));while(history.size()>16)history.removeLast();}body=column();body.setPadding(dp(16),dp(6),dp(16),dp(24));currentTitle=title;mount();}
    private void mount(){
        root.removeAllViews();bar=column();bar.setPadding(dp(16),0,dp(16),0);root.addView(bar);
        if(!history.isEmpty())button(bar,"‹ Volver",this::back);heading(bar,currentTitle,24);
        progress=text(bar,"",14);progress.setVisibility(View.GONE);
        ScrollView scroll=new ScrollView(this);scroll.setFillViewport(true);if(body.getParent() instanceof android.view.ViewGroup)((android.view.ViewGroup)body.getParent()).removeView(body);scroll.addView(body);root.addView(scroll,new LinearLayout.LayoutParams(-1,0,1));
    }
    private void back(){if(busy)return;if(!history.isEmpty()){Screen s=history.pop();body=s.content;currentTitle=s.title;mount();}else moveTaskToBack(true);}
    // API 33+ uses the platform dispatcher registered above. This override serves API 24–32.
    @android.annotation.SuppressLint("GestureBackNavigation")
    @Override public void onBackPressed(){back();}
    private <T> void run(Work<T> work,Consumer<T> done,boolean report){
        if(busy)return;busy=true;progress.setText("Conectando con Bastion…");progress.setVisibility(View.VISIBLE);
        io.submit(()->{try{T result=work.run();runOnUiThread(()->{if(isDestroyed())return;busy=false;progress.setVisibility(View.GONE);done.accept(result);});}
            catch(Exception e){runOnUiThread(()->{if(isDestroyed())return;busy=false;progress.setVisibility(View.GONE);if(report)failure(e);});}});
    }
    void error(String message){new AlertDialog.Builder(this).setTitle("Bastion").setMessage(message).setPositiveButton("Aceptar",null).show();}
    private void failure(Exception e){
        if(e instanceof BastionApi.Failure){BastionApi.Failure f=(BastionApi.Failure)e;if(f.status==401){access=new JSONObject();history.clear();login();error("La sesión terminó. Inicia sesión nuevamente.");return;}if(f.status==428){reauthenticate();return;}if(f.status==412){error("Este registro cambió en otro dispositivo. Tus campos siguen en pantalla. Vuelve al detalle y actualiza antes de guardar.");return;}}
        error(e instanceof IOException&&!(e instanceof BastionApi.Failure)?"No se pudo conectar. Revisa Internet y vuelve a intentarlo. No se ha confirmado la operación.":e.getMessage()==null?"No se pudo completar la operación.":e.getMessage());
    }
    private void login(){
        history.clear();page("Bastion",false);heading(body,"Iniciar sesión",26);text(body,"Tu trabajo, desde el celular.",16);
        LinearLayout form=card(body);text(form,"Correo",14);EditText email=input("Correo",false);email.setInputType(InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS);form.addView(email);
        text(form,"Contraseña",14);EditText password=input("Contraseña",true);form.addView(password);CheckBox keep=new CheckBox(this);keep.setText("Mantener sesión iniciada");keep.setMinHeight(dp(48));form.addView(keep);
        button(form,"Ingresar",()->{String mail=email.getText().toString().trim(),secret=password.getText().toString();if(mail.isEmpty()||secret.isEmpty()){error("Escribe tu correo y contraseña.");return;}JSONObject data=new JSONObject();Contracts.put(data,"email",mail);Contracts.put(data,"password",secret);Contracts.put(data,"keep_me_logged_in",keep.isChecked());
            run(()->api.request("POST","/api/v1/auth/login",data,null,null).json(),value->{password.setText("");if(Contracts.object(value).optString("status").equals("totp_required"))totp(false);else authenticated();},true);
        });
        button(body,"Olvidé mi contraseña",()->{page("Recuperar acceso",true);EditText mail=input("Correo",false);body.addView(mail);button(body,"Enviar enlace",()->{JSONObject b=new JSONObject();Contracts.put(b,"email",mail.getText().toString().trim());run(()->api.request("POST","/api/v1/auth/password-reset",b,null,null),r->error("Si la cuenta existe, recibirás las instrucciones en tu correo."),true);});});
        text(body,"Aplicación nativa · 0.2.0 Preview",12);
    }
    private void totp(boolean recovery){
        page(recovery?"Código de recuperación":"Verificación en dos pasos",true);text(body,recovery?"Escribe uno de tus códigos de recuperación.":"Escribe el código de tu aplicación de autenticación.",16);
        EditText code=input("Código",false);if(!recovery)code.setInputType(InputType.TYPE_CLASS_NUMBER);body.addView(code);
        button(body,"Verificar",()->{JSONObject b=new JSONObject();Contracts.put(b,"code",code.getText().toString().trim());run(()->api.request("POST","/api/v1/auth/"+(recovery?"recovery-code":"totp"),b,null,null),r->{code.setText("");authenticated();},true);});
        if(!recovery)button(body,"Usar código de recuperación",()->totp(true));
    }
    private void reauthenticate(){
        EditText code=input("Código TOTP",false);code.setInputType(InputType.TYPE_CLASS_NUMBER);
        AlertDialog dialog=new AlertDialog.Builder(this).setTitle("Confirma tu identidad").setMessage("Tus cambios siguen en pantalla. Después de verificar, vuelve a pulsar Guardar.").setView(code).setPositiveButton("Verificar",null).setNegativeButton("Cancelar",null).create();
        dialog.setOnShowListener(d->dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(v->{JSONObject b=new JSONObject();Contracts.put(b,"code",code.getText().toString().trim());run(()->api.request("POST","/api/v1/auth/reauth-totp",b,null,null),r->{code.setText("");dialog.dismiss();Toast.makeText(this,"Identidad confirmada",Toast.LENGTH_LONG).show();},true);}));dialog.show();
    }
    private void authenticated(){run(()->api.request("GET","/api/v1/auth/access",null,null,null).json(),value->{access=Contracts.object(value);home();},true);}
    private void home(){
        history.clear();page("Bastion",false);heading(body,"Mis módulos",22);text(body,"Consulta y trabaja con la información de tu empresa.",16);
        for(int i=0;i<contracts.modules.length();i++){JSONObject module=contracts.modules.optJSONObject(i);String id=module.optString("id");
            if(id.equals("core")||Contracts.allowed(access,id+"/"+module.optString("scope")+"/ver")){LinearLayout c=card(body);button(c,module.optString("title"),()->module(module));}
        }
        button(body,"Actualizar permisos",this::authenticated);
        button(body,"Cerrar sesión",()->new AlertDialog.Builder(this).setTitle("Cerrar sesión").setMessage("Se cerrará esta sesión en el servidor.").setPositiveButton("Cerrar sesión",(d,w)->run(()->api.request("POST","/api/v1/auth/logout",new JSONObject(),null,null),r->{api.clear();access=new JSONObject();login();},true)).setNegativeButton("Cancelar",null).show());
    }
    private void module(JSONObject module){
        page(module.optString("title"),true);String id=module.optString("id");
        for(int i=0;i<contracts.operations.length();i++){JSONObject o=contracts.operations.optJSONObject(i);String path=o.optString("path");
            if(!id.equals(o.optString("module"))||!contracts.allowed(access,o)||(path.contains("{")&&!path.contains("/contabilidad/origen/")))continue;
            if(path.endsWith("/catalogo")||path.endsWith("/calculate")||path.endsWith("/calculo"))continue;
            if(!o.optString("method").equals("GET")&&contracts.find("GET",path)!=null)continue;
            button(body,o.optString("title"),()->open(o,new JSONObject(),new JSONObject(),new JSONObject()));
        }
    }
    private JSONObject querySchema(JSONObject op){JSONObject props=new JSONObject();JSONArray required=new JSONArray();JSONArray parameters=op.optJSONArray("parameters");if(parameters!=null)for(int i=0;i<parameters.length();i++){JSONObject p=parameters.optJSONObject(i);if(p.optString("in").equals("query")){Contracts.put(props,p.optString("name"),p.optJSONObject("schema"));if(p.optBoolean("required"))required.put(p.optString("name"));}}JSONObject s=new JSONObject();Contracts.put(s,"type","object");Contracts.put(s,"properties",props);Contracts.put(s,"required",required);return s;}
    private void open(JSONObject op,JSONObject vars,JSONObject query,JSONObject seed){
        JSONObject pathProps=new JSONObject();JSONArray params=op.optJSONArray("parameters");if(params!=null)for(int i=0;i<params.length();i++){JSONObject p=params.optJSONObject(i);if(p.optString("in").equals("path")&&!vars.has(p.optString("name")))Contracts.put(pathProps,p.optString("name"),p.optJSONObject("schema"));}
        if(pathProps.length()>0){page(op.optString("title"),true);JSONObject s=new JSONObject();Contracts.put(s,"type","object");Contracts.put(s,"properties",pathProps);Contracts.put(s,"required",new JSONArray(Contracts.keys(pathProps)));NativeForm f=new NativeForm(this,s,new JSONObject());body.addView(f.view);button(body,"Continuar",()->{try{JSONObject v=Contracts.copy(vars);JSONObject entered=f.value();for(String k:Contracts.keys(entered))Contracts.put(v,k,entered.opt(k));open(op,v,query,seed);}catch(Exception e){failure(e);}});return;}
        if(op.optString("method").equals("GET")){
            JSONObject qs=querySchema(op);JSONArray required=qs.optJSONArray("required");boolean missing=false;if(required!=null)for(int i=0;i<required.length();i++)missing|=!query.has(required.optString(i));
            if(missing){filters(op,vars,query);return;}load(op,vars,query);return;
        }edit(op,vars,seed);
    }
    private String url(JSONObject op,JSONObject vars,JSONObject query){String p=Contracts.bind(op.optString("path"),vars);StringJoiner q=new StringJoiner("&");for(String key:Contracts.keys(query))if(!query.isNull(key)&&!query.optString(key).isEmpty())q.add(Uri.encode(key)+"="+Uri.encode(query.optString(key)));return p+(q.length()>0?"?"+q:"");}
    private void load(JSONObject op,JSONObject vars,JSONObject query){run(()->api.request("GET",url(op,vars,query),null,vars.optString("_version",null),null),response->{
        try{if(!response.mime.contains("json")){download(response.bytes,response.mime,response.filename);return;}Object value=response.json();if(value instanceof JSONObject&&((JSONObject)value).has("content_base64")){JSONObject file=(JSONObject)value;download(android.util.Base64.decode(file.getString("content_base64"),android.util.Base64.DEFAULT),file.optString("mime","application/octet-stream"),file.optString("filename","documento"));return;}
            display(op,vars,query,value);
        }catch(Exception e){failure(e);}
    },true);}
    private void filters(JSONObject op,JSONObject vars,JSONObject query){page("Filtros",true);NativeForm form=new NativeForm(this,querySchema(op),query);body.addView(form.view);button(body,"Aplicar filtros",()->{try{load(op,vars,form.value());}catch(Exception e){failure(e);}});}
    private void display(JSONObject op,JSONObject vars,JSONObject query,Object value){
        page(op.optString("title"),true);button(body,"Actualizar",()->load(op,vars,query));if(querySchema(op).optJSONObject("properties").length()>0)button(body,"Buscar y filtrar",()->filters(op,vars,query));
        JSONObject record=Contracts.object(value);for(String k:new String[]{"version","quote_version","order_version","expedient_version"})if(record.has(k))Contracts.put(vars,"_version",record.opt(k));JSONArray list=value instanceof JSONArray?(JSONArray)value:record.optJSONArray("items");
        if(list==null)list=record.optJSONArray("rows");
        actions(body,op.optString("path"),vars,record);
        if(record.has("proceso_id")&&Contracts.allowed(access,"cotizaciones/documentos/editar"))button(body,"Crear cotización del proceso",()->{try{JSONObject seed=new JSONObject().put("data",ProcessQuote.prepare(record));JSONObject context=new JSONObject().put("_seed",true);open(contracts.find("POST","/api/v1/cotizaciones"),context,new JSONObject(),seed);}catch(Exception e){failure(e);}});
        if(op.optString("path").equals("/api/v1/settings/backups"))backupControls(record,()->load(op,vars,query));
        resultActions(op,vars,record);
        if(list!=null){
            if(list.length()==0)text(body,"No hay registros para estos filtros.",16);
            for(int i=0;i<list.length();i++){Object row=list.opt(i);if(row instanceof JSONObject){JSONObject item=(JSONObject)row;LinearLayout card=card(body);heading(card,recordTitle(item),19);summary(card,item);button(card,"Abrir",()->record(op,vars,item));}else text(body,String.valueOf(row),16);}
            int page=Math.max(1,record.optInt("page",query.optInt("page",1)));text(body,"Página "+page,14);
            if(page>1)button(body,"Anterior",()->{JSONObject q=Contracts.copy(query);Contracts.put(q,"page",page-1);load(op,vars,q);});
            if(record.optBoolean("has_more",list.length()>=record.optInt("page_size",25)))button(body,"Siguiente",()->{JSONObject q=Contracts.copy(query);Contracts.put(q,"page",page+1);load(op,vars,q);});
            JSONObject rest=Contracts.copy(record);rest.remove("items");rest.remove("rows");rest.remove("page");rest.remove("page_size");rest.remove("has_more");if(rest.length()>0)render(body,rest,0);
        }else render(body,value,0);
    }
    private String recordTitle(JSONObject row){for(String k:new String[]{"cliente_name","supplier_name","client_name","name","title","description","expediente","resumen_corto","codigo_proceso","n_orders","invoice_number","full_name","email","month","code","filename","numero"})if(row.has(k)&&!row.isNull(k)&&!row.optString(k).isEmpty())return row.optString(k);return "Registro "+row.optString("id","");}
    private void summary(LinearLayout parent,JSONObject row){for(String k:new String[]{"quote_number","invoice_number","n_orders","status","state","quote_date","invoice_date","order_date","expense_date","date","total","balance","email","rtn","size_bytes"})if(row.has(k)&&!row.isNull(k))text(parent,Labels.of(k)+": "+row.optString(k),15);}
    private void record(JSONObject listing,JSONObject parentVars,JSONObject item){
        String prefix=listing.optString("path")+"/";JSONObject candidate=null;
        for(int i=0;i<contracts.operations.length();i++){JSONObject op=contracts.operations.optJSONObject(i);String p=op.optString("path");if(p.startsWith(prefix)&&p.substring(prefix.length()).matches("\\{[^}]+}")){candidate=op;if(op.optString("method").equals("GET"))break;}}
        if(candidate==null){page(recordTitle(item),true);render(body,item,0);return;}
        String template=candidate.optString("path"),name=template.substring(prefix.length()+1,template.length()-1);JSONObject vars=Contracts.copy(parentVars);
        Object identity=item.opt(name);if(identity==null)identity=item.opt("id");if(identity==null&&name.equals("numero"))identity=item.opt("lot_number");if(identity==null&&name.equals("job"))identity=item.opt("job_id");
        if(identity==null&&name.equals("item"))identity=item.opt("line_id");
        if(identity==null&&name.equals("id"))identity=item.opt("proceso_id");
        if(identity==null){page(recordTitle(item),true);render(body,item,0);return;}Contracts.put(vars,name,identity);
        JSONObject get=contracts.find("GET",template);if(get!=null){open(get,vars,new JSONObject(),item);return;}
        page(recordTitle(item),true);actions(body,template,vars,item);render(body,item,0);
    }
    private void actions(LinearLayout parent,String path,JSONObject vars,JSONObject seed){
        for(int i=0;i<contracts.operations.length();i++){JSONObject o=contracts.operations.optJSONObject(i);String p=o.optString("path");if(!contracts.allowed(access,o))continue;
            boolean same=p.equals(path)&&!o.optString("method").equals("GET");
            String suffix=p.startsWith(path+"/")?p.substring(path.length()+1):"";
            boolean child=!suffix.isEmpty()&&!suffix.startsWith("{");
        if(same||child)button(parent,o.optString("title"),()->open(o,Contracts.copy(vars),new JSONObject(),seed));
        }
    }
    private void edit(JSONObject op,JSONObject vars,JSONObject record){
        page(op.optString("title"),true);JSONObject schema=op.optJSONObject("body");if(schema!=null)schema=Contracts.copy(schema);JSONObject initial=record.optJSONObject("data");if(initial==null)initial=record.optJSONObject("credentials");if(initial==null)initial=record;
        if(op.optString("path").endsWith("/review")&&schema!=null){JSONObject p=schema.optJSONObject("properties");if(!Contracts.allowed(access,"procesos/marcas/editar"))p.remove("marked");if(!Contracts.allowed(access,"procesos/notas/editar"))p.remove("note");}
        if(op.optString("method").equals("POST")&&!op.optString("path").contains("{")&&!vars.optBoolean("_seed"))initial=new JSONObject();
        NativeForm form=schema==null?null:new NativeForm(this,schema,initial);if(form!=null)body.addView(form.view);
        final String version=record.optString("version",record.optString("quote_version",record.optString("order_version",vars.optString("_version",null))));
        // One key per identical submitted payload. Ambiguous network errors retain it for a safe retry.
        final String[] attempt={"",""};
        button(body,op.optString("title").equals("Calcular")?"Calcular":"Guardar",()->{
            try{JSONObject payload=form==null?new JSONObject():form.value();String serial=payload.toString();if(!serial.equals(attempt[0])){attempt[0]=serial;attempt[1]=UUID.randomUUID().toString();}
                String path=Contracts.bind(op.optString("path"),vars);String key=attempt[1].isEmpty()?UUID.randomUUID().toString():attempt[1];
                Runnable send=()->run(()->api.request(op.optString("method"),path,payload,version,key),r->{
                    try{if(!r.mime.contains("json")&&r.bytes.length>0){download(r.bytes,r.mime,r.filename);return;}Object result=r.json();JSONObject obj=Contracts.object(result);if(obj.has("content_base64")){download(android.util.Base64.decode(obj.getString("content_base64"),android.util.Base64.DEFAULT),obj.optString("mime","application/octet-stream"),obj.optString("filename","documento"));return;}page("Operación completada",true);text(body,"Bastion confirmó la operación.",18);resultActions(op,vars,obj);render(body,result,0);button(body,"Ir a mis módulos",this::home);if(path.equals("/api/v1/account/password")||path.endsWith("/totp/change/complete")){api.clear();access=new JSONObject();button(body,"Iniciar sesión",this::login);}}
                    catch(Exception e){failure(e);}
                },true);
                if(op.optString("method").equals("DELETE")||path.matches(".*(purge|mark-sent|convert.*|marcar-enviada|/emails|/restore|/reversar|/cancelar)$"))new AlertDialog.Builder(this).setTitle(op.optString("title")).setMessage("Confirma esta operación sobre el registro seleccionado.").setPositiveButton("Confirmar",(d,w)->send.run()).setNegativeButton("Cancelar",null).show();else send.run();
            }catch(Exception e){failure(e);}
        });
        JSONObject calculation=contracts.find("POST",op.optString("path").replace("/{id}","")+"/calculate");if(calculation==null)calculation=contracts.find("POST",op.optString("path").replace("/{id}","")+"/calculo");
        if(form!=null&&calculation!=null){JSONObject calc=calculation;button(body,"Calcular sin guardar",()->{try{JSONObject payload=form.value();run(()->api.request("POST",calc.optString("path"),payload,null,null).json(),r->{page("Cálculo",true);render(body,r,0);},true);}catch(Exception e){failure(e);}});}
    }
    private void render(LinearLayout parent,Object value,int depth){
        if(depth>10){text(parent,"Contenido anidado",14);return;}
        if(value instanceof JSONObject){JSONObject object=(JSONObject)value;for(String k:Contracts.keys(object)){
            if(k.equals("csrf_token")||k.equals("content_base64")||k.equals("token")||k.equals("setup_token"))continue;Object v=object.opt(k);if(v==null||v==JSONObject.NULL)continue;
            if(v instanceof JSONObject||v instanceof JSONArray){LinearLayout group=card(parent);heading(group,Labels.of(k),18);render(group,v,depth+1);}else text(parent,Labels.of(k)+": "+(v instanceof Boolean?((Boolean)v?"Sí":"No"):v.toString()),16);
        }}else if(value instanceof JSONArray){JSONArray array=(JSONArray)value;for(int i=0;i<array.length();i++)render(parent,array.opt(i),depth+1);}else if(value!=null&&value!=JSONObject.NULL)text(parent,value.toString(),16);
    }
    void selectCatalog(String module,Consumer<JSONObject> selected){
        LinearLayout content=column();content.setPadding(dp(16),dp(8),dp(16),dp(8));EditText search=input("Buscar por nombre o RTN",false);content.addView(search);LinearLayout results=column();ScrollView scroll=new ScrollView(this);scroll.addView(results);content.addView(scroll,new LinearLayout.LayoutParams(-1,dp(300)));
        AlertDialog dialog=new AlertDialog.Builder(this).setTitle(module.equals("clientes")?"Seleccionar cliente":"Seleccionar proveedor").setView(content).setNegativeButton("Cerrar",null).create();
        final int[] page={1};Runnable[] fetch={null};fetch[0]=()->run(()->api.request("GET","/api/v1/"+module+"/catalogo?page="+page[0]+"&page_size=25&search="+Uri.encode(search.getText().toString()),null,null,null).json(),r->{JSONObject data=Contracts.object(r);JSONArray rows=data.optJSONArray("items");results.removeAllViews();if(rows!=null)for(int i=0;i<rows.length();i++){JSONObject row=rows.optJSONObject(i);button(results,recordTitle(row),()->{selected.accept(row);dialog.dismiss();});}if(data.optBoolean("has_more"))button(results,"Siguiente",()->{page[0]++;fetch[0].run();});},true);
        button(content,"Buscar",()->{page[0]=1;fetch[0].run();});dialog.show();fetch[0].run();
    }
    private void resultActions(JSONObject op,JSONObject vars,JSONObject result){
        if(result.has("setup_token")){JSONObject confirm=contracts.find("POST","/api/v1/account/totp/change/complete");JSONObject context=new JSONObject();Contracts.put(context,"_seed",true);button(body,"Confirmar nuevo autenticador",()->open(confirm,context,new JSONObject(),result));}
        if(result.has("expediente_id")&&result.has("state")&&result.opt("id") instanceof String){JSONObject context=Contracts.copy(vars);Contracts.put(context,"id",result.opt("expediente_id"));Contracts.put(context,"job",result.opt("id"));JSONObject status=contracts.find("GET","/api/v1/licitaciones/{id}/pdf/trabajos/{job}");button(body,"Actualizar estado del PDF",()->open(status,context,new JSONObject(),result));if(result.optString("state").equals("ready")){JSONObject file=contracts.find("GET","/api/v1/licitaciones/{id}/pdf/trabajos/{job}/archivo");button(body,"Abrir PDF",()->open(file,context,new JSONObject(),result));}}
        if(!op.optString("method").equals("GET")){
            JSONObject entity=result.optJSONObject("client");if(entity==null)entity=result;
            String module=op.optString("path").split("/")[3];Object id=entity.opt("id");if(result.has("target_id")){id=result.opt("target_id");module=result.optString("target_module").replace("ordenes_compra","ordenes-compra");}
            JSONObject get=contracts.find("GET","/api/v1/"+module+"/{id}");if(id!=null&&get!=null){JSONObject context=new JSONObject();Contracts.put(context,"id",id);button(body,"Abrir registro guardado",()->open(get,context,new JSONObject(),result));}
        }
    }
    void pickFile(BiConsumer<String,byte[]> callback){filePicked=callback;Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT).setType("*/*").addCategory(Intent.CATEGORY_OPENABLE);startActivityForResult(intent,101);}
    private void backupControls(JSONObject status,Runnable reload){
        if(!access.optBoolean("is_system_administrator"))return;
        text(body,"Las copias y restauraciones pueden activar mantenimiento. El trabajo continúa en el servidor.",16);
        java.util.function.Consumer<JSONObject> command=payload->run(()->api.request("POST","/api/v1/settings/backups",payload,null,null).json(),r->reload.run(),true);
        if(status.optBoolean("available")&&!status.optBoolean("busy")){
            button(body,"Crear respaldo ahora",()->new AlertDialog.Builder(this).setTitle("Crear respaldo").setMessage("Se iniciará la copia de datos y documentos en el servidor.").setPositiveButton("Crear",(d,w)->{JSONObject b=new JSONObject();Contracts.put(b,"action","backup");command.accept(b);}).setNegativeButton("Cancelar",null).show());
            JSONArray history=status.optJSONArray("history");if(history!=null)for(int i=0;i<history.length();i++){JSONObject item=history.optJSONObject(i);LinearLayout c=card(body);text(c,item.optString("date")+" · "+item.optString("status"),16);
                button(c,"Preparar restauración",()->{JSONObject b=new JSONObject();Contracts.put(b,"action","prepare_s3");Contracts.put(b,"key",item.opt("key"));command.accept(b);});
                button(c,"Descargar respaldo",()->{JSONObject b=new JSONObject();Contracts.put(b,"action","download");Contracts.put(b,"key",item.opt("key"));run(()->api.request("POST","/api/v1/settings/backups",b,null,null).json(),r->{try{Uri u=Uri.parse(Contracts.object(r).getString("url"));String host=u.getHost();if(!"https".equals(u.getScheme())||host==null||!(host.equals("s3.amazonaws.com")||host.endsWith(".amazonaws.com"))||u.getUserInfo()!=null)throw new IllegalArgumentException("Enlace de respaldo inválido.");android.app.DownloadManager.Request request=new android.app.DownloadManager.Request(u).setTitle("Respaldo Bastion").setNotificationVisibility(android.app.DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED).setDestinationInExternalFilesDir(this,null,"bastion-respaldo-"+System.currentTimeMillis()+".zip");((android.app.DownloadManager)getSystemService(DOWNLOAD_SERVICE)).enqueue(request);Toast.makeText(this,"Descarga iniciada",Toast.LENGTH_LONG).show();}catch(Exception e){failure(e);}},true);});
            }
            button(body,"Preparar ZIP del teléfono",()->pickFile((filename,bytes)->{
                if(!filename.toLowerCase(Locale.ROOT).endsWith(".zip")){error("Selecciona un ZIP de Bastion.");return;}
                run(()->{String token="";try{JSONObject begin=new JSONObject().put("action","upload_begin").put("size",bytes.length);token=Contracts.object(api.request("POST","/api/v1/settings/backups",begin,null,null).json()).getString("token");for(int offset=0;offset<bytes.length;offset+=393216){byte[] chunk=Arrays.copyOfRange(bytes,offset,Math.min(offset+393216,bytes.length));JSONObject part=new JSONObject().put("action","upload_chunk").put("token",token).put("offset",offset).put("data",android.util.Base64.encodeToString(chunk,android.util.Base64.NO_WRAP));api.request("POST","/api/v1/settings/backups",part,null,null);}return api.request("POST","/api/v1/settings/backups",new JSONObject().put("action","upload_finish").put("token",token),null,null);}catch(Exception e){if(!token.isEmpty())try{api.request("POST","/api/v1/settings/backups",new JSONObject().put("action","upload_abort").put("token",token),null,null);}catch(Exception ignored){}throw e;}},r->reload.run(),true);
            }));
            JSONObject prepared=status.optJSONObject("prepared");if(prepared!=null){text(body,prepared.optString("summary"),16);text(body,"Confirmación válida hasta "+prepared.optString("expires_at"),14);
                button(body,"Confirmar restauración",()->new AlertDialog.Builder(this).setTitle("Reemplazar datos de Bastion").setMessage(prepared.optString("summary")+"\nSe activará mantenimiento y se creará un punto de recuperación previo. ¿Confirmas reemplazar los datos indicados?").setPositiveButton("Restaurar",(d,w)->{JSONObject b=new JSONObject();Contracts.put(b,"action","restore");Contracts.put(b,"token",prepared.opt("token"));Contracts.put(b,"confirmed",true);command.accept(b);}).setNegativeButton("Cancelar",null).show());
                button(body,"Cancelar preparación",()->{JSONObject b=new JSONObject();Contracts.put(b,"action","discard");Contracts.put(b,"token",prepared.opt("token"));command.accept(b);});
            }
        }
    }
    private void download(byte[] bytes,String mime,String filename){
        if(mime.startsWith("application/pdf")){page(filename,true);ImageView image=new ImageView(this);image.setAdjustViewBounds(true);image.setContentDescription("Página del documento PDF");body.addView(image,new LinearLayout.LayoutParams(-1,-2));TextView label=text(body,"",14);final int[] index={0};Runnable[] render={null};render[0]=()->run(()->{
            File file=File.createTempFile("pdf-",".pdf",getCacheDir());try{try(FileOutputStream out=new FileOutputStream(file)){out.write(bytes);}try(android.os.ParcelFileDescriptor fd=android.os.ParcelFileDescriptor.open(file,android.os.ParcelFileDescriptor.MODE_READ_ONLY);android.graphics.pdf.PdfRenderer pdf=new android.graphics.pdf.PdfRenderer(fd)){
                int count=pdf.getPageCount();index[0]=Math.max(0,Math.min(index[0],count-1));try(android.graphics.pdf.PdfRenderer.Page p=pdf.openPage(index[0])){int width=Math.min(1440,getResources().getDisplayMetrics().widthPixels*2);int height=(int)Math.min(4096,(long)width*p.getHeight()/Math.max(1,p.getWidth()));android.graphics.Bitmap bitmap=android.graphics.Bitmap.createBitmap(width,height,android.graphics.Bitmap.Config.ARGB_8888);bitmap.eraseColor(Color.WHITE);p.render(bitmap,null,null,android.graphics.pdf.PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY);return new Object[]{bitmap,count};}
            }}finally{if(!file.delete())file.deleteOnExit();}
        },r->{image.setImageBitmap((android.graphics.Bitmap)r[0]);label.setText("Página "+(index[0]+1)+" de "+r[1]);},true);
            button(body,"Página anterior",()->{index[0]--;render[0].run();});button(body,"Página siguiente",()->{index[0]++;render[0].run();});button(body,"Guardar PDF",()->export(bytes,mime,filename));render[0].run();
        }else export(bytes,mime,filename);
    }
    private void export(byte[] bytes,String mime,String filename){exportBytes=bytes;exportMime=mime;String safe=filename.replaceAll("[\\\\/:*?\"<>|\\p{Cntrl}]","_");Intent intent=new Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType(mime.split(";")[0]).putExtra(Intent.EXTRA_TITLE,safe);startActivityForResult(intent,102);}
    @Override protected void onActivityResult(int request,int result,Intent data){super.onActivityResult(request,result,data);if(result!=RESULT_OK||data==null||data.getData()==null){filePicked=null;exportBytes=null;return;}Uri uri=data.getData();
        if(request==101&&filePicked!=null){BiConsumer<String,byte[]> callback=filePicked;filePicked=null;run(()->{String name="archivo";try(android.database.Cursor cursor=getContentResolver().query(uri,new String[]{OpenableColumns.DISPLAY_NAME},null,null,null)){if(cursor!=null&&cursor.moveToFirst())name=cursor.getString(0);}byte[] bytes=BastionApi.readBounded(getContentResolver().openInputStream(uri),20*1024*1024);if(bytes.length==0)throw new IOException("Archivo vacío.");return new Object[]{name,bytes};},r->callback.accept((String)r[0],(byte[])r[1]),true);}
        if(request==102&&exportBytes!=null){byte[] bytes=exportBytes;exportBytes=null;run(()->{try(OutputStream out=getContentResolver().openOutputStream(uri)){if(out==null)throw new IOException();out.write(bytes);}return true;},r->Toast.makeText(this,"Archivo guardado",Toast.LENGTH_LONG).show(),true);}
    }
    @Override protected void onDestroy(){io.shutdownNow();exportBytes=null;filePicked=null;super.onDestroy();}
}
