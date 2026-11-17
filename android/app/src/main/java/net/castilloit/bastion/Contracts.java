package net.castilloit.bastion;

import android.content.Context;
import org.json.*;
import java.util.*;
import java.nio.charset.StandardCharsets;

final class Contracts {
    final JSONArray modules, operations;
    Contracts(Context context) throws Exception {
        JSONObject root=new JSONObject(new String(BastionApi.readBounded(context.getAssets().open("contracts.json"),8*1024*1024),StandardCharsets.UTF_8));
        modules=root.getJSONArray("modules");operations=root.getJSONArray("operations");
    }
    static List<String> keys(JSONObject object){List<String> result=new ArrayList<>();if(object!=null)object.keys().forEachRemaining(result::add);return result;}
    static JSONObject copy(JSONObject object){try{return new JSONObject(object.toString());}catch(JSONException e){throw new IllegalArgumentException(e);}}
    static JSONObject object(Object value){return value instanceof JSONObject?(JSONObject)value:new JSONObject();}
    static void put(JSONObject object,String key,Object value){try{object.put(key,value);}catch(JSONException e){throw new IllegalArgumentException(e);}}
    static String type(JSONObject schema){Object t=schema.opt("type");if(t instanceof JSONArray){JSONArray a=(JSONArray)t;for(int i=0;i<a.length();i++)if(!a.optString(i).equals("null"))return a.optString(i);}return t instanceof String?(String)t:schema.has("properties")?"object":"string";}
    static boolean nullable(JSONObject schema){return schema.optBoolean("nullable")||schema.optJSONArray("type")!=null&&schema.optJSONArray("type").toString().contains("\"null\"");}
    static boolean has(JSONArray array,String value){if(array==null)return false;for(int i=0;i<array.length();i++)if(value.equals(array.optString(i)))return true;return false;}
    static boolean allowed(JSONObject access,String requirement){
        if(requirement.equals("account"))return true;
        if(requirement.equals("admin"))return access.optBoolean("is_system_administrator");
        for(String alternative:requirement.split("\\|")){
            String[] parts=alternative.split("/");if(parts.length!=3)continue;
            boolean module=parts[0].equals("core");JSONArray modules=access.optJSONArray("modules");if(modules!=null)for(int i=0;i<modules.length();i++)module|=parts[0].equals(modules.optJSONObject(i).optString("module_id"));
            if(!module)continue;if(access.optBoolean("is_system_administrator"))return true;
            JSONArray permissions=access.optJSONArray("permissions");if(permissions!=null)for(int i=0;i<permissions.length();i++){
                JSONObject p=permissions.optJSONObject(i);if(parts[0].equals(p.optString("module_id"))&&parts[2].equals(p.optString("capability"))&&(parts[1].equals("*")||p.optString("scope_key").equals("*")||parts[1].equals(p.optString("scope_key"))))return true;
            }
        }return false;
    }
    boolean allowed(JSONObject access,JSONObject op){
        if(!allowed(access,op.optString("permission")))return false;
        String p=op.optString("path");
        if(p.startsWith("/api/v1/reportes/")){String source=p.split("/")[4];if(source.equals("ordenes"))source="ordenes_compra";return allowed(access,source+"/"+(source.equals("gastos")?"registros":"documentos")+"/ver");}
        if(p.endsWith("convert-order")||p.endsWith("order-conversion"))return allowed(access,"ordenes_compra/documentos/editar")&&allowed(access,"ordenes_compra/documentos/ver");
        if(p.endsWith("convert-invoice")||p.endsWith("convert/invoice"))return allowed(access,"facturas/documentos/editar")&&allowed(access,"facturas/documentos/ver");
        return true;
    }
    JSONObject find(String method,String path){for(int i=0;i<operations.length();i++){JSONObject op=operations.optJSONObject(i);if(method.equals(op.optString("method"))&&path.equals(op.optString("path")))return op;}return null;}
    static String bind(String path,JSONObject variables){for(String key:keys(variables))path=path.replace("{"+key+"}",android.net.Uri.encode(variables.optString(key)));return path;}
}
