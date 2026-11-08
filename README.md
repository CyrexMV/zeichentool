# Zeichentool

Ein modernes Zeichenprogramm, **geschrieben in Rust** (kompiliert zu WebAssembly) und als
installierbare App (PWA) für Tablets gebaut – optimiert für das **Galaxy Tab S12 Ultra mit S‑Pen**.

Die komplette Zeichen‑Engine (Pinsel, Ebenen, Mischmodi, Füllen, Wischen, Verlauf, Dateiformat)
und die gesamte Eingabe‑ und Oberflächenlogik laufen in Rust. HTML/CSS liefern nur das Aussehen.

## Funktionen

**S‑Pen**
- Druckempfindlichkeit mit einstellbarer Druckkurve (weich / normal / hart)
- Neigung: Bleistift und Kohle schattieren breiter, wenn der Stift schräg gehalten wird
- Schweben (Hover): Pinselgröße wird als Kreis angezeigt, bevor der Stift aufsetzt
- S‑Pen‑Taste: halten = Radierer (oder Pipette); alternativ Klick = Rückgängig bzw. Pinsel ↔ Radierer
- Handballen‑Erkennung: Finger werden ignoriert, während der Stift zeichnet
- Alle Zwischenpunkte des Stifts (bis 240 Hz) werden verarbeitet, Low‑Latency‑Canvas
- S‑Pen‑Testfeld in den Einstellungen (Druck, Neigung, Taste live anzeigen)

**Zeichnen**
- 14 Pinsel: Fineliner, Tusche, Kalligraphie, Bleistift, Kohle, Rundpinsel, Flachpinsel, Aquarell,
  Weicher Pinsel, Airbrush, Marker, Textmarker, Pixel, Sprühdose – jeweils anpassbar
  (Härte, Abstand, Fluss, Druck → Größe/Deckkraft, Körnung, Neigung)
- Stabilisierung für ruhige Linien
- Radierer (mit jeder Pinselspitze), Wischfinger
- Füllwerkzeug (Toleranz, alle Ebenen oder nur aktuelle Ebene)
- Formen: Linie, Rechteck, Ellipse, Lasso‑Füllung, gefüllt oder als Kontur
- **QuickShape**: Stift am Strichende halten → perfekte Linie bzw. Kreis/Ellipse
- **Symmetrie**: vertikal, horizontal, vierfach, radial (2–24 Segmente)
- Pipette (auch: Finger lange halten)

**Ebenen**
- Bis zu 48 Ebenen, Deckkraft, 9 Mischmodi, Transparenz sperren, Ebene sperren
- Duplizieren, Zusammenfügen, Spiegeln, Leeren, Verschieben, Bild als Ebene importieren
- Hintergrundfarbe oder transparent

**Bedienung**
- Zwei Finger: zoomen, drehen (rastet bei 0°/90° ein), verschieben
- Zwei‑Finger‑Tipp = Rückgängig, Drei‑Finger‑Tipp = Wiederholen
- Ansicht spiegeln zur Proportionskontrolle, Vollbild
- Tastatur (z. B. Book Cover Keyboard): B, E, S, G, I, U, V, L, C, [ ], 0, Strg+Z/Y/S, Leertaste+Ziehen

**Speichern**
- Automatisches Speichern im Gerät (IndexedDB) – die Zeichnung ist nach dem Neustart wieder da
- Projektdatei `.zeichen` (alle Ebenen) speichern und öffnen
- Export als PNG (mit oder ohne Hintergrund)
- Funktioniert offline (Service Worker)

## Auf das Galaxy Tab installieren

1. Die App ist über **GitHub Pages** veröffentlicht: **https://cyrexmv.github.io/zeichentool/**
   - Ausgeliefert wird der Branch `gh-pages` (*Settings → Pages → Deploy from a branch → gh-pages / root*).
   - Jeder Push nach `main` baut die App über `.github/workflows/pages.yml` neu und aktualisiert `gh-pages`.
2. Die Adresse auf dem Tablet in **Samsung Internet** oder **Chrome** öffnen.
3. Menü → **„Seite hinzufügen zu → Startbildschirm“** (Samsung Internet) bzw.
   **„App installieren“** (Chrome).
4. Das Zeichentool erscheint als App auf dem Startbildschirm, startet im Vollbild und läuft offline.

Optional kann aus der veröffentlichten Adresse mit [PWABuilder](https://www.pwabuilder.com/)
eine echte Android‑APK erzeugt werden.

## Als Android-App (APK) installieren

Die App gibt es auch als eigene APK. Sie bringt die komplette Rust/WebAssembly-App mit,
läuft ohne Internet im Vollbild und speichert Exporte direkt in die Galerie
(*Bilder/Zeichentool*) bzw. Projekte nach *Download/Zeichentool*.

1. Auf dem Tablet **https://cyrexmv.github.io/zeichentool/zeichentool.apk** öffnen
   (oder im Zeichentool: Menü → „Android-App (APK) herunterladen“).
2. Beim ersten Mal die Installation aus dieser Quelle erlauben
   (*Einstellungen → Apps → Spezieller Zugriff → Unbekannte Apps installieren*).
3. APK öffnen → **Installieren**.

Jeder Push baut über `.github/workflows/pages.yml` eine neue APK mit höherer Versionsnummer.

**Signatur (wichtig für Updates):** Android installiert ein Update nur, wenn es mit demselben
Schlüssel signiert ist. Ohne eigene Secrets signiert die CI mit einem *temporären* Schlüssel –
die APK funktioniert, aber für ein Update muss die alte App vorher deinstalliert werden
(vorher die Zeichnung über Menü → „Projekt speichern“ sichern). Für dauerhafte Updates einmalig
einen Schlüssel erzeugen und in GitHub unter *Settings → Secrets and variables → Actions* hinterlegen:

```bash
keytool -genkeypair -keystore zeichentool.jks -alias zeichentool -keyalg RSA -keysize 4096 -validity 36500
base64 -w0 zeichentool.jks   # Ausgabe als Secret ZT_KEYSTORE_B64
```

Secrets: `ZT_KEYSTORE_B64`, `ZT_KEYSTORE_PASSWORD`, `ZT_KEY_ALIAS` (= `zeichentool`), `ZT_KEY_PASSWORD`.
Die `.jks`-Datei sicher aufbewahren und **nicht** ins Repository legen.

APK lokal bauen (Android SDK + JDK 17 nötig): `./build.sh && cd android && ./gradlew assembleRelease`

## Selbst bauen

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130
./build.sh
python3 -m http.server -d web 8080   # → http://localhost:8080
```

Tests der Engine: `cargo test`

## Aufbau

```
src/engine/   plattformunabhängige Zeichen-Engine (reines Rust, getestet)
  brush.rs      Pinselspitzen und Stempel-Rasterung
  stroke.rs     Glättung, Druck, Neigung, Symmetrie, QuickShape
  doc.rs        Dokument, Ebenen-Compositing, Mischmodi
  fill.rs       Flood-Fill und Polygon-Füllung
  smudge.rs     Wischfinger
  history.rs    Rückgängig/Wiederholen (komprimierte Patches)
  project.rs    .zeichen-Format, PNG-Export
src/web/      Browser-Anbindung (wasm-bindgen / web-sys)
  input.rs      S-Pen, Gesten, Maus, Tastatur
  tools.rs      Werkzeuge
  ui.rs         Oberfläche
  storage.rs    Autospeichern, Dateien, Service Worker
web/          index.html, style.css, Manifest, Icons, Service Worker
android/      Android-Hülle (WebView) – packt web/ in die APK, S-Pen-Events gehen direkt an Rust
```
