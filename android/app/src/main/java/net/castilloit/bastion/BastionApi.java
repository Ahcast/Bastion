package net.castilloit.bastion;

import android.content.Context;
import org.json.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** First-party native client. Same server session, CSRF, TOTP and domain policies as web. */
final class BastionApi {
    static final String BASE = "https://api.bastion.castilloit.net";
    interface Transport { Response send(String method, String path, byte[] body, Map<String,String> headers) throws Exception; }
    static class Response {
        final int status; final byte[] bytes; final String mime, filename; final Map<String,List<String>> headers;
        Response(int status, byte[] bytes, String mime, String filename, Map<String,List<String>> headers) {
            this.status=status; this.bytes=bytes; this.mime=mime; this.filename=filename; this.headers=headers;
        }
        Object json() throws JSONException { return bytes.length == 0 ? new JSONObject() : new JSONTokener(new String(bytes, StandardCharsets.UTF_8)).nextValue(); }
    }
    static final class Failure extends IOException {
        final int status; final String title;
        Failure(int status,String title,String message) { super(message); this.status=status; this.title=title; }
    }
    private final SessionVault vault;
    private final Map<String,JSONObject> cookies = new LinkedHashMap<>();
    private final Transport transport;
    private String csrf="";
    BastionApi(Context context) { this(context, new HttpsTransport()); }
    BastionApi(Context context, Transport transport) {
        vault=new SessionVault(context); this.transport=transport;
        try { JSONArray saved=new JSONArray(vault.read()); for(int i=0;i<saved.length();i++){JSONObject c=saved.getJSONObject(i); cookies.put(c.getString("name"),c);} } catch(JSONException ignored){vault.clear();}
    }
    synchronized void clear() { cookies.clear();csrf="";vault.clear(); }
    synchronized void refreshCsrf() throws Exception { request("GET","/api/v1/auth/csrf",null,null,null); }
    synchronized Response request(String method,String path,JSONObject body,String version,String key) throws Exception {
        return request(method,path,body,version,key,false);
    }
    private Response request(String method,String path,JSONObject body,String version,String key,boolean retried) throws Exception {
        if(!path.startsWith("/api/v1/") || path.contains("..") || path.contains("#") || path.contains("\\")) throw new IOException("Ruta inválida.");
        boolean writes=!method.equals("GET");
        if(writes&&csrf.isEmpty()) refreshCsrf();
        Map<String,String> headers=new LinkedHashMap<>(); headers.put("Accept","application/json, application/pdf, application/octet-stream");
        headers.put("Cache-Control","no-store");headers.put("User-Agent","Bastion-Android/0.2.0");
        StringBuilder cookieHeader=new StringBuilder(); long now=System.currentTimeMillis();
        for(JSONObject c:cookies.values()) if(c.optLong("expires")>now){if(cookieHeader.length()>0)cookieHeader.append("; ");cookieHeader.append(c.optString("name")).append('=').append(c.optString("value"));}
        if(cookieHeader.length()>0)headers.put("Cookie",cookieHeader.toString());
        if(writes){ headers.put("Origin","https://bastion.castilloit.net");headers.put("X-CSRF-Token",csrf); }
        if(version!=null&&!version.isEmpty())headers.put("If-Match","\""+version.replace("\"", "")+"\"");
        if(key!=null)headers.put("Idempotency-Key",key);
        byte[] bytes=body==null?null:body.toString().getBytes(StandardCharsets.UTF_8);
        if(bytes!=null)headers.put("Content-Type","application/json; charset=utf-8");
        Response response=transport.send(method,path,bytes,headers);
        for(Map.Entry<String,List<String>> entry:response.headers.entrySet())if(entry.getKey()!=null&&entry.getKey().equalsIgnoreCase("Set-Cookie")) for(String raw:entry.getValue()){
            for(HttpCookie c:HttpCookie.parse(raw)) {
                if(!c.getName().startsWith("__Host-bastion_")||!c.getSecure()||!"/".equals(c.getPath())||c.getDomain()!=null)continue;
                if(c.getMaxAge()==0){cookies.remove(c.getName());continue;}
                long seconds=c.getMaxAge()<0?86400:Math.min(c.getMaxAge(),60L*60*24*365);
                cookies.put(c.getName(),new JSONObject().put("name",c.getName()).put("value",c.getValue()).put("expires",now+seconds*1000));
            }
        }
        vault.write(new JSONArray(cookies.values()).toString());
        Object parsed=null;
        if(response.mime.contains("json") || response.bytes.length==0)parsed=response.json();
        JSONObject obj=parsed instanceof JSONObject?(JSONObject)parsed:new JSONObject();
        if(obj.has("csrf_token"))csrf=obj.getString("csrf_token");
        if(response.status==403&&obj.optString("title").equals("CSRF Validation Failed")&&!retried){refreshCsrf();return request(method,path,body,version,key,true);}
        if(response.status<200||response.status>=300){
            if(response.status==401)clear();
            throw new Failure(response.status,obj.optString("title"),obj.optString("detail", "No se pudo completar la operación ("+response.status+")."));
        }
        return response;
    }
    static class HttpsTransport implements Transport {
        public Response send(String method,String path,byte[] body,Map<String,String> headers) throws Exception {
            HttpURLConnection connection=(HttpURLConnection)new URL(BASE+path).openConnection();
            connection.setInstanceFollowRedirects(false);connection.setUseCaches(false);connection.setConnectTimeout(20000);connection.setReadTimeout(120000);
            connection.setRequestMethod(method);
            for(Map.Entry<String,String> h:headers.entrySet())connection.setRequestProperty(h.getKey(),h.getValue());
            try {
                if(body!=null){connection.setDoOutput(true);connection.setFixedLengthStreamingMode(body.length);try(OutputStream out=connection.getOutputStream()){out.write(body);}}
                int code=connection.getResponseCode();
                String mime=connection.getContentType(); if(mime==null)mime="application/octet-stream";
                String disposition=connection.getHeaderField("Content-Disposition"),filename="documento";
                if(disposition!=null){java.util.regex.Matcher m=java.util.regex.Pattern.compile("filename=\"([^\"]+)\"").matcher(disposition);if(m.find())filename=m.group(1);}
                InputStream stream=code>=400?connection.getErrorStream():connection.getInputStream();
                byte[] result=stream==null?new byte[0]:readBounded(stream,145*1024*1024);
                return new Response(code,result,mime,filename,connection.getHeaderFields());
            }finally{connection.disconnect();}
        }
    }
    static byte[] readBounded(InputStream input,int maximum) throws IOException {
        try(InputStream in=input;ByteArrayOutputStream out=new ByteArrayOutputStream()){
            byte[] buffer=new byte[16384];int n,total=0;while((n=in.read(buffer))!=-1){total+=n;if(total>maximum)throw new IOException("El archivo excede el tamaño permitido.");out.write(buffer,0,n);}return out.toByteArray();
        }
    }
}
