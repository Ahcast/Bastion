package net.castilloit.bastion;

import android.app.Instrumentation;
import android.content.*;
import android.view.*;
import android.widget.*;
import androidx.test.platform.app.InstrumentationRegistry;
import org.json.*;
import org.junit.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.atomic.AtomicReference;
import static org.junit.Assert.*;

/** Synthetic business fixtures only. Never logs into or changes the production service. */
public class NativeAppTest {
    private Instrumentation instrumentation; private Context context; private MainActivity activity;
    private FakeServer server;
    @Before public void setup(){instrumentation=InstrumentationRegistry.getInstrumentation();context=instrumentation.getTargetContext();new SessionVault(context).clear();server=new FakeServer();MainActivity.testFactory=c->new BastionApi(c,server);activity=(MainActivity)instrumentation.startActivitySync(new Intent(context,MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));idle();}
    @After public void cleanup(){instrumentation.runOnMainSync(()->activity.finish());MainActivity.testFactory=null;new SessionVault(context).clear();}
    private void idle(){for(int i=0;i<100;i++){instrumentation.waitForIdleSync();AtomicReference<Boolean> busy=new AtomicReference<>(true);instrumentation.runOnMainSync(()->{try{java.lang.reflect.Field f=MainActivity.class.getDeclaredField("busy");f.setAccessible(true);busy.set((Boolean)f.get(activity));}catch(Exception e){throw new AssertionError(e);}});if(!busy.get())return;android.os.SystemClock.sleep(50);}fail("Native request did not complete");}
    private View find(View root,String text){if(root instanceof TextView&&text.contentEquals(((TextView)root).getText()))return root;if(root instanceof ViewGroup){ViewGroup g=(ViewGroup)root;for(int i=0;i<g.getChildCount();i++){View child=find(g.getChildAt(i),text);if(child!=null)return child;}}return null;}
    private void click(String title){instrumentation.runOnMainSync(()->{View v=find(activity.getWindow().getDecorView(),title);assertNotNull(title,v);assertTrue(title,v.performClick());});idle();}
    private void fill(String hint,String value){instrumentation.runOnMainSync(()->{EditText e=findInput(activity.getWindow().getDecorView(),hint);assertNotNull(hint,e);e.setText(value);});}
    private EditText findInput(View root,String hint){if(root instanceof EditText&&hint.contentEquals(((EditText)root).getHint()))return (EditText)root;if(root instanceof ViewGroup){ViewGroup g=(ViewGroup)root;for(int i=0;i<g.getChildCount();i++){EditText e=findInput(g.getChildAt(i),hint);if(e!=null)return e;}}return null;}
    private void login(){fill("Correo","qa@example.invalid");fill("Contraseña","synthetic-only");click("Ingresar");fill("Código","123456");click("Verificar");}
    private void screenshot(String name){instrumentation.waitForIdleSync();android.os.SystemClock.sleep(250);instrumentation.runOnMainSync(()->{try{View v=activity.getWindow().getDecorView();android.graphics.Bitmap bitmap=android.graphics.Bitmap.createBitmap(v.getWidth(),v.getHeight(),android.graphics.Bitmap.Config.ARGB_8888);v.draw(new android.graphics.Canvas(bitmap));java.io.File dir=new java.io.File(context.getExternalFilesDir(null),"native-qa");assertTrue(dir.isDirectory()||dir.mkdirs());try(java.io.FileOutputStream out=new java.io.FileOutputStream(new java.io.File(dir,name+".png"))){bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG,100,out);}bitmap.recycle();}catch(Exception e){throw new AssertionError(e);}});}
    @Test public void nativeLoginTotpAllModulesAndClientWrite() throws Exception {
        screenshot("login");login();screenshot("modules");Contracts c=new Contracts(context);
        for(int i=0;i<c.modules.length();i++){String title=c.modules.getJSONObject(i).getString("title");instrumentation.runOnMainSync(()->assertNotNull(title,find(activity.getWindow().getDecorView(),title)));}
        assertFalse(hasWebView(activity.getWindow().getDecorView()));
        click("Clientes");click("Listado");click("Crear");fill("Nombre del cliente","Cliente de prueba");fill("Correo","cliente@example.invalid");click("Guardar");
        assertEquals("Cliente de prueba",server.created.getString("cliente_name"));assertEquals("cliente@example.invalid",server.created.getString("email"));assertTrue(server.lastHeaders.containsKey("Idempotency-Key"));assertEquals("https://bastion.castilloit.net",server.lastHeaders.get("Origin"));assertEquals("bound",server.lastHeaders.get("X-CSRF-Token"));
        screenshot("client-saved");
    }
    private boolean hasWebView(View v){if(v instanceof android.webkit.WebView)return true;if(v instanceof ViewGroup){ViewGroup g=(ViewGroup)v;for(int i=0;i<g.getChildCount();i++)if(hasWebView(g.getChildAt(i)))return true;}return false;}
    @Test public void allModuleLandingScreensUseNativeControls() throws Exception {
        login();Contracts c=new Contracts(context);for(int i=0;i<c.modules.length();i++){String title=c.modules.getJSONObject(i).getString("title");click(title);assertFalse(hasWebView(activity.getWindow().getDecorView()));click("‹ Volver");}
    }
    @Test public void quoteInvoiceOrderAndAccountingEditorsPreserveDomainFields() throws Exception {
        Contracts c=new Contracts(context);
        String quote="{\"client_name\":\"Cliente\",\"order_number\":\"OC-1\",\"quote_date\":\"2026-11-17\",\"valid_days\":30,\"status\":\"Borrador\",\"notes\":\"Nota\",\"terms\":\"Entrega\",\"discount_type\":\"amount\",\"discount_value\":\"0\",\"items\":[{\"description\":\"Equipo portátil\",\"quantity\":\"2\",\"unit_price\":\"12000.50\",\"discount_type\":\"percent\",\"discount_value\":\"5\",\"tax_type\":\"isv15\"}]}";
        JSONObject q=roundtrip(c,"/api/v1/cotizaciones",new JSONObject(quote));assertEquals("Equipo portátil",q.getJSONArray("items").getJSONObject(0).getString("description"));assertEquals("12000.50",q.getJSONArray("items").getJSONObject(0).getString("unit_price"));
        JSONObject invoice=new JSONObject(quote);invoice.put("invoice_date","2026-11-17");invoice.put("payment_term_type","credito");invoice.put("credit_days",30);invoice.put("public_notes","");invoice.put("expenses",new JSONArray());invoice.getJSONArray("items").getJSONObject(0).put("cost_price","10000");invoice.put("initial_payment",new JSONObject("{\"amount\":\"10.50\",\"date\":\"2026-11-17\",\"method\":\"Efectivo\",\"reference\":\"Caja\"}"));JSONObject inv=roundtrip(c,"/api/v1/facturas",invoice);assertEquals("10.50",inv.getJSONObject("initial_payment").getString("amount"));assertEquals("10000",inv.getJSONArray("items").getJSONObject(0).getString("cost_price"));
        JSONObject order=new JSONObject("{\"n_orders\":\"OC-2\",\"n_bill\":\"\",\"order_date\":\"2026-11-17\",\"received_date\":null,\"client_name\":\"Cliente\",\"client_rtn\":\"\",\"id_destination\":null,\"comments\":\"\",\"products\":[{\"description\":\"Equipo\",\"line_id\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"cantidad\":1,\"sale_price\":\"10.00\",\"purchase_price\":\"15.00\",\"tracking\":\"\",\"isv\":true,\"status\":\"Pendiente\",\"coments\":\"\"}],\"expenses\":[]}");JSONObject ord=roundtrip(c,"/api/v1/ordenes-compra",order);assertEquals("10.00",ord.getJSONArray("products").getJSONObject(0).getString("sale_price"));assertEquals(48,ord.getJSONArray("products").getJSONObject(0).getString("line_id").length());
        JSONObject entry=new JSONObject("{\"date\":\"2026-11-17\",\"description\":\"Asiento\",\"reference\":\"QA\",\"kind\":\"manual\",\"source\":null,\"lines\":[{\"account\":\"1101\",\"debit\":\"10.01\",\"credit\":\"0\"},{\"account\":\"2101\",\"debit\":\"0\",\"credit\":\"10.01\"}]}");assertEquals("10.01",roundtrip(c,"/api/v1/contabilidad/asientos",entry).getJSONArray("lines").getJSONObject(0).getString("debit"));
    }
    private JSONObject roundtrip(Contracts c,String path,JSONObject input){AtomicReference<JSONObject> result=new AtomicReference<>();instrumentation.runOnMainSync(()->{try{NativeForm form=new NativeForm(activity,c.find("POST",path).getJSONObject("body"),input);result.set(form.value());}catch(Exception e){throw new AssertionError(path,e);}});return result.get();}
    @Test public void transportRetriesCsrfOncePreservesIdempotencyAndClearsExpiredSession() throws Exception {
        final int[] post={0};List<String> keys=new ArrayList<>();BastionApi client=new BastionApi(context,(method,path,body,headers)->{
            if(path.endsWith("/csrf"))return response(200,"{\"csrf_token\":\"token\"}","__Host-bastion_csrf=token; Path=/; Secure; Max-Age=300");
            keys.add(headers.get("Idempotency-Key"));assertTrue(headers.get("Cookie").contains("__Host-bastion_csrf=token"));post[0]++;
            if(post[0]==1)return response(403,"{\"title\":\"CSRF Validation Failed\"}",null);
            if(post[0]==2)return response(201,"{}","__Host-bastion_session=private-test-value; Path=/; Secure; HttpOnly; Max-Age=3600");
            return response(401,"{\"detail\":\"Expired\"}",null);
        });
        client.request("POST","/api/v1/clientes",new JSONObject(),null,"same-key-123456789");assertEquals(Arrays.asList("same-key-123456789","same-key-123456789"),keys);
        String persisted=context.getSharedPreferences("session",Context.MODE_PRIVATE).getString("sealed","");assertFalse(persisted.contains("private-test-value"));assertTrue(new SessionVault(context).read().contains("private-test-value"));
        try{client.request("POST","/api/v1/clientes",new JSONObject(),"8","next-key-123456789");fail();}catch(BastionApi.Failure e){assertEquals(401,e.status);}assertEquals("[]",new SessionVault(context).read());
    }
    @Test public void permissionsRequireEnabledModulesAndSourceReportAccess() throws Exception {
        JSONObject a=new JSONObject("{\"is_system_administrator\":false,\"modules\":[{\"module_id\":\"clientes\"},{\"module_id\":\"reportes\"}],\"permissions\":[{\"module_id\":\"clientes\",\"scope_key\":\"catalogo\",\"capability\":\"ver\"},{\"module_id\":\"reportes\",\"scope_key\":\"facturas\",\"capability\":\"ver\"}]}");
        assertTrue(Contracts.allowed(a,"clientes/catalogo/ver"));assertFalse(Contracts.allowed(a,"clientes/catalogo/editar"));assertFalse(Contracts.allowed(a,"facturas/documentos/ver"));assertFalse(new Contracts(context).allowed(a,new Contracts(context).find("GET","/api/v1/reportes/facturas")));
    }
    @Test public void processQuotationDoesNotTurnSpecificationsIntoQuantities() throws Exception {
        assertEquals("",ProcessQuote.item("220 V estabilizador").getString("quantity"));
        assertEquals("220 V estabilizador",ProcessQuote.item("220 V estabilizador").getString("description"));
        assertEquals("2",ProcessQuote.item("2 x Computadora portátil").getString("quantity"));
        assertEquals("1500",ProcessQuote.item("Cantidad: 1,500 unidades de cable").getString("quantity"));
        assertEquals("",ProcessQuote.item("16 puertos switch Ethernet").getString("quantity"));
    }
    @Test public void backupRestoreRequiresAnExplicitUserAction(){
        login();click("Ajustes");click("Respaldos");
        instrumentation.runOnMainSync(()->assertNotNull(find(activity.getWindow().getDecorView(),"Confirmar restauración")));
        assertEquals(0,server.backupWrites);
    }
    @Test public void anonymousHandshakeReachesRealApiOverHttps() throws Exception {
        // One public, read-only request; no credentials, login, or production business writes.
        BastionApi client=new BastionApi(context,new BastionApi.HttpsTransport());
        BastionApi.Response response=client.request("GET","/api/v1/auth/csrf",null,null,null);
        assertEquals(200,response.status);assertTrue(Contracts.object(response.json()).getString("csrf_token").length()>20);
        assertTrue(new SessionVault(context).read().contains("__Host-bastion_csrf"));client.clear();
    }
    @Test public void nativePdfRendersWithoutAnExternalActivity() throws Exception {
        login();android.graphics.pdf.PdfDocument document=new android.graphics.pdf.PdfDocument();android.graphics.pdf.PdfDocument.Page page=document.startPage(new android.graphics.pdf.PdfDocument.PageInfo.Builder(300,400,1).create());page.getCanvas().drawText("Bastion QA",20,30,new android.graphics.Paint());document.finishPage(page);java.io.ByteArrayOutputStream bytes=new java.io.ByteArrayOutputStream();document.writeTo(bytes);document.close();
        instrumentation.runOnMainSync(()->{try{java.lang.reflect.Method method=MainActivity.class.getDeclaredMethod("download",byte[].class,String.class,String.class);method.setAccessible(true);method.invoke(activity,bytes.toByteArray(),"application/pdf","prueba.pdf");}catch(Exception e){throw new AssertionError(e);}});idle();
        instrumentation.runOnMainSync(()->assertNotNull(find(activity.getWindow().getDecorView(),"Página 1 de 1")));screenshot("native-pdf");
    }
    static BastionApi.Response response(int code,String json,String cookie){Map<String,List<String>> headers=new HashMap<>();if(cookie!=null)headers.put("Set-Cookie",Arrays.asList(cookie));return new BastionApi.Response(code,json.getBytes(StandardCharsets.UTF_8),"application/json","",headers);}
    class FakeServer implements BastionApi.Transport {
        boolean logged;JSONObject created;Map<String,String> lastHeaders;int backupWrites;
        public BastionApi.Response send(String method,String path,byte[] body,Map<String,String> headers) throws Exception {
            if(path.endsWith("/csrf"))return response(200,"{\"csrf_token\":\"initial\"}","__Host-bastion_csrf=initial; Path=/; Secure; Max-Age=300");
            if(path.endsWith("/me"))return response(401,"{}",null);
            if(path.endsWith("/login"))return response(200,"{\"status\":\"totp_required\"}","__Host-bastion_challenge=challenge; Path=/; Secure; Max-Age=300");
            if(path.endsWith("/totp")){assertTrue(headers.get("Cookie").contains("challenge"));logged=true;return response(200,"{\"status\":\"authenticated\",\"csrf_token\":\"bound\"}","__Host-bastion_session=fixture; Path=/; Secure; Max-Age=3600");}
            assertTrue(logged);
            if(path.equals("/api/v1/settings/backups")){if(!method.equals("GET"))backupWrites++;return response(200,"{\"available\":true,\"busy\":false,\"history\":[],\"prepared\":{\"summary\":\"Datos ficticios\",\"token\":\"fixture-token\",\"expires_at\":\"2026-11-17\"}}",null);}
            if(path.endsWith("/access")){JSONObject access=new JSONObject();access.put("is_system_administrator",true);JSONArray modules=new JSONArray();Contracts c=new Contracts(context);for(int i=0;i<c.modules.length();i++)modules.put(new JSONObject().put("module_id",c.modules.getJSONObject(i).getString("id")));access.put("modules",modules);access.put("permissions",new JSONArray());return response(200,access.toString(),null);}
            if(path.equals("/api/v1/clientes")&&method.equals("POST")){created=new JSONObject(new String(body,StandardCharsets.UTF_8));lastHeaders=new HashMap<>(headers);return response(201,new JSONObject().put("client",created).toString(),null);}
            return response(200,"{\"items\":[],\"page\":1,\"has_more\":false}",null);
        }
    }
}

