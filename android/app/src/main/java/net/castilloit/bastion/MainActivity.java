package net.castilloit.bastion;

import android.os.Bundle;
import android.content.Intent;
import com.google.androidbrowserhelper.trusted.LauncherActivity;

/** Launch only the configured site, including for explicit third-party intents. */
public final class MainActivity extends LauncherActivity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        getIntent().setData(null);
        getIntent().setClipData(null);
        getIntent().setAction(Intent.ACTION_MAIN);
        getIntent().replaceExtras((Bundle) null);
        super.onCreate(savedInstanceState);
    }
}
