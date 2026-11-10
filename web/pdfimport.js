// PDF-Seiten und Bilder für „PDF importieren“ und „Eigenes Papier“ in JPEG umwandeln.
// pdf.js (Mozilla, Apache-2.0) liegt lokal unter vendor/pdfjs und wird erst bei Bedarf geladen.
(function () {
  let loading = null;
  function loadPdfJs() {
    if (window.pdfjsLib) return Promise.resolve(window.pdfjsLib);
    if (!loading) {
      loading = new Promise((resolve, reject) => {
        const s = document.createElement('script');
        s.src = 'vendor/pdfjs/pdf.min.js';
        s.onload = () => {
          window.pdfjsLib.GlobalWorkerOptions.workerSrc = 'vendor/pdfjs/pdf.worker.min.js';
          resolve(window.pdfjsLib);
        };
        s.onerror = () => { loading = null; reject(new Error('PDF-Modul konnte nicht geladen werden')); };
        document.head.appendChild(s);
      });
    }
    return loading;
  }

  function canvasJpeg(canvas, quality) {
    return new Promise((resolve) => canvas.toBlob((b) => b.arrayBuffer().then((a) => resolve(new Uint8Array(a))), 'image/jpeg', quality));
  }

  window.notionPdf = {
    /** Seitenzahl und Größe (pt) der ersten Seite. */
    async info(bytes) {
      const lib = await loadPdfJs();
      const doc = await lib.getDocument({ data: bytes.slice() }).promise;
      const p = await doc.getPage(1);
      const vp = p.getViewport({ scale: 1 });
      const r = { pages: doc.numPages, w: vp.width, h: vp.height };
      doc.destroy();
      return r;
    },
    /** Rendert alle Seiten (längste Kante `maxPx`) als JPEG; `progress(i, n)` nach jeder Seite.
     *  Mehrere Seiten laufen gleichzeitig: während eine Seite gezeichnet wird, werden andere schon
     *  im Hintergrund zu JPEG kodiert bzw. vom PDF-Worker vorbereitet. */
    async render(bytes, maxPx, progress) {
      const lib = await loadPdfJs();
      const doc = await lib.getDocument({ data: bytes.slice() }).promise;
      const n = doc.numPages;
      const out = new Array(n);
      const cores = navigator.hardwareConcurrency || 4;
      const lanes = Math.max(2, Math.min(4, cores - 1, n));
      let next = 1, done = 0;
      async function lane() {
        const canvas = document.createElement('canvas');
        const ctx = canvas.getContext('2d', { alpha: false });
        while (next <= n) {
          const i = next++;
          const page = await doc.getPage(i);
          const vp1 = page.getViewport({ scale: 1 });
          const scale = maxPx / Math.max(vp1.width, vp1.height);
          const vp = page.getViewport({ scale });
          canvas.width = Math.round(vp.width);
          canvas.height = Math.round(vp.height);
          ctx.fillStyle = '#fff';
          ctx.fillRect(0, 0, canvas.width, canvas.height);
          await page.render({ canvasContext: ctx, viewport: vp, intent: 'print' }).promise;
          out[i - 1] = { w: vp1.width, h: vp1.height, jpeg: await canvasJpeg(canvas, 0.8) };
          page.cleanup();
          done++;
          if (progress) progress(done, n);
        }
      }
      await Promise.all(Array.from({ length: lanes }, lane));
      doc.destroy();
      return out;
    },
    /** Handschrift-PDF (z. B. GoodNotes-Export): Handschrift als Vektoren herauslösen.
     *  Pro Seite: Hintergrund ohne Handschrift als JPEG + Liste der Striche
     *  { c: [r,g,b,a], fill: bool, w: Linienbreite, p: Float32Array (x,y,…; NaN,NaN trennt Teilpfade) }
     *  in Seitenkoordinaten (pt, oben links). */
    async renderInk(bytes, maxPx, progress) {
      const lib = await loadPdfJs();
      const OPS = lib.OPS, Util = lib.Util;
      const doc = await lib.getDocument({ data: bytes.slice() }).promise;
      const n = doc.numPages;
      const out = [];
      const canvas = document.createElement('canvas');
      const real = canvas.getContext('2d', { alpha: false });
      for (let pi = 1; pi <= n; pi++) {
        const page = await doc.getPage(pi);
        const vp1 = page.getViewport({ scale: 1 });
        const pw = vp1.width, ph = vp1.height;
        const ol = await page.getOperatorList();
        // --- Zeichenbefehle durchgehen ---
        let ctm = vp1.transform.slice();
        const stack = [];
        let fill = [0, 0, 0], stroke = [0, 0, 0], fa = 1, sa = 1, lw = 1, pattern = false;
        let path = [], cur = null, curves = 0, group = 0;
        let fillIdx = 0, strokeIdx = 0;
        const skipFill = new Set(), skipStroke = new Set();
        const ink = [];
        const tp = (x, y) => [ctm[0] * x + ctm[2] * y + ctm[4], ctm[1] * x + ctm[3] * y + ctm[5]];
        const rgb = (a) => {
          if (typeof a[0] === 'string') { const h = a[0].replace('#', ''); return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)]; }
          return [a[0], a[1], a[2]].map((v) => (v <= 1 ? Math.round(v * 255) : Math.round(v)));
        };
        const bez = (p0, p1, p2, p3) => {
          const len = Math.hypot(p3[0] - p0[0], p3[1] - p0[1]) + Math.hypot(p1[0] - p0[0], p1[1] - p0[1]) + Math.hypot(p3[0] - p2[0], p3[1] - p2[1]);
          const k = Math.max(1, Math.min(10, Math.ceil(len / 1.2)));
          for (let i = 1; i <= k; i++) {
            const t = i / k, u = 1 - t;
            cur.push([u * u * u * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t * t * t * p3[0], u * u * u * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t * t * t * p3[1]]);
          }
        };
        const scaleOf = () => Math.sqrt(Math.abs(ctm[0] * ctm[3] - ctm[1] * ctm[2]));
        const isInk = () => {
          if (!path.length || pattern || group > 0 || curves === 0) return false;
          let x0 = 1e9, y0 = 1e9, x1 = -1e9, y1 = -1e9;
          for (const sp of path) for (const q of sp) { if (q[0] < x0) x0 = q[0]; if (q[0] > x1) x1 = q[0]; if (q[1] < y0) y0 = q[1]; if (q[1] > y1) y1 = q[1]; }
          return (x1 - x0) < pw * 0.7 && (y1 - y0) < ph * 0.7;
        };
        const flat = () => {
          let len = 0; for (const sp of path) len += sp.length * 2 + 2;
          const a = new Float32Array(len); let o = 0;
          for (const sp of path) { for (const q of sp) { a[o++] = q[0]; a[o++] = q[1]; } a[o++] = NaN; a[o++] = NaN; }
          return a;
        };
        const fn = ol.fnArray, args = ol.argsArray;
        for (let i = 0; i < fn.length; i++) {
          const f = fn[i], a = args[i];
          switch (f) {
            case OPS.save: stack.push([ctm.slice(), fill, stroke, fa, sa, lw, pattern]); break;
            case OPS.restore: { const s = stack.pop(); if (s) [ctm, fill, stroke, fa, sa, lw, pattern] = s; break; }
            case OPS.transform: ctm = Util.transform(ctm, a); break;
            case OPS.paintFormXObjectBegin: stack.push([ctm.slice(), fill, stroke, fa, sa, lw, pattern]); if (a && a[0]) ctm = Util.transform(ctm, a[0]); break;
            case OPS.paintFormXObjectEnd: { const s = stack.pop(); if (s) [ctm, fill, stroke, fa, sa, lw, pattern] = s; break; }
            case OPS.beginGroup: group++; break;
            case OPS.endGroup: group = Math.max(0, group - 1); break;
            case OPS.setLineWidth: lw = a[0]; break;
            case OPS.setFillRGBColor: fill = rgb(a); pattern = false; break;
            case OPS.setStrokeRGBColor: stroke = rgb(a); break;
            case OPS.setFillGray: fill = rgb([a[0], a[0], a[0]]); pattern = false; break;
            case OPS.setStrokeGray: stroke = rgb([a[0], a[0], a[0]]); break;
            case OPS.setFillColorN: pattern = Array.isArray(a) && (a[0] === 'TilingPattern' || a[0] === 'Shading'); break;
            case OPS.setGState:
              for (const kv of a[0] || []) { if (kv[0] === 'ca') fa = kv[1]; if (kv[0] === 'CA') sa = kv[1]; }
              break;
            case OPS.constructPath: {
              const ops = a[0], co = a[1];
              let j = 0;
              for (const op of ops) {
                if (op === OPS.moveTo) { cur = [tp(co[j], co[j + 1])]; path.push(cur); j += 2; }
                else if (op === OPS.lineTo) { if (!cur) { cur = []; path.push(cur); } cur.push(tp(co[j], co[j + 1])); j += 2; }
                else if (op === OPS.curveTo) { if (!cur) { cur = [tp(co[j + 4], co[j + 5])]; path.push(cur); } bez(cur[cur.length - 1], tp(co[j], co[j + 1]), tp(co[j + 2], co[j + 3]), tp(co[j + 4], co[j + 5])); curves++; j += 6; }
                else if (op === OPS.curveTo2) { if (!cur) { cur = [tp(co[j + 2], co[j + 3])]; path.push(cur); } const p0 = cur[cur.length - 1]; bez(p0, p0, tp(co[j], co[j + 1]), tp(co[j + 2], co[j + 3])); curves++; j += 4; }
                else if (op === OPS.curveTo3) { if (!cur) { cur = [tp(co[j + 2], co[j + 3])]; path.push(cur); } const e = tp(co[j + 2], co[j + 3]); bez(cur[cur.length - 1], tp(co[j], co[j + 1]), e, e); curves++; j += 4; }
                else if (op === OPS.rectangle) {
                  const x = co[j], y = co[j + 1], w = co[j + 2], h = co[j + 3];
                  cur = [tp(x, y), tp(x + w, y), tp(x + w, y + h), tp(x, y + h)]; path.push(cur); cur = null; j += 4;
                } else if (op === OPS.closePath) { if (cur && cur.length) cur.push(cur[0].slice()); cur = null; }
              }
              break;
            }
            case OPS.fill: case OPS.eoFill: case OPS.fillStroke: case OPS.eoFillStroke: case OPS.closeFillStroke: case OPS.closeEOFillStroke: {
              const main = group === 0;
              if (isInk()) {
                ink.push({ c: [...fill, Math.round(fa * 255)], fill: true, w: 0, p: flat() });
                if (main) skipFill.add(fillIdx);
              }
              if (main) fillIdx++;
              if (f >= OPS.fillStroke) { if (main) strokeIdx++; }
              path = []; cur = null; curves = 0;
              break;
            }
            case OPS.stroke: case OPS.closeStroke: {
              const main = group === 0;
              let x0 = 1e9, y0 = 1e9, x1 = -1e9, y1 = -1e9;
              for (const sp of path) for (const q of sp) { if (q[0] < x0) x0 = q[0]; if (q[0] > x1) x1 = q[0]; if (q[1] < y0) y0 = q[1]; if (q[1] > y1) y1 = q[1]; }
              if (path.length && group === 0 && (x1 - x0) < pw * 0.7 && (y1 - y0) < ph * 0.7) {
                ink.push({ c: [...stroke, Math.round(sa * 255)], fill: false, w: Math.max(0.3, lw * scaleOf()), p: flat() });
                if (main) skipStroke.add(strokeIdx);
              }
              if (main) strokeIdx++;
              path = []; cur = null; curves = 0;
              break;
            }
            case OPS.endPath: path = []; cur = null; curves = 0; break;
            default: break;
          }
        }
        // --- Hintergrund ohne Handschrift zeichnen ---
        const scale = maxPx / Math.max(pw, ph);
        const vp = page.getViewport({ scale });
        canvas.width = Math.round(vp.width);
        canvas.height = Math.round(vp.height);
        real.fillStyle = '#fff';
        real.fillRect(0, 0, canvas.width, canvas.height);
        let nf = 0, ns = 0;
        const proxy = new Proxy(real, {
          get(t, p) {
            if (p === 'fill') return function (...x) { const i = nf++; if (skipFill.has(i)) { t.beginPath(); return; } return t.fill(...x); };
            if (p === 'stroke') return function (...x) { const i = ns++; if (skipStroke.has(i)) { t.beginPath(); return; } return t.stroke(...x); };
            const v = t[p];
            return typeof v === 'function' ? v.bind(t) : v;
          },
          set(t, p, v) { t[p] = v; return true; },
        });
        await page.render({ canvasContext: proxy, viewport: vp }).promise;
        let inkOk = nf === fillIdx && ns === strokeIdx;
        if (!inkOk) {
          // Zählung passt nicht (ungewöhnliches PDF) → Seite normal als Bild, ohne Handschrift-Übernahme
          real.fillStyle = '#fff';
          real.fillRect(0, 0, canvas.width, canvas.height);
          await page.render({ canvasContext: real, viewport: vp }).promise;
        }
        out.push({ w: pw, h: ph, jpeg: await canvasJpeg(canvas, 0.85), ink: inkOk ? ink : [], ok: inkOk });
        page.cleanup();
        if (progress) progress(pi, n);
      }
      doc.destroy();
      return out;
    },
    /** Bilddatei → JPEG (längste Kante höchstens `maxPx`), für eigenes Papier. */
    async image(file, maxPx) {
      const url = URL.createObjectURL(file);
      try {
        const img = new Image();
        img.src = url;
        await img.decode();
        const f = Math.min(1, maxPx / Math.max(img.naturalWidth, img.naturalHeight));
        const c = document.createElement('canvas');
        c.width = Math.max(1, Math.round(img.naturalWidth * f));
        c.height = Math.max(1, Math.round(img.naturalHeight * f));
        const g = c.getContext('2d');
        g.fillStyle = '#fff';
        g.fillRect(0, 0, c.width, c.height);
        g.drawImage(img, 0, 0, c.width, c.height);
        return { w: c.width, h: c.height, jpeg: await canvasJpeg(c, 0.9) };
      } finally {
        URL.revokeObjectURL(url);
      }
    },
  };
})();

// Teilen im Browser (Web Share API mit Dateien); liefert true, wenn geteilt wurde.
window.notionShare = async function (b64, mime, name) {
  try {
    const bin = atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    const file = new File([bytes], name, { type: mime });
    if (navigator.canShare && navigator.canShare({ files: [file] })) {
      await navigator.share({ files: [file], title: name });
      return true;
    }
  } catch (e) {
    if (e && e.name === 'AbortError') return true;
  }
  return false;
};

// Zeitraffer-Video: Canvas Bild für Bild aufnehmen (MP4, falls möglich, sonst WebM).
window.notionRec = {
  start(canvas, fps) {
    if (!canvas.captureStream || typeof MediaRecorder === 'undefined') return null;
    const types = ['video/mp4;codecs=avc1', 'video/mp4', 'video/webm;codecs=vp9', 'video/webm;codecs=vp8', 'video/webm'];
    const mime = types.find((t) => MediaRecorder.isTypeSupported && MediaRecorder.isTypeSupported(t));
    if (!mime) return null;
    const stream = canvas.captureStream(0);
    const track = stream.getVideoTracks()[0];
    const rec = new MediaRecorder(stream, { mimeType: mime, videoBitsPerSecond: 6000000 });
    const chunks = [];
    rec.ondataavailable = (e) => { if (e.data && e.data.size) chunks.push(e.data); };
    rec.start(500);
    return { rec, track, chunks, mime, fps };
  },
  frame(h) {
    if (h && h.track && h.track.requestFrame) h.track.requestFrame();
  },
  stop(h) {
    return new Promise((resolve) => {
      h.rec.onstop = async () => {
        const blob = new Blob(h.chunks, { type: h.mime.split(';')[0] });
        resolve({ bytes: new Uint8Array(await blob.arrayBuffer()), mime: h.mime.split(';')[0] });
      };
      h.rec.stop();
    });
  },
};
