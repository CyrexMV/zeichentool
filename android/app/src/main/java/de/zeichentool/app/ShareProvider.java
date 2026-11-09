package de.zeichentool.app;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;

/**
 * Stellt Dateien aus dem Teilen-Ordner (cache/share) für andere Apps bereit
 * (nur lesend und nur mit erteilter Freigabe über das Teilen-Intent).
 */
public class ShareProvider extends ContentProvider {
    public static final String AUTHORITY = "de.zeichentool.app.share";

    static File dir(android.content.Context c) {
        File d = new File(c.getCacheDir(), "share");
        //noinspection ResultOfMethodCallIgnored
        d.mkdirs();
        return d;
    }

    private File file(Uri uri) throws FileNotFoundException {
        String name = uri.getLastPathSegment();
        if (name == null || getContext() == null) {
            throw new FileNotFoundException();
        }
        File d = dir(getContext());
        File f = new File(d, name);
        try {
            // keine Pfade außerhalb des Teilen-Ordners
            if (!f.getCanonicalFile().getParentFile().equals(d.getCanonicalFile()) || !f.exists()) {
                throw new FileNotFoundException();
            }
        } catch (IOException e) {
            throw new FileNotFoundException();
        }
        return f;
    }

    @Override
    public boolean onCreate() {
        return true;
    }

    @Override
    public Cursor query(Uri uri, String[] projection, String selection, String[] args, String sort) {
        try {
            File f = file(uri);
            MatrixCursor c = new MatrixCursor(new String[] {OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE});
            c.addRow(new Object[] {f.getName(), f.length()});
            return c;
        } catch (FileNotFoundException e) {
            return null;
        }
    }

    @Override
    public String getType(Uri uri) {
        String n = uri.getLastPathSegment() == null ? "" : uri.getLastPathSegment().toLowerCase();
        if (n.endsWith(".pdf")) return "application/pdf";
        if (n.endsWith(".png")) return "image/png";
        return "application/octet-stream";
    }

    @Override
    public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        return ParcelFileDescriptor.open(file(uri), ParcelFileDescriptor.MODE_READ_ONLY);
    }

    @Override
    public Uri insert(Uri uri, ContentValues values) {
        throw new UnsupportedOperationException();
    }

    @Override
    public int delete(Uri uri, String selection, String[] args) {
        return 0;
    }

    @Override
    public int update(Uri uri, ContentValues values, String selection, String[] args) {
        return 0;
    }
}
