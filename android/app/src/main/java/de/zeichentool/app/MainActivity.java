package de.zeichentool.app;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.ContentResolver;
import android.content.ContentValues;
import android.content.Intent;
import android.hardware.biometrics.BiometricManager;
import android.hardware.biometrics.BiometricPrompt;
import android.net.Uri;
import android.os.CancellationSignal;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.provider.MediaStore;
import android.util.Base64;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.webkit.JavascriptInterface;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Toast;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Android-Hülle für das Zeichentool.
 *
 * Die komplette App (Rust/WebAssembly + Oberfläche) liegt in der APK unter assets/www
 * und wird über eine virtuelle HTTPS-Adresse geladen – es wird kein Internet benötigt.
 * S-Pen-Eingaben (Druck, Neigung, Schweben, Seitentaste) reicht die WebView als
 * Pointer-Events direkt an die Rust-Engine weiter.
 */
public class MainActivity extends Activity {
    private static final String HOST = "appassets.androidplatform.net";
    private static final String START_URL = "https://" + HOST + "/index.html";
    private static final int REQUEST_FILE = 4711;

    private WebView web;
    private ValueCallback<Uri[]> fileCallback;

    // --- S-Pen-Zustand (direkt aus den Android-Rohdaten) ---
    private static final int PEN_BUTTONS = MotionEvent.BUTTON_STYLUS_PRIMARY
            | MotionEvent.BUTTON_STYLUS_SECONDARY | MotionEvent.BUTTON_SECONDARY | MotionEvent.BUTTON_TERTIARY;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private boolean penButton = false;
    private boolean penInRange = false;
    private boolean penTouching = false;
    private long penLastSeen = -100000;
    /** Wie lange nach dem Stift Finger/Handballen gesperrt bleiben (aus den App-Einstellungen). */
    private volatile long palmLockMs = 2000;

    // --- Handballen-Filter ---
    /** Zeiger, die die WebView gerade als gedrückt kennt. */
    private final Set<Integer> forwarded = new HashSet<>();
    /** Zeiger (Finger/Handballen), die verworfen werden. */
    private final Set<Integer> blocked = new HashSet<>();
    /** Bedienflächen (Leisten, Menüs) in CSS-Pixeln: x, y, w, h, … – dort werden Finger nie blockiert. */
    private volatile float[] uiRects = new float[0];

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        web = new WebView(this);
        web.setBackgroundColor(0xFF15161B);
        web.setOverScrollMode(View.OVER_SCROLL_NEVER);
        web.setHapticFeedbackEnabled(false);
        web.setLongClickable(false);
        web.setOnLongClickListener(v -> true);
        setContentView(web);

        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        s.setDatabaseEnabled(true);
        s.setAllowFileAccess(false);
        s.setAllowContentAccess(true);
        s.setSupportZoom(false);
        s.setBuiltInZoomControls(false);
        s.setDisplayZoomControls(false);
        s.setTextZoom(100);
        s.setUseWideViewPort(true);
        s.setLoadWithOverviewMode(false);
        s.setCacheMode(WebSettings.LOAD_NO_CACHE);

        web.addJavascriptInterface(new Bridge(), "ZeichentoolAndroid");
        web.setWebViewClient(new AssetClient());
        web.setWebChromeClient(new ChromeClient());
        web.loadUrl(START_URL);

        hideSystemBars();
    }

    // ------------------------------------------------------------------
    // S-Pen: Taste, Schweben, Handballen
    // ------------------------------------------------------------------

    private static boolean isPen(MotionEvent e, int i) {
        int t = e.getToolType(i);
        return t == MotionEvent.TOOL_TYPE_STYLUS || t == MotionEvent.TOOL_TYPE_ERASER;
    }

    private void js(String code) {
        if (web != null) {
            web.evaluateJavascript(code, null);
        }
    }

    private void setPenInRange(boolean range) {
        if (range != penInRange) {
            penInRange = range;
            js("window.zeichentoolPenState&&window.zeichentoolPenState(" + range + ")");
        }
    }

    private void setPenButton(boolean down) {
        if (down != penButton) {
            penButton = down;
            js("window.zeichentoolPenButton&&window.zeichentoolPenButton(" + down + ")");
        }
    }

    private final Runnable hoverExitCheck = () -> {
        if (!penTouching && SystemClock.uptimeMillis() - penLastSeen >= 120) {
            setPenInRange(false);
            setPenButton(false);
        }
    };

    /** Liest Stift-Taste und -Nähe aus jedem Ereignis. */
    private void observePen(MotionEvent e) {
        int pen = -1;
        for (int i = 0; i < e.getPointerCount(); i++) {
            if (isPen(e, i)) {
                pen = i;
                break;
            }
        }
        if (pen < 0) {
            return;
        }
        int action = e.getActionMasked();
        penLastSeen = SystemClock.uptimeMillis();
        if (action == MotionEvent.ACTION_HOVER_EXIT) {
            handler.removeCallbacks(hoverExitCheck);
            handler.postDelayed(hoverExitCheck, 150);
        } else {
            setPenInRange(true);
        }
        boolean isActionPointer = e.getActionIndex() == pen;
        if (action == MotionEvent.ACTION_DOWN || (action == MotionEvent.ACTION_POINTER_DOWN && isActionPointer)) {
            penTouching = true;
        } else if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL
                || (action == MotionEvent.ACTION_POINTER_UP && isActionPointer)) {
            penTouching = false;
        }
        setPenButton((e.getButtonState() & PEN_BUTTONS) != 0);
    }

    private boolean palmLocked() {
        return penInRange || penTouching || SystemClock.uptimeMillis() - penLastSeen < palmLockMs;
    }

    /** Liegt der Punkt (Fensterpixel) auf einer Bedienfläche? */
    private boolean onUi(MotionEvent e, int i) {
        float d = getResources().getDisplayMetrics().density;
        float x = e.getX(i) / d;
        float y = e.getY(i) / d;
        float[] r = uiRects;
        for (int k = 0; k + 3 < r.length; k += 4) {
            if (x >= r[k] && x <= r[k] + r[k + 2] && y >= r[k + 1] && y <= r[k + 1] + r[k + 3]) {
                return true;
            }
        }
        return false;
    }

    /** Sehr große Auflagefläche = Handballen. */
    private boolean isPalm(MotionEvent e, int i) {
        float density = getResources().getDisplayMetrics().density;
        return e.getTouchMajor(i) > 110 * density;
    }

    @Override
    public boolean dispatchGenericMotionEvent(MotionEvent e) {
        observePen(e);
        return super.dispatchGenericMotionEvent(e);
    }

    /** S-Pen-Tasten, die Android als Tasten meldet (z. B. Bluetooth-S-Pen). */
    @Override
    public boolean dispatchKeyEvent(KeyEvent e) {
        int k = e.getKeyCode();
        // KEYCODE_STYLUS_BUTTON_PRIMARY/SECONDARY/TERTIARY/TAIL (Android 14+)
        if (k >= 308 && k <= 311) {
            if (e.getAction() == KeyEvent.ACTION_DOWN && e.getRepeatCount() == 0) {
                penButton = false;
                setPenButton(true);
            } else if (e.getAction() == KeyEvent.ACTION_UP) {
                setPenButton(false);
            }
            return true;
        }
        return super.dispatchKeyEvent(e);
    }

    /**
     * Handballen-Filter wie in Notiz-Apps: Solange der S-Pen schwebt, schreibt (und kurz danach)
     * werden Finger-/Handballen-Berührungen gar nicht erst an die Zeichenfläche weitergegeben.
     * Der Stift selbst wird immer weitergereicht.
     */
    @Override
    public boolean dispatchTouchEvent(MotionEvent e) {
        observePen(e);
        int action = e.getActionMasked();
        int ai = e.getActionIndex();
        if (action == MotionEvent.ACTION_DOWN) {
            forwarded.clear();
            blocked.clear();
        }
        if (action == MotionEvent.ACTION_DOWN || action == MotionEvent.ACTION_POINTER_DOWN) {
            if (isPen(e, ai)) {
                if (!forwarded.isEmpty() && !anyPenForwarded(e)) {
                    // Handballen/Finger lagen schon auf → in der WebView abbrechen
                    cancelForwarded(e, ai);
                }
            } else if (!onUi(e, ai) && (palmLocked() || isPalm(e, ai))) {
                blocked.add(e.getPointerId(ai));
            }
        }
        boolean anyBlocked = false;
        for (int i = 0; i < e.getPointerCount(); i++) {
            if (blocked.contains(e.getPointerId(i))) {
                anyBlocked = true;
                break;
            }
        }
        boolean result = true;
        if (!anyBlocked) {
            if (action == MotionEvent.ACTION_DOWN || action == MotionEvent.ACTION_POINTER_DOWN) {
                forwarded.add(e.getPointerId(ai));
            }
            result = super.dispatchTouchEvent(e);
        } else {
            MotionEvent out = rebuild(e, action, ai);
            if (out != null) {
                result = super.dispatchTouchEvent(out);
                out.recycle();
            }
        }
        if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
            forwarded.clear();
            blocked.clear();
        } else if (action == MotionEvent.ACTION_POINTER_UP) {
            int id = e.getPointerId(ai);
            forwarded.remove(id);
            blocked.remove(id);
        }
        return result;
    }

    private boolean anyPenForwarded(MotionEvent e) {
        for (int i = 0; i < e.getPointerCount(); i++) {
            if (isPen(e, i) && forwarded.contains(e.getPointerId(i))) {
                return true;
            }
        }
        return false;
    }

    private void cancelForwarded(MotionEvent e, int except) {
        List<Integer> idx = new ArrayList<>();
        for (int i = 0; i < e.getPointerCount(); i++) {
            if (i != except && forwarded.contains(e.getPointerId(i))) {
                idx.add(i);
            }
        }
        if (!idx.isEmpty()) {
            MotionEvent c = build(e, MotionEvent.ACTION_CANCEL, idx);
            super.dispatchTouchEvent(c);
            c.recycle();
        }
        for (int i : idx) {
            blocked.add(e.getPointerId(i));
        }
        forwarded.clear();
    }

    /** Baut ein Ereignis nur mit den erlaubten Zeigern. */
    private MotionEvent rebuild(MotionEvent e, int action, int ai) {
        List<Integer> idx = new ArrayList<>();
        for (int i = 0; i < e.getPointerCount(); i++) {
            if (!blocked.contains(e.getPointerId(i))) {
                idx.add(i);
            }
        }
        int out;
        switch (action) {
            case MotionEvent.ACTION_DOWN:
            case MotionEvent.ACTION_POINTER_DOWN: {
                int id = e.getPointerId(ai);
                if (blocked.contains(id)) {
                    return null;
                }
                forwarded.add(id);
                int pos = idx.indexOf(ai);
                out = forwarded.size() == 1 ? MotionEvent.ACTION_DOWN
                        : (MotionEvent.ACTION_POINTER_DOWN | (pos << MotionEvent.ACTION_POINTER_INDEX_SHIFT));
                if (forwarded.size() == 1) {
                    idx.clear();
                    idx.add(ai);
                }
                break;
            }
            case MotionEvent.ACTION_UP:
            case MotionEvent.ACTION_POINTER_UP: {
                int id = e.getPointerId(ai);
                if (blocked.contains(id) || !forwarded.contains(id)) {
                    return null;
                }
                int pos = idx.indexOf(ai);
                out = forwarded.size() == 1 ? MotionEvent.ACTION_UP
                        : (MotionEvent.ACTION_POINTER_UP | (pos << MotionEvent.ACTION_POINTER_INDEX_SHIFT));
                break;
            }
            case MotionEvent.ACTION_CANCEL:
                if (forwarded.isEmpty()) {
                    return null;
                }
                out = MotionEvent.ACTION_CANCEL;
                break;
            case MotionEvent.ACTION_MOVE:
                out = MotionEvent.ACTION_MOVE;
                break;
            default:
                return null;
        }
        // nur Zeiger, die die WebView kennt
        List<Integer> known = new ArrayList<>();
        for (int i : idx) {
            if (forwarded.contains(e.getPointerId(i))) {
                known.add(i);
            }
        }
        if (known.isEmpty()) {
            return null;
        }
        return build(e, out, known);
    }

    private static MotionEvent build(MotionEvent e, int action, List<Integer> idx) {
        int n = idx.size();
        MotionEvent.PointerProperties[] props = new MotionEvent.PointerProperties[n];
        MotionEvent.PointerCoords[] coords = new MotionEvent.PointerCoords[n];
        for (int k = 0; k < n; k++) {
            props[k] = new MotionEvent.PointerProperties();
            e.getPointerProperties(idx.get(k), props[k]);
            coords[k] = new MotionEvent.PointerCoords();
        }
        int hs = action == MotionEvent.ACTION_MOVE ? e.getHistorySize() : 0;
        MotionEvent out;
        if (hs > 0) {
            for (int k = 0; k < n; k++) {
                e.getHistoricalPointerCoords(idx.get(k), 0, coords[k]);
            }
            out = MotionEvent.obtain(e.getDownTime(), e.getHistoricalEventTime(0), action, n, props, coords,
                    e.getMetaState(), e.getButtonState(), e.getXPrecision(), e.getYPrecision(),
                    e.getDeviceId(), e.getEdgeFlags(), e.getSource(), e.getFlags());
            for (int h = 1; h < hs; h++) {
                for (int k = 0; k < n; k++) {
                    e.getHistoricalPointerCoords(idx.get(k), h, coords[k]);
                }
                out.addBatch(e.getHistoricalEventTime(h), coords, e.getMetaState());
            }
            for (int k = 0; k < n; k++) {
                e.getPointerCoords(idx.get(k), coords[k]);
            }
            out.addBatch(e.getEventTime(), coords, e.getMetaState());
        } else {
            for (int k = 0; k < n; k++) {
                e.getPointerCoords(idx.get(k), coords[k]);
            }
            out = MotionEvent.obtain(e.getDownTime(), e.getEventTime(), action, n, props, coords,
                    e.getMetaState(), e.getButtonState(), e.getXPrecision(), e.getYPrecision(),
                    e.getDeviceId(), e.getEdgeFlags(), e.getSource(), e.getFlags());
        }
        return out;
    }

    // ------------------------------------------------------------------
    // Vollbild
    // ------------------------------------------------------------------

    @SuppressWarnings("deprecation")
    private void hideSystemBars() {
        if (Build.VERSION.SDK_INT >= 30) {
            getWindow().setDecorFitsSystemWindows(false);
            WindowInsetsController c = getWindow().getInsetsController();
            if (c != null) {
                c.hide(WindowInsets.Type.systemBars());
                c.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        } else {
            getWindow().getDecorView().setSystemUiVisibility(
                    View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                            | View.SYSTEM_UI_FLAG_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN);
        }
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            hideSystemBars();
        }
    }

    // ------------------------------------------------------------------
    // Lebenszyklus
    // ------------------------------------------------------------------

    @Override
    protected void onPause() {
        super.onPause();
        if (web != null) {
            web.onPause();
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (web != null) {
            web.onResume();
        }
        hideSystemBars();
    }

    @Override
    protected void onDestroy() {
        if (web != null) {
            web.destroy();
            web = null;
        }
        super.onDestroy();
    }

    /** Zurück-Taste: Menü schließen → zurück zur Bibliothek → App verlassen (alles ist autogespeichert). */
    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed() {
        if (web == null) {
            finish();
            return;
        }
        web.evaluateJavascript(
                "(function(){if(document.querySelector('.popover:not(.hidden),.modal:not(.hidden)')){"
                        + "window.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape'}));return 1;}"
                        + "var e=document.getElementById('editor');if(e&&!e.classList.contains('hidden')){"
                        + "document.querySelector('[data-act=back]').click();return 1;}return 0;})()",
                result -> {
                    if (!"1".equals(result)) {
                        finish();
                    }
                });
    }

    // ------------------------------------------------------------------
    // Dateiauswahl (Projekt öffnen, Bild importieren)
    // ------------------------------------------------------------------

    @Override
    @SuppressWarnings("deprecation")
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        if (requestCode == REQUEST_FILE) {
            Uri[] result = null;
            if (resultCode == RESULT_OK && data != null && data.getData() != null) {
                result = new Uri[]{data.getData()};
            }
            if (fileCallback != null) {
                fileCallback.onReceiveValue(result);
                fileCallback = null;
            }
            return;
        }
        super.onActivityResult(requestCode, resultCode, data);
    }

    private class ChromeClient extends WebChromeClient {
        @Override
        @SuppressWarnings("deprecation")
        public boolean onShowFileChooser(WebView view, ValueCallback<Uri[]> callback, FileChooserParams params) {
            if (fileCallback != null) {
                fileCallback.onReceiveValue(null);
            }
            fileCallback = callback;
            // Nur einschränken, wenn ausschließlich Bilder bzw. ausschließlich PDFs erlaubt sind –
            // sonst (z. B. Import: PDF, Bild, .znote) alle Dateien zeigen.
            boolean any = false, allImage = true, allPdf = true;
            String[] types = params.getAcceptTypes();
            if (types != null) {
                for (String raw : types) {
                    if (raw == null) {
                        continue;
                    }
                    for (String part : raw.split(",")) {
                        String t = part.trim().toLowerCase();
                        if (t.isEmpty()) {
                            continue;
                        }
                        any = true;
                        if (!t.startsWith("image")) {
                            allImage = false;
                        }
                        if (!t.equals("application/pdf") && !t.equals(".pdf")) {
                            allPdf = false;
                        }
                    }
                }
            }
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            i.addCategory(Intent.CATEGORY_OPENABLE);
            i.setType(any && allImage ? "image/*" : any && allPdf ? "application/pdf" : "*/*");
            try {
                startActivityForResult(i, REQUEST_FILE);
            } catch (ActivityNotFoundException e) {
                fileCallback = null;
                callback.onReceiveValue(null);
                return false;
            }
            return true;
        }
    }

    // ------------------------------------------------------------------
    // App-Dateien aus der APK ausliefern
    // ------------------------------------------------------------------

    private class AssetClient extends WebViewClient {
        @Override
        public WebResourceResponse shouldInterceptRequest(WebView view, WebResourceRequest request) {
            Uri uri = request.getUrl();
            if (!HOST.equals(uri.getHost())) {
                return null;
            }
            String path = uri.getPath();
            if (path == null || path.isEmpty() || "/".equals(path)) {
                path = "/index.html";
            }
            if (path.contains("..")) {
                return notFound();
            }
            try {
                InputStream in = getAssets().open("www" + path);
                String mime = mimeType(path);
                WebResourceResponse res = new WebResourceResponse(mime, mime.startsWith("text/") || mime.endsWith("json") || mime.endsWith("javascript") ? "utf-8" : null, in);
                Map<String, String> headers = new HashMap<>();
                headers.put("Cache-Control", "no-cache");
                headers.put("Access-Control-Allow-Origin", "*");
                res.setResponseHeaders(headers);
                return res;
            } catch (IOException e) {
                return notFound();
            }
        }

        @Override
        public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
            Uri uri = request.getUrl();
            if (HOST.equals(uri.getHost())) {
                return false;
            }
            try {
                startActivity(new Intent(Intent.ACTION_VIEW, uri));
            } catch (ActivityNotFoundException ignored) {
            }
            return true;
        }

        private WebResourceResponse notFound() {
            return new WebResourceResponse("text/plain", "utf-8", 404, "Not Found", null, null);
        }
    }

    private static String mimeType(String path) {
        String p = path.toLowerCase();
        if (p.endsWith(".html")) return "text/html";
        if (p.endsWith(".js") || p.endsWith(".mjs")) return "text/javascript";
        if (p.endsWith(".wasm")) return "application/wasm";
        if (p.endsWith(".css")) return "text/css";
        if (p.endsWith(".json")) return "application/json";
        if (p.endsWith(".webmanifest")) return "application/manifest+json";
        if (p.endsWith(".svg")) return "image/svg+xml";
        if (p.endsWith(".png")) return "image/png";
        if (p.endsWith(".jpg") || p.endsWith(".jpeg")) return "image/jpeg";
        if (p.endsWith(".webp")) return "image/webp";
        return "application/octet-stream";
    }

    // ------------------------------------------------------------------
    // Brücke für die Rust-App: Dateien in Galerie / Downloads speichern
    // ------------------------------------------------------------------

    /** Ergebnis des Fingerabdrucks an die Web-App melden. */
    private void authResult(boolean ok) {
        runOnUiThread(() -> web.evaluateJavascript("window.zeichentoolAuthResult&&window.zeichentoolAuthResult(" + ok + ")", null));
    }

    public class Bridge {
        /** Gibt es einen eingerichteten Fingerabdruck / eine Gesichtserkennung? */
        @JavascriptInterface
        public boolean canAuthenticate() {
            try {
                BiometricManager bm = getSystemService(BiometricManager.class);
                if (bm == null) {
                    return false;
                }
                int r = Build.VERSION.SDK_INT >= 30
                        ? bm.canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_WEAK)
                        : bm.canAuthenticate();
                return r == BiometricManager.BIOMETRIC_SUCCESS;
            } catch (Exception e) {
                return false;
            }
        }

        /** Fingerabdruck-Abfrage zum Entsperren eines Notizbuchs. */
        @JavascriptInterface
        public void authenticate(String title) {
            runOnUiThread(() -> {
                try {
                    BiometricPrompt prompt = new BiometricPrompt.Builder(MainActivity.this)
                            .setTitle(title == null || title.isEmpty() ? "Notizbuch entsperren" : title)
                            .setSubtitle("Notizbuch entsperren")
                            .setNegativeButton("PIN eingeben", getMainExecutor(), (d, w) -> authResult(false))
                            .build();
                    prompt.authenticate(new CancellationSignal(), getMainExecutor(), new BiometricPrompt.AuthenticationCallback() {
                        @Override
                        public void onAuthenticationSucceeded(BiometricPrompt.AuthenticationResult result) {
                            authResult(true);
                        }

                        @Override
                        public void onAuthenticationError(int code, CharSequence msg) {
                            authResult(false);
                        }
                    });
                } catch (Exception e) {
                    authResult(false);
                }
            });
        }

        /** Speichert eine Datei. Bilder landen in „Bilder/Zeichentool“, Projekte in „Download/Zeichentool“. */
        @JavascriptInterface
        public boolean saveFile(String name, String mime, String base64) {
            try {
                byte[] data = Base64.decode(base64, Base64.DEFAULT);
                boolean image = mime != null && mime.startsWith("image/");
                ContentValues v = new ContentValues();
                v.put(MediaStore.MediaColumns.DISPLAY_NAME, name);
                v.put(MediaStore.MediaColumns.MIME_TYPE, mime);
                v.put(MediaStore.MediaColumns.RELATIVE_PATH,
                        (image ? Environment.DIRECTORY_PICTURES : Environment.DIRECTORY_DOWNLOADS) + "/Zeichentool");
                v.put(MediaStore.MediaColumns.IS_PENDING, 1);
                Uri collection = image
                        ? MediaStore.Images.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
                        : MediaStore.Downloads.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY);
                ContentResolver cr = getContentResolver();
                Uri uri = cr.insert(collection, v);
                if (uri == null) {
                    return false;
                }
                try (OutputStream os = cr.openOutputStream(uri)) {
                    if (os == null) {
                        return false;
                    }
                    os.write(data);
                }
                v.clear();
                v.put(MediaStore.MediaColumns.IS_PENDING, 0);
                cr.update(uri, v, null, null);
                final String where = image ? "Galerie (Bilder/Zeichentool)" : "Download/Zeichentool";
                runOnUiThread(() -> Toast.makeText(MainActivity.this, "Gespeichert in " + where, Toast.LENGTH_SHORT).show());
                return true;
            } catch (Exception e) {
                return false;
            }
        }

        /** Sperrzeit nach dem Stift in Sekunden (aus den Einstellungen der App). */
        @JavascriptInterface
        public void setPalmLock(double seconds) {
            palmLockMs = (long) Math.max(0, Math.min(10_000, seconds * 1000));
        }

        /** Bedienflächen der Oberfläche: "x,y,w,h;x,y,w,h…" in CSS-Pixeln. */
        @JavascriptInterface
        public void setUiRects(String spec) {
            List<Float> v = new ArrayList<>();
            if (spec != null) {
                for (String part : spec.split(";")) {
                    String[] n = part.split(",");
                    if (n.length != 4) {
                        continue;
                    }
                    try {
                        for (String x : n) {
                            v.add(Float.parseFloat(x));
                        }
                    } catch (NumberFormatException ignored) {
                        return;
                    }
                }
            }
            float[] r = new float[v.size()];
            for (int i = 0; i < r.length; i++) {
                r[i] = v.get(i);
            }
            uiRects = r;
        }

        /** Datei über das Android-Teilen-Menü an andere Apps schicken (WhatsApp, Mail, Drive …). */
        @JavascriptInterface
        public boolean shareFile(String name, String mime, String base64) {
            try {
                byte[] data = Base64.decode(base64, Base64.DEFAULT);
                java.io.File dir = ShareProvider.dir(MainActivity.this);
                // alte geteilte Dateien aufräumen
                java.io.File[] old = dir.listFiles();
                if (old != null) {
                    for (java.io.File f : old) {
                        //noinspection ResultOfMethodCallIgnored
                        f.delete();
                    }
                }
                String safe = name.replaceAll("[^A-Za-z0-9ÄÖÜäöüß ._-]", "_");
                java.io.File f = new java.io.File(dir, safe);
                try (java.io.FileOutputStream os = new java.io.FileOutputStream(f)) {
                    os.write(data);
                }
                Uri uri = new Uri.Builder().scheme("content").authority(ShareProvider.AUTHORITY).appendPath(safe).build();
                Intent send = new Intent(Intent.ACTION_SEND);
                send.setType(mime == null || mime.isEmpty() ? "application/octet-stream" : mime);
                send.putExtra(Intent.EXTRA_STREAM, uri);
                send.putExtra(Intent.EXTRA_SUBJECT, safe);
                send.setClipData(android.content.ClipData.newRawUri(safe, uri));
                send.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                Intent chooser = Intent.createChooser(send, "Teilen");
                chooser.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                runOnUiThread(() -> startActivity(chooser));
                return true;
            } catch (Exception e) {
                return false;
            }
        }

        @JavascriptInterface
        public String version() {
            try {
                return getPackageManager().getPackageInfo(getPackageName(), 0).versionName;
            } catch (Exception e) {
                return "";
            }
        }
    }
}
