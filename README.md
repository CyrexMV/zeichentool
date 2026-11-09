# Notion

Eine moderne, GoodNotes-artige Notiz-App – **komplett in Rust geschrieben** (WebAssembly)
und als **Android-APK** sowie installierbare Web-App verfügbar. Optimiert für das
**Galaxy Tab S12 Ultra mit S-Pen**.

👉 Web-App: **https://cyrexmv.github.io/zeichentool/** · APK: **https://cyrexmv.github.io/zeichentool/zeichentool.apk**

## Funktionen

**Bibliothek**
- **Ordner**: anlegen, umbenennen, auflösen; Notizbücher per ⋯ → „In Ordner verschieben“; die Suche findet über alle Ordner
- **Notizbuchvorlagen**: Notizbuch als Vorlage speichern (⋯ → „Als Vorlage speichern“) und neue Notizbücher daraus erstellen
- Notizbücher mit farbigen Buch-Covern, Suche, Sortierung (zuletzt bearbeitet / Name / erstellt), Favoriten
- Umbenennen, Cover ändern, Duplizieren, Löschen, Export – alles über das ⋯-Menü der Karte
- Import von Backups (`.znote`), Bildern und alten Zeichnungen; die bisherige Zeichnung wird automatisch übernommen

**Notizbuch**
- Mehrere Seiten, vertikal durchscrollen (mit Schwung), Seitenübersicht mit Vorschaubildern, Lesezeichen
- Seiten hinzufügen, duplizieren, löschen, verschieben; Vorlage pro Seite oder für alle Seiten ändern
- Vorlagen: Blanko, Liniert, Kariert, Gepunktet, Cornell, Notenlinien, Wochenplaner, **Checkliste** – eng/mittel/weit
- Farbige Planer mit fertigen Beschriftungen: **Woche farbig** (Mo–So + Notizen), **Einkaufsliste** (nach Kategorien, abhakbar),
  **Tagesplaner** (Zeitplan, Top 3, To-do, Wasser), **Monatskalender**, **Stundenplan** (mit Hausaufgaben), **Essensplan**
  (mit Einkaufsliste), **Gewohnheiten** (31 Tage) und **Lernkarte** – passen sich an Hoch-/Querformat an
- **Checkliste**: Kästchen antippen → abgehakt; die Schrift in der Zeile wird automatisch durchgestrichen und ausgegraut
- Papierfarben: Weiß, Creme, Notizgelb, Hellblau, Dunkel, Schwarz, Tafelgrün
- Formate: A4 hoch/quer, Quadrat, Bildschirm

**Vektor-Tinte** (gestochen scharf bei jedem Zoom)
- Stifte: Füller, Kugelschreiber, Pinselstift, Bleistift – mit Druck, Stabilisierung, sanfter Verjüngung
- Textmarker (liegt unter der Schrift, optional gerade Linien)
- Radierer: präzise (schneidet Striche) oder ganze Striche, „nur Textmarker“, automatisch zurück zum Stift
- Je drei Favoriten-Farben und -Stärken direkt in der Werkzeugleiste (antippen = wechseln, nochmal = bearbeiten)
- **Formen**: Werkzeug oder Stift am Strichende halten → Linie, Kreis, Ellipse, Dreieck, Rechteck, Vieleck
- **Lasso**: einkreisen → verschieben, skalieren, umfärben, kopieren, ausschneiden, duplizieren, löschen, einfügen
- **Text**-Felder (Größe, Farbe, fett), **Bilder** einfügen, **Laserpointer** zum Präsentieren
- Rückgängig/Wiederholen (auch Zwei-/Drei-Finger-Tipp)
- **Lineal & Geodreieck**: mit einem Finger verschieben, mit zwei drehen (rastet bei 0°/45°/90° ein); am Rand
  entlang zeichnet der Stift exakt gerade Linien, mit cm-Skala, Winkelmesser und Längenanzeige
- **Lernkarten auf dem Startbildschirm**: „Neues Lernkarten-Set“ legt ein Kartenset an (jede Seite = Karte, oben Frage,
  unten Antwort); „Lernen“ startet direkt vom Startbildschirm, mit Fortschrittsbalken je Set
- **Lernkarten**: Frage und Antwort mit dem Lasso markieren → „Lernkarte“; lernen mit Umdrehen und
  Karteikasten (gewusst → nächstes Fach, sonst zurück); Karten zeigen immer den aktuellen Stand der Seite
- **Inhaltsverzeichnis**: Seitentitel vergeben (⋯ → „Seitentitel“), Übersicht unter Seiten → „Inhalt“
- **Seiten in andere Notizbücher** kopieren oder verschieben (⋯ → „Seite in anderes Notizbuch“)

**S-Pen**
- Druckempfindlichkeit, Pinselvorschau beim Schweben, niedrige Latenz (Zwischen- und Vorhersagepunkte)
- **S-Pen-Taste**: Radierer ↔ Stift (oder halten = radieren, oder Rückgängig)
- **Handballen-Filter** direkt in der Android-App: Solange der Stift in der Nähe ist (und einstellbar lange
  danach), erreichen Hand und Finger die Seite gar nicht; eine schon verschobene Ansicht springt zurück

**Export & Speichern**
- Automatisches Speichern (IndexedDB), offline nutzbar
- **PDF** (Vektor, Text auswählbar) für das ganze Notizbuch oder eine Seite, **PNG** einer Seite, **Backup** (`.znote`)
- In der APK landen Exporte direkt in *Download/Zeichentool* bzw. Bilder in *Bilder/Zeichentool*

**Design**: helles und dunkles Farbschema (oder automatisch nach System), Glas-Leisten, Hoch- und Querformat.

## Installation auf dem Tablet

**APK:** Link oben öffnen → Installieren (beim ersten Mal „Unbekannte Apps installieren“ erlauben).
**Web-App:** Seite in Samsung Internet/Chrome öffnen → „Zum Startbildschirm hinzufügen“.

**Signatur (wichtig für Updates):** Android installiert ein Update nur, wenn es mit demselben
Schlüssel signiert ist. Einmalig einrichten:

1. GitHub → Repository → *Settings → Secrets and variables → Actions → New repository secret*
2. Name: `ZT_SIGNING_PASSWORD`, Wert: ein langes, eigenes Passwort (mindestens 12, besser 20+ Zeichen) –
   im Passwort-Manager aufbewahren und **nie ändern**.
3. Beim nächsten Build erzeugt GitHub den Schlüssel und legt ihn **nur verschlüsselt** (AES-256) im Branch
   `signing` ab. Ab dann ist jede APK gleich signiert – Updates einfach über die alte App installieren.

Ohne dieses Secret wird mit einem temporären Schlüssel signiert (dann vor jedem Update neu installieren).
Alternativ kann ein eigener Keystore als `ZT_KEYSTORE_B64` (+ `ZT_KEYSTORE_PASSWORD`, `ZT_KEY_ALIAS`,
`ZT_KEY_PASSWORD`) hinterlegt werden.

## Selbst bauen

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130
./build.sh                                   # Web-App nach web/
python3 -m http.server -d web 8080           # → http://localhost:8080
cd android && ./gradlew assembleRelease      # APK (Android SDK + JDK 17)
cargo test                                   # Tests der Engine
```

Jeder Push baut über `.github/workflows/pages.yml` Web-App und APK und veröffentlicht beides auf GitHub Pages.

## Aufbau

```
src/engine/   Notiz-Engine (reines Rust, getestet)
  ink.rs        Vektor-Striche: Umriss mit variabler Breite, Treffertests, Präzisionsradierer
  shapes.rs     Formerkennung (QuickShape)
  note.rs       Notizbuch → Seiten → Elemente (Strich, Text, Bild)
  paper.rs      Vorlagen als Vektor-Grundformen
  history.rs    Rückgängig/Wiederholen
  format.rs     .znote-Format, Bibliothek, PNG, Import alter Zeichnungen
  pdf.rs        Vektor-PDF-Export
src/web/      Browser-Anbindung (wasm-bindgen / web-sys)
  render.rs     Canvas-Rendering mit Seiten-Zwischenspeicher je Zoomstufe
  input.rs      S-Pen, Handballen-Sperre, Gesten mit Schwung, Tastatur
  tools.rs      Werkzeuge, Lasso, Text, Bilder
  ui.rs         Werkzeugleiste, Optionen, Seitenübersicht, Dialoge
  library.rs    Bibliothek
  storage.rs    IndexedDB, Export/Import, Android-Brücke
web/          index.html, style.css, Manifest, Icons, Service Worker
android/      Android-Hülle (WebView) mit nativem S-Pen-/Handballen-Filter
```
