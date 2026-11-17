package net.castilloit.bastion;

import org.json.*;
import java.text.SimpleDateFormat;
import java.util.*;
import java.util.regex.*;

/** Mirrors web/src/processQuote.ts: numbers in model/specification text are not quantities. */
final class ProcessQuote {
    static JSONObject prepare(JSONObject row) throws Exception {
        String raw=Contracts.object(row.opt("source")).optString("items");JSONArray descriptions=null;
        try{if(raw.trim().startsWith("[")){JSONArray parsed=new JSONArray(raw);boolean strings=true;for(int i=0;i<parsed.length();i++)strings&=parsed.opt(i) instanceof String;if(strings)descriptions=parsed;}}catch(JSONException ignored){}
        if(descriptions==null){descriptions=new JSONArray();for(String line:raw.split("\\r?\\n"))if(!line.trim().isEmpty())descriptions.put(line.replaceFirst("^\\d+\\.\\s+", ""));}
        if(descriptions.length()==0||descriptions.length()>200)throw new IllegalArgumentException("El proceso debe contener entre 1 y 200 ítems analizados.");
        JSONArray items=new JSONArray();for(int i=0;i<descriptions.length();i++)items.put(item(descriptions.getString(i).trim()));
        if(row.optString("entidad").length()>255||row.optString("expediente").length()>100)throw new IllegalArgumentException("La entidad o expediente supera el tamaño permitido; no se recortaron los datos.");
        SimpleDateFormat date=new SimpleDateFormat("yyyy-MM-dd",Locale.ROOT);date.setTimeZone(TimeZone.getTimeZone("America/Tegucigalpa"));
        return new JSONObject().put("client_name",row.optString("entidad")).put("order_number",row.optString("expediente")).put("quote_date",date.format(new Date())).put("valid_days",30).put("status","Borrador").put("notes","").put("terms","Tiempo de entrega: ").put("discount_type","amount").put("discount_value","0").put("items",items);
    }
    static JSONObject item(String text) throws Exception {
        String description=text,quantity="";String[] patterns={"^(?:cantidad|cant\\.?|qty|quantity)\\s*[:=]?\\s*([0-9]+(?:[.,][0-9]+)*)\\s*(?:[×x:;,]\\s*|\\s+)(.+)$","^([0-9]+(?:[.,][0-9]+)*)\\s*[×x]\\s*(.+)$","^([0-9]+(?:[.,][0-9]+)*)\\s+(.+)$"};
        for(int i=0;i<patterns.length;i++){Matcher m=Pattern.compile(patterns[i],Pattern.CASE_INSENSITIVE|Pattern.UNICODE_CASE).matcher(text);if(!m.matches())continue;String q=m.group(1),d=m.group(2).trim();
            if(q.matches("^[1-9][0-9]{0,2}(?:,[0-9]{3})+(?:\\.[0-9]{1,4})?$"))q=q.replace(",","");else if(q.matches("^[0-9]{1,12},[0-9]{1,4}$"))q=q.replace(',','.');
            boolean spec=Pattern.compile("^(?:v(?:dc|ac)?|a|ah|hz|khz|w|kw|kv|hp|rpm|mm|cm|pulg(?:adas?|\\.)?|inch(?:es)?|gb|mb|tb|puertos|voltios|amperios|usd|hnl|l)(?=[\\s,;:.]|$)",Pattern.CASE_INSENSITIVE).matcher(d).find();
            if(q.matches("^[0-9]{1,12}(?:\\.[0-9]{1,4})?$")&&!d.isEmpty()&&!Character.isDigit(d.charAt(0))&&(i<2||!spec)){quantity=q.replaceFirst("^0+(?=[0-9])","");description=d;}break;
        }
        if(description.length()>3000)throw new IllegalArgumentException("Una especificación supera 3000 caracteres. Revisa el proceso original.");
        return new JSONObject().put("description",description).put("quantity",quantity).put("unit_price","0").put("discount_type","amount").put("discount_value","0").put("tax_type","isv15");
    }
}
