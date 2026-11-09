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
    /** Rendert alle Seiten (längste Kante `maxPx`) als JPEG; `progress(i, n)` nach jeder Seite. */
    async render(bytes, maxPx, progress) {
      const lib = await loadPdfJs();
      const doc = await lib.getDocument({ data: bytes.slice() }).promise;
      const out = [];
      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d');
      for (let i = 1; i <= doc.numPages; i++) {
        const page = await doc.getPage(i);
        const vp1 = page.getViewport({ scale: 1 });
        const scale = maxPx / Math.max(vp1.width, vp1.height);
        const vp = page.getViewport({ scale });
        canvas.width = Math.round(vp.width);
        canvas.height = Math.round(vp.height);
        ctx.fillStyle = '#fff';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        await page.render({ canvasContext: ctx, viewport: vp }).promise;
        out.push({ w: vp1.width, h: vp1.height, jpeg: await canvasJpeg(canvas, 0.86) });
        page.cleanup();
        if (progress) progress(i, doc.numPages);
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
