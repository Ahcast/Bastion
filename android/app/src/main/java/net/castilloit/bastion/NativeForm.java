package net.castilloit.bastion;

import android.app.DatePickerDialog;
import android.text.InputType;
import android.text.InputFilter;
import android.view.View;
import android.widget.*;
import org.json.*;
import java.util.*;

/** Reusable Android widgets, driven only by the app's reviewed domain contracts. */
final class NativeForm {
    interface Value { Object get() throws Exception; }
    final MainActivity activity;
    final JSONObject schema, initial;
    final LinearLayout view;
    final Map<String,Value> fields=new LinkedHashMap<>();
    final Map<String,EditText> inputs=new HashMap<>();
    NativeForm(MainActivity activity,JSONObject schema,JSONObject initial){
        this.activity=activity;this.schema=schema;this.initial=initial;view=activity.column();
        JSONObject properties=schema.optJSONObject("properties");
        if(properties!=null)for(String key:Contracts.keys(properties)){
            JSONObject s=properties.optJSONObject(key);if(s==null)continue;
            boolean required=Contracts.has(schema.optJSONArray("required"),key);
            fields.put(key,field(view,key,s,initial.opt(key),required));
        }
        // Open dictionaries (tender values) have native key/value rows, never raw JSON editors.
        if(properties==null&&schema.opt("additionalProperties") instanceof JSONObject){
            JSONObject valueSchema=schema.optJSONObject("additionalProperties");
            for(String key:Contracts.keys(initial))fields.put(key,field(view,key,valueSchema,initial.opt(key),false));
            activity.button(view,"Agregar valor",()->{
                EditText keyInput=activity.input("Nombre de variable",false);
                new android.app.AlertDialog.Builder(activity).setTitle("Agregar variable").setView(keyInput).setPositiveButton("Agregar",(d,w)->{
                    String key=keyInput.getText().toString().trim();if(!key.isEmpty()&&!fields.containsKey(key))fields.put(key,field(view,key,valueSchema,null,false));
                }).setNegativeButton("Cancelar",null).show();
            });
        }
    }
    JSONObject value() throws Exception {
        JSONObject result=new JSONObject();
        for(Map.Entry<String,Value> f:fields.entrySet()){Object v=f.getValue().get();if(v!=null)result.put(f.getKey(),v);}
        return result;
    }
    void set(String key,String value){EditText e=inputs.get(key);if(e!=null)e.setText(value);}
    private Value field(LinearLayout parent,String key,JSONObject s,Object original,boolean required){
        Object seed=original==JSONObject.NULL?null:original;
        if(seed==null&&s.has("default"))seed=s.opt("default");
        if(s.has("const"))seed=s.opt("const");
        if(seed==JSONObject.NULL)seed=null;
        if(key.equals("setup_token")){final Object token=seed;return ()->token;}
        String type=Contracts.type(s);
        String title=Labels.of(key)+(required&&!Contracts.nullable(s)?" *":"");
        if(type.equals("object")){
            LinearLayout group=activity.card(parent);activity.heading(group,title,18);
            NativeForm nested=new NativeForm(activity,s,Contracts.object(seed));
            boolean optional=Contracts.nullable(s)||!required;
            CheckBox include=new CheckBox(activity);include.setText("Incluir "+Labels.of(key).toLowerCase(new Locale("es")));include.setChecked(seed instanceof JSONObject);
            if(optional){group.addView(include);nested.view.setVisibility(include.isChecked()?View.VISIBLE:View.GONE);include.setOnCheckedChangeListener((b,on)->nested.view.setVisibility(on?View.VISIBLE:View.GONE));}
            group.addView(nested.view);
            return ()->optional&&!include.isChecked()?(required?JSONObject.NULL:null):nested.value();
        }
        if(type.equals("array")){
            LinearLayout group=activity.card(parent);activity.heading(group,title,18);
            JSONObject item=s.optJSONObject("items");if(item==null)item=new JSONObject();final JSONObject itemSchema=item;
            ArrayList<Value> values=new ArrayList<>();LinearLayout rows=activity.column();group.addView(rows);
            java.util.function.Consumer<Object> add=value->{
                LinearLayout row=activity.card(rows);Value v;
                if(value instanceof JSONObject&&Contracts.type(itemSchema).equals("object")){
                    JSONObject object=(JSONObject)value;String label=object.optString("description",object.optString("descripcion",object.optString("label",object.optString("account","Elemento"))));activity.text(row,label,16);
                    final Value[] editor={()->object};final boolean[] opened={false};LinearLayout content=activity.column();content.setVisibility(View.GONE);row.addView(content);
                    activity.button(row,"Editar elemento",()->{if(!opened[0]){editor[0]=field(content,"",itemSchema,value,true);opened[0]=true;}content.setVisibility(content.getVisibility()==View.VISIBLE?View.GONE:View.VISIBLE);});v=()->editor[0].get();
                }else v=field(row,"",itemSchema,value,true);
                values.add(v);
                activity.button(row,"Quitar",()->{values.remove(v);rows.removeView(row);});
            };
            if(seed instanceof JSONArray){JSONArray a=(JSONArray)seed;for(int i=0;i<a.length();i++)add.accept(a.opt(i));}
            else for(int i=0;i<Math.min(s.optInt("minItems",0),10);i++)add.accept(null);
            activity.button(group,"Agregar "+Labels.of(key).toLowerCase(new Locale("es")),()->{if(values.size()<s.optInt("maxItems",100))add.accept(null);else activity.error("Se alcanzó el máximo de elementos.");});
            return ()->{if(values.size()<s.optInt("minItems",0))throw new IllegalArgumentException(title+": faltan elementos.");JSONArray a=new JSONArray();for(Value v:values)a.put(v.get());return a;};
        }
        if(s.optJSONArray("type")!=null&&Contracts.has(s.optJSONArray("type"),"boolean")&&Contracts.has(s.optJSONArray("type"),"string")){
            LinearLayout group=activity.column();parent.addView(group);activity.text(group,title,14);
            Spinner format=new Spinner(activity);format.setAdapter(new ArrayAdapter<>(activity,android.R.layout.simple_spinner_dropdown_item,new String[]{"Texto","Sí / No","Sin valor"}));format.setMinimumHeight(activity.dp(48));group.addView(format);
            EditText text=activity.input("Valor",false);if(seed instanceof String)text.setText((String)seed);group.addView(text);
            CheckBox check=new CheckBox(activity);check.setText("Sí");check.setChecked(Boolean.TRUE.equals(seed));group.addView(check);
            format.setSelection(seed instanceof Boolean?1:seed==null&&Contracts.nullable(s)?2:0);
            format.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener(){public void onNothingSelected(AdapterView<?> p){}public void onItemSelected(AdapterView<?> p,View v,int position,long id){text.setVisibility(position==0?View.VISIBLE:View.GONE);check.setVisibility(position==1?View.VISIBLE:View.GONE);}});
            return ()->format.getSelectedItemPosition()==0?text.getText().toString():format.getSelectedItemPosition()==1?check.isChecked():JSONObject.NULL;
        }
        if(type.equals("boolean")){
            CheckBox checkbox=new CheckBox(activity);checkbox.setText(title);checkbox.setTextSize(16);checkbox.setMinHeight(activity.dp(48));checkbox.setChecked(Boolean.TRUE.equals(seed));parent.addView(checkbox);
            if(!required&&original==null){CheckBox include=new CheckBox(activity);include.setText("Modificar "+Labels.of(key).toLowerCase(new Locale("es")));parent.addView(include);checkbox.setEnabled(false);include.setOnCheckedChangeListener((b,on)->checkbox.setEnabled(on));return ()->include.isChecked()?checkbox.isChecked():null;}
            return checkbox::isChecked;
        }
        if(key.equals("content_base64")){
            final String[] content={seed instanceof String?(String)seed:null};
            TextView label=activity.text(parent,"Ningún archivo seleccionado",14);
            activity.button(parent,"Seleccionar archivo",()->activity.pickFile((name,bytes)->{content[0]=android.util.Base64.encodeToString(bytes,android.util.Base64.NO_WRAP);set("filename",name);if(inputs.containsKey("title")&&inputs.get("title").getText().length()==0)set("title",name);label.setText(name+" · "+bytes.length/1024+" KB");}));
            return ()->{if(content[0]==null&&required)throw new IllegalArgumentException("Selecciona un archivo.");return content[0];};
        }
        JSONArray choices=s.optJSONArray("enum");
        if(choices!=null){
            activity.text(parent,title,14);Spinner spinner=new Spinner(activity);ArrayList<String> raw=new ArrayList<>(),labels=new ArrayList<>();
            if(!required){raw.add("");labels.add("Todos / sin seleccionar");}
            for(int i=0;i<choices.length();i++){raw.add(choices.optString(i));labels.add(Labels.of(choices.optString(i)));}
            if(seed!=null&&!raw.contains(seed.toString())){raw.add(seed.toString());labels.add(Labels.of(seed.toString()));}
            ArrayAdapter<String> adapter=new ArrayAdapter<>(activity,android.R.layout.simple_spinner_item,labels);adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item);spinner.setAdapter(adapter);spinner.setMinimumHeight(activity.dp(48));
            if(seed!=null&&raw.contains(seed.toString()))spinner.setSelection(raw.indexOf(seed.toString()));parent.addView(spinner);
            return ()->{String v=raw.get(spinner.getSelectedItemPosition());return v.isEmpty()&&!required?null:type.equals("integer")?Long.valueOf(v):v;};
        }
        activity.text(parent,title,14);
        EditText input=activity.input(Labels.of(key),s.optString("format").equals("password")||key.contains("password"));inputs.put(key,input);parent.addView(input);
        if(seed!=null)input.setText(seed.toString());
        if(s.has("const")||s.optBoolean("readOnly"))input.setEnabled(false);
        if(type.equals("integer"))input.setInputType(InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_FLAG_SIGNED);
        else if(type.equals("number")||key.matches(".*(price|amount|quantity|discount_value|debit|credit|precio_unitario)$"))input.setInputType(InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_FLAG_DECIMAL|InputType.TYPE_NUMBER_FLAG_SIGNED);
        else if(key.contains("email")||s.optString("format").equals("email"))input.setInputType(InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS);
        else if(key.contains("phone"))input.setInputType(InputType.TYPE_CLASS_PHONE);
        else if(key.matches("description|descripcion|notes|note|terms|comments|coments|public_notes|message|direction")){input.setSingleLine(false);input.setMinLines(2);input.setMaxLines(6);input.setInputType(InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_FLAG_MULTI_LINE|InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);}
        if(s.has("maxLength"))input.setFilters(new InputFilter[]{new InputFilter.LengthFilter(s.optInt("maxLength"))});
        if(s.optString("format").equals("date")){
            input.setHint("AAAA-MM-DD");activity.button(parent,"Elegir fecha",()->{
                Calendar c=Calendar.getInstance(TimeZone.getTimeZone("America/Tegucigalpa"));
                String[] date=input.getText().toString().split("-");try{if(date.length==3)c.set(Integer.parseInt(date[0]),Integer.parseInt(date[1])-1,Integer.parseInt(date[2]));}catch(NumberFormatException ignored){}
                new DatePickerDialog(activity,(p,y,m,d)->input.setText(String.format(Locale.ROOT,"%04d-%02d-%02d",y,m+1,d)),c.get(Calendar.YEAR),c.get(Calendar.MONTH),c.get(Calendar.DAY_OF_MONTH)).show();
            });
        }
        if(key.equals("id_client"))activity.button(parent,"Buscar cliente",()->activity.selectCatalog("clientes",row->{set("id_client",row.optString("id"));set("client_name",row.optString("cliente_name",row.optString("name")));set("client_rtn",row.optString("rtn"));set("client_email",row.optString("email"));}));
        if(key.equals("id_supplier"))activity.button(parent,"Buscar proveedor",()->activity.selectCatalog("proveedores",row->{set("id_supplier",row.optString("id"));set("supplier_name",row.optString("supplier_name"));}));
        return ()->{
            String value=input.getText().toString();boolean empty=value.trim().isEmpty();
            if(empty&&Contracts.nullable(s))return JSONObject.NULL;
            if(empty&&!required&&!initial.has(key)&&!s.has("default"))return null;
            if(value.length()<s.optInt("minLength",0))throw new IllegalArgumentException(title+": completa este campo.");
            if(!empty&&s.has("pattern")&&!value.matches(s.optString("pattern")))throw new IllegalArgumentException(title+": formato inválido.");
            if(type.equals("integer")||type.equals("number")){
                if(empty)throw new IllegalArgumentException(title+": escribe un número.");
                java.math.BigDecimal number;try{number=new java.math.BigDecimal(value);}catch(NumberFormatException e){throw new IllegalArgumentException(title+": número inválido.");}
                if(s.has("minimum")&&number.compareTo(new java.math.BigDecimal(s.optString("minimum")))<0||s.has("maximum")&&number.compareTo(new java.math.BigDecimal(s.optString("maximum")))>0)throw new IllegalArgumentException(title+": número fuera del rango permitido.");
                if(type.equals("integer"))return number.longValueExact();return number;
            }
            return value;
        };
    }
}
