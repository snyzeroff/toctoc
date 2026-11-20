// Code partagé par la fenêtre principale et la fenêtre rapide :
// affichage des messages, zone d'écriture, images, dessin.
window.Toc = (() => {
  const { invoke, convertFileSrc } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;

  // ───────── icônes ─────────
  const P = {
    send: '<path d="M4 12l16-8-6 16-2.5-6.5z"/><path d="M11.5 13.5L20 4"/>',
    image: '<rect x="3" y="4" width="18" height="16" rx="3"/><circle cx="9" cy="10" r="1.8"/><path d="M21 16l-5-5-9 9"/>',
    draw: '<path d="M4 20l4.5-1 10-10a2.1 2.1 0 00-3-3l-10 10z"/><path d="M13.5 6.5l3 3"/>',
    ping: '<circle cx="12" cy="12" r="2.4" fill="currentColor" stroke="none"/><path d="M7.8 7.8a6 6 0 000 8.4M16.2 7.8a6 6 0 010 8.4M5 5a10 10 0 000 14M19 5a10 10 0 010 14"/>',
    gear: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 00.3 1.8l.1.1a2 2 0 11-2.8 2.8l-.1-.1a1.7 1.7 0 00-1.8-.3 1.7 1.7 0 00-1 1.5V21a2 2 0 11-4 0v-.1a1.7 1.7 0 00-1.1-1.5 1.7 1.7 0 00-1.8.3l-.1.1a2 2 0 11-2.8-2.8l.1-.1a1.7 1.7 0 00.3-1.8 1.7 1.7 0 00-1.5-1H3a2 2 0 110-4h.1a1.7 1.7 0 001.5-1.1 1.7 1.7 0 00-.3-1.8l-.1-.1a2 2 0 112.8-2.8l.1.1a1.7 1.7 0 001.8.3H9a1.7 1.7 0 001-1.5V3a2 2 0 114 0v.1a1.7 1.7 0 001 1.5 1.7 1.7 0 001.8-.3l.1-.1a2 2 0 112.8 2.8l-.1.1a1.7 1.7 0 00-.3 1.8V9a1.7 1.7 0 001.5 1H21a2 2 0 110 4h-.1a1.7 1.7 0 00-1.5 1z"/>',
    expand: '<path d="M14 4h6v6M10 20H4v-6M20 4l-7 7M4 20l7-7"/>',
    close: '<path d="M6 6l12 12M18 6L6 18"/>',
    undo: '<path d="M9 14L4 9l5-5"/><path d="M4 9h11a5 5 0 010 10h-3"/>',
    eraser: '<path d="M7 21h13"/><path d="M5.5 15.5l9-9a2 2 0 012.8 0l2.2 2.2a2 2 0 010 2.8L12 19H8.5z"/>',
    trash: '<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>',
    bold: '<path d="M7 5h6a3.5 3.5 0 010 7H7zM7 12h7a3.5 3.5 0 010 7H7z"/>',
    italic: '<path d="M10 5h8M6 19h8M14 5l-4 14"/>',
    code: '<path d="M9 8l-4 4 4 4M15 8l4 4-4 4"/>',
    list: '<path d="M9 6h11M9 12h11M9 18h11"/><circle cx="4.5" cy="6" r="1" fill="currentColor"/><circle cx="4.5" cy="12" r="1" fill="currentColor"/><circle cx="4.5" cy="18" r="1" fill="currentColor"/>',
    link: '<path d="M10 14a4.5 4.5 0 006.4 0l3-3a4.5 4.5 0 00-6.4-6.4l-1 1"/><path d="M14 10a4.5 4.5 0 00-6.4 0l-3 3a4.5 4.5 0 006.4 6.4l1-1"/>',
  };
  const icon = (name) =>
    `<svg class="ico" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${P[name]}</svg>`;

  // ───────── texte mis en forme (markdown), nettoyé ─────────
  marked.setOptions({ gfm: true, breaks: true });
  function md(text) {
    const html = marked.parse(text || "");
    return DOMPurify.sanitize(html, {
      FORBID_TAGS: ["img", "style", "iframe", "form", "input", "button", "video", "audio", "svg", "math"],
      FORBID_ATTR: ["style", "class", "id"],
    });
  }
  // les liens s'ouvrent dans le navigateur, jamais dans la fenêtre
  document.addEventListener("click", (e) => {
    const a = e.target.closest("a[href]");
    if (!a) return;
    e.preventDefault();
    invoke("open_link", { url: a.href }).catch(() => {});
  });

  const mediaUrl = (name) => convertFileSrc(name, "tocmedia");

  // ───────── petits utilitaires ─────────
  let toastTimer;
  function toast(text) {
    let t = document.getElementById("toc-toast");
    if (!t) {
      t = document.createElement("div");
      t.id = "toc-toast";
      t.className = "toast";
      document.body.append(t);
    }
    t.textContent = text;
    t.hidden = false;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (t.hidden = true), 3200);
  }

  const time = (ts) => new Date(ts).toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
  function dayLabel(ts) {
    const d = new Date(ts);
    const today = new Date();
    const yesterday = new Date(Date.now() - 864e5);
    if (d.toDateString() === today.toDateString()) return "Aujourd'hui";
    if (d.toDateString() === yesterday.toDateString()) return "Hier";
    const opts = { weekday: "long", day: "numeric", month: "long" };
    if (d.getFullYear() !== today.getFullYear()) opts.year = "numeric";
    return d.toLocaleDateString("fr-FR", opts);
  }

  function blobToB64(blob) {
    return new Promise((ok, ko) => {
      const r = new FileReader();
      r.onload = () => ok(String(r.result).split(",")[1]);
      r.onerror = ko;
      r.readAsDataURL(blob);
    });
  }
  function loadImage(src) {
    return new Promise((ok, ko) => {
      const img = new Image();
      img.onload = () => ok(img);
      img.onerror = () => ko(new Error("Image illisible."));
      img.src = src;
    });
  }
  const canvasBlob = (canvas, type, q) => new Promise((ok) => canvas.toBlob(ok, type, q));

  /** Prépare une image avant l'envoi : réduite à 1600 px, compressée. */
  async function prepareImage(blob) {
    if (!blob.type.startsWith("image/")) throw new Error("Ce fichier n'est pas une image.");
    if (blob.type === "image/gif" && blob.size <= 6e6) return { blob, mime: "image/gif" };
    const url = URL.createObjectURL(blob);
    try {
      const img = await loadImage(url);
      const scale = Math.min(1, 1600 / Math.max(img.naturalWidth, img.naturalHeight));
      if (blob.type === "image/png" && scale === 1 && blob.size < 1.5e6) return { blob, mime: "image/png" };
      const c = document.createElement("canvas");
      c.width = Math.max(1, Math.round(img.naturalWidth * scale));
      c.height = Math.max(1, Math.round(img.naturalHeight * scale));
      const g = c.getContext("2d");
      g.fillStyle = "#fff";
      g.fillRect(0, 0, c.width, c.height);
      g.drawImage(img, 0, 0, c.width, c.height);
      const out = await canvasBlob(c, "image/jpeg", 0.85);
      if (out.size > 7.5e6) throw new Error("Image trop lourde.");
      return { blob: out, mime: "image/jpeg" };
    } finally {
      URL.revokeObjectURL(url);
    }
  }

  // ───────── la conversation ─────────
  class Conversation {
    constructor(el, { pageSize = 60, onImage, older = true } = {}) {
      this.el = el;
      this.pageSize = pageSize;
      this.onImage = onImage;
      this.msgs = [];
      this.full = !older;
      this.loading = false;
      el.classList.add("conv");
      if (older) el.addEventListener("scroll", () => el.scrollTop < 80 && this.loadOlder());
      listen("history", (e) => {
        const { op, msg } = e.payload;
        if (op === "add") this.add(msg);
        else if (op === "update") this.update(msg);
        else if (op === "clear") {
          this.msgs = [];
          this.render();
        }
      });
    }
    nearBottom() {
      const el = this.el;
      return el.scrollHeight - el.scrollTop - el.clientHeight < 120;
    }
    toBottom() {
      this.el.scrollTop = this.el.scrollHeight;
      // les images se chargent après coup
      requestAnimationFrame(() => (this.el.scrollTop = this.el.scrollHeight));
    }
    async load() {
      this.msgs = await invoke("history", { limit: this.pageSize });
      this.full = this.full || this.msgs.length < this.pageSize;
      this.render();
      this.toBottom();
    }
    async loadOlder() {
      if (this.loading || this.full || !this.msgs.length) return;
      this.loading = true;
      const older = await invoke("history", { before: this.msgs[0].id, limit: this.pageSize });
      if (older.length < this.pageSize) this.full = true;
      if (older.length) {
        const h = this.el.scrollHeight;
        this.msgs = older.concat(this.msgs);
        this.render();
        this.el.scrollTop = this.el.scrollHeight - h;
      }
      this.loading = false;
    }
    render() {
      this.el.innerHTML = "";
      if (!this.msgs.length) {
        this.el.innerHTML = '<div class="empty">Aucun message pour l\'instant.<br>Dites bonjour !</div>';
        return;
      }
      let prev = null;
      for (const m of this.msgs) {
        this.appendEl(m, prev);
        prev = m;
      }
    }
    appendEl(m, prev) {
      if (!prev || new Date(prev.ts).toDateString() !== new Date(m.ts).toDateString()) {
        const sep = document.createElement("div");
        sep.className = "day";
        sep.textContent = dayLabel(m.ts);
        this.el.append(sep);
      }
      const node = this.node(m);
      if (prev && prev.out === m.out && m.ts - prev.ts < 120000) node.classList.add("follow");
      this.el.append(node);
    }
    add(m) {
      if (this.msgs.some((x) => x.id === m.id)) return;
      const stick = m.out || this.nearBottom();
      if (!this.msgs.length) this.el.innerHTML = "";
      this.appendEl(m, this.msgs[this.msgs.length - 1]);
      this.msgs.push(m);
      if (stick) this.toBottom();
    }
    update(m) {
      const i = this.msgs.findIndex((x) => x.id === m.id);
      if (i < 0) return;
      this.msgs[i] = m;
      const old = this.el.querySelector(`[data-id="${m.id}"] .meta`);
      if (old) old.replaceWith(this.meta(m));
    }
    meta(m) {
      const d = document.createElement("div");
      d.className = "meta";
      let s = time(m.ts);
      if (m.out) {
        if (m.status === "sending") s += " · envoi…";
        else if (m.status === "sent") s += " · ✓";
        else if (m.status === "delivered") s += " · ✓✓ reçu";
      }
      d.textContent = s;
      if (m.out && m.status === "failed") {
        d.classList.add("failed");
        d.textContent = time(m.ts) + " · non envoyé · ";
        const b = document.createElement("button");
        b.className = "retry";
        b.textContent = "Réessayer";
        b.onclick = () => invoke("retry", { id: m.id });
        d.append(b);
      }
      return d;
    }
    node(m) {
      const row = document.createElement("div");
      row.className = "msg " + (m.out ? "out" : "in");
      row.dataset.id = m.id;
      const bubble = document.createElement("div");
      bubble.className = "bubble";
      if (m.kind === "ping") {
        bubble.classList.add("ping");
        bubble.innerHTML = icon("ping") + "<span>Toc toc</span>";
      } else {
        if (m.image) {
          const img = document.createElement("img");
          img.className = "photo";
          img.loading = "lazy";
          img.alt = m.drawing ? "Dessin" : "Image";
          img.src = mediaUrl(m.image);
          img.onclick = () => this.onImage && this.onImage(m);
          img.onload = () => this.nearBottom() && this.toBottom();
          bubble.append(img);
          bubble.classList.add("has-photo");
        }
        if (m.text) {
          const t = document.createElement("div");
          t.className = "md";
          t.innerHTML = md(m.text);
          bubble.append(t);
        } else bubble.classList.add("only-photo");
      }
      row.append(bubble, this.meta(m));
      return row;
    }
  }

  // ───────── la zone d'écriture ─────────
  class Composer {
    constructor(root, { onDraw, toolbar = false } = {}) {
      this.root = root;
      this.onDraw = onDraw;
      this.pending = null;
      root.classList.add("composer");
      root.innerHTML = `
        ${toolbar ? `<div class="fmt">
          <button data-act="bold" title="Gras (Ctrl+B)">${icon("bold")}</button>
          <button data-act="italic" title="Italique (Ctrl+I)">${icon("italic")}</button>
          <button data-act="code" title="Code">${icon("code")}</button>
          <button data-act="list" title="Liste">${icon("list")}</button>
          <button data-act="link" title="Lien (Ctrl+K)">${icon("link")}</button>
        </div>` : ""}
        <div class="attach" hidden><img alt=""><span></span><button data-act="unattach" title="Retirer">${icon("close")}</button></div>
        <div class="field">
          <textarea rows="1" placeholder="Message" maxlength="4000"></textarea>
        </div>
        <div class="actions">
          <button data-act="ping" class="tool" title="Envoyer un ping">${icon("ping")}</button>
          <button data-act="image" class="tool" title="Joindre une image">${icon("image")}</button>
          <button data-act="draw" class="tool" title="Dessiner">${icon("draw")}</button>
          <span class="grow"></span>
          <button data-act="send" class="send" title="Envoyer (Entrée)">${icon("send")}</button>
        </div>
        <input type="file" accept="image/*" hidden>`;
      this.ta = root.querySelector("textarea");
      this.file = root.querySelector("input[type=file]");
      this.ta.addEventListener("input", () => this.grow());
      this.ta.addEventListener("keydown", (e) => this.key(e));
      this.ta.addEventListener("paste", (e) => {
        const f = [...(e.clipboardData?.files || [])].find((f) => f.type.startsWith("image/"));
        if (f) {
          e.preventDefault();
          this.attach(f);
        }
      });
      root.addEventListener("click", (e) => {
        const b = e.target.closest("button[data-act]");
        if (b) this.act(b.dataset.act);
      });
      this.file.onchange = () => {
        if (this.file.files[0]) this.attach(this.file.files[0]);
        this.file.value = "";
        window.dispatchEvent(new Event("toc-picker-done"));
      };
      // glisser-déposer une image n'importe où dans la fenêtre
      window.addEventListener("dragover", (e) => e.preventDefault());
      window.addEventListener("drop", (e) => {
        e.preventDefault();
        const f = [...(e.dataTransfer?.files || [])].find((f) => f.type.startsWith("image/"));
        if (f) this.attach(f);
      });
    }
    focus() {
      this.ta.focus();
    }
    grow() {
      this.ta.style.height = "auto";
      this.ta.style.height = Math.min(this.ta.scrollHeight, 160) + "px";
    }
    key(e) {
      const mod = e.ctrlKey || e.metaKey;
      if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
        e.preventDefault();
        this.send();
      } else if (mod && e.key.toLowerCase() === "b") {
        e.preventDefault();
        this.wrap("**", "**");
      } else if (mod && e.key.toLowerCase() === "i") {
        e.preventDefault();
        this.wrap("*", "*");
      } else if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        this.act("link");
      }
    }
    wrap(before, after, placeholder = "texte") {
      const ta = this.ta;
      const [s, e] = [ta.selectionStart, ta.selectionEnd];
      const sel = ta.value.slice(s, e) || placeholder;
      ta.setRangeText(before + sel + after, s, e, "end");
      ta.setSelectionRange(s + before.length, s + before.length + sel.length);
      ta.focus();
      this.grow();
    }
    act(a) {
      if (a === "send") this.send();
      else if (a === "ping") invoke("ping");
      else if (a === "image") {
        window.dispatchEvent(new Event("toc-picker-open"));
        this.file.click();
      } else if (a === "draw") this.onDraw && this.onDraw();
      else if (a === "unattach") this.attach(null);
      else if (a === "bold") this.wrap("**", "**");
      else if (a === "italic") this.wrap("*", "*");
      else if (a === "code") {
        const sel = this.ta.value.slice(this.ta.selectionStart, this.ta.selectionEnd);
        sel.includes("\n") ? this.wrap("```\n", "\n```", "code") : this.wrap("`", "`", "code");
      } else if (a === "list") {
        const ta = this.ta;
        const start = ta.value.lastIndexOf("\n", ta.selectionStart - 1) + 1;
        ta.setRangeText("- ", start, start, "end");
        ta.focus();
      } else if (a === "link") {
        const sel = this.ta.value.slice(this.ta.selectionStart, this.ta.selectionEnd);
        if (/^https?:\/\//.test(sel)) this.wrap("[", `](${sel})`, "lien");
        else this.wrap("[", "](https://)", "texte du lien");
      }
    }
    /** Joint une image (fichier, presse-papiers, dessin) ; `null` retire. */
    async attach(blob, drawing = false) {
      const box = this.root.querySelector(".attach");
      if (this.pending?.url) URL.revokeObjectURL(this.pending.url);
      this.pending = null;
      box.hidden = true;
      if (!blob) return;
      try {
        const p = await prepareImage(blob);
        p.drawing = drawing;
        p.url = URL.createObjectURL(p.blob);
        this.pending = p;
        box.querySelector("img").src = p.url;
        box.querySelector("span").textContent = drawing ? "Dessin" : `Image · ${Math.max(1, Math.round(p.blob.size / 1024))} Ko`;
        box.hidden = false;
        this.ta.focus();
      } catch (err) {
        toast(err.message || String(err));
      }
    }
    async send() {
      const text = this.ta.value.trim();
      const p = this.pending;
      if (!text && !p) return;
      const btn = this.root.querySelector(".send");
      btn.disabled = true;
      try {
        await invoke("send_message", {
          text,
          image: p ? await blobToB64(p.blob) : null,
          mime: p ? p.mime : null,
          drawing: !!p?.drawing,
        });
        this.ta.value = "";
        this.grow();
        this.attach(null);
      } catch (err) {
        toast(String(err));
      } finally {
        btn.disabled = false;
        this.ta.focus();
      }
    }
  }

  // ───────── la planche à dessin ─────────
  const COLORS = ["#1d1d1f", "#e5484d", "#f5a524", "#30b55c", "#3f8bff", "#8e5cff", "#ff6fb1", "#ffffff"];
  const SIZES = [3, 7, 16];

  /** Ouvre la planche. `background` : adresse d'une image à annoter. Renvoie un Blob ou null. */
  function draw({ background = null } = {}) {
    return new Promise(async (done) => {
      let bg = null;
      if (background) {
        try {
          bg = await loadImage(background);
        } catch {
          bg = null;
        }
      }
      const W = bg ? Math.round(bg.naturalWidth * Math.min(1, 1600 / Math.max(bg.naturalWidth, bg.naturalHeight))) : 1200;
      const H = bg ? Math.round(bg.naturalHeight * (W / bg.naturalWidth)) : 800;
      const ov = document.createElement("div");
      ov.className = "draw-overlay";
      ov.innerHTML = `
        <div class="draw-panel">
          <div class="draw-top">
            <button class="link" data-act="cancel">Annuler</button>
            <strong>${bg ? "Annoter l'image" : "Dessin"}</strong>
            <button class="pill-btn" data-act="send">Envoyer</button>
          </div>
          <div class="draw-stage"><canvas></canvas></div>
          <div class="draw-tools">
            <div class="swatches">${COLORS.map((c, i) => `<button data-color="${c}" style="--c:${c}" class="${i ? "" : "on"}" title="Couleur"></button>`).join("")}</div>
            <div class="sizes">${SIZES.map((s, i) => `<button data-size="${s}" class="${i === 1 ? "on" : ""}" title="Épaisseur"><i style="--s:${4 + i * 5}px"></i></button>`).join("")}</div>
            <div class="draw-btns">
              <button data-act="eraser" title="Gomme">${icon("eraser")}</button>
              <button data-act="undo" title="Annuler le dernier trait (Ctrl+Z)">${icon("undo")}</button>
              <button data-act="clear" title="Tout effacer">${icon("trash")}</button>
            </div>
          </div>
        </div>`;
      document.body.append(ov);
      const canvas = ov.querySelector("canvas");
      canvas.width = W;
      canvas.height = H;
      const view = canvas.getContext("2d");
      const layer = document.createElement("canvas");
      layer.width = W;
      layer.height = H;
      const lg = layer.getContext("2d");
      lg.lineCap = "round";
      lg.lineJoin = "round";
      const unit = W / 1200;
      let color = COLORS[0];
      let size = SIZES[1];
      let eraser = false;
      const strokes = [];
      let cur = null;

      function compose() {
        view.fillStyle = "#fff";
        view.fillRect(0, 0, W, H);
        if (bg) view.drawImage(bg, 0, 0, W, H);
        view.drawImage(layer, 0, 0);
      }
      function fit() {
        const st = ov.querySelector(".draw-stage");
        // on rétrécit d'abord la toile pour mesurer la place vraiment disponible
        canvas.style.width = canvas.style.height = "0px";
        const r = Math.min((st.clientWidth - 20) / W, (st.clientHeight - 20) / H);
        canvas.style.width = W * r + "px";
        canvas.style.height = H * r + "px";
      }
      function seg(s, i) {
        const p = s.pts;
        lg.globalCompositeOperation = s.eraser ? "destination-out" : "source-over";
        lg.strokeStyle = s.color;
        lg.fillStyle = s.color;
        lg.lineWidth = s.size * unit;
        if (p.length === 1 || i === 0) {
          lg.beginPath();
          lg.arc(p[0].x, p[0].y, (s.size * unit) / 2, 0, Math.PI * 2);
          lg.fill();
          return;
        }
        const a = p[i - 2] || p[i - 1];
        const b = p[i - 1];
        const c = p[i];
        lg.beginPath();
        lg.moveTo((a.x + b.x) / 2, (a.y + b.y) / 2);
        lg.quadraticCurveTo(b.x, b.y, (b.x + c.x) / 2, (b.y + c.y) / 2);
        lg.stroke();
      }
      function redraw() {
        lg.clearRect(0, 0, W, H);
        for (const s of strokes) for (let i = 0; i < s.pts.length; i++) seg(s, i);
        lg.globalCompositeOperation = "source-over";
        compose();
      }
      const pos = (e) => {
        const r = canvas.getBoundingClientRect();
        return { x: ((e.clientX - r.left) * W) / r.width, y: ((e.clientY - r.top) * H) / r.height };
      };
      canvas.addEventListener("pointerdown", (e) => {
        canvas.setPointerCapture(e.pointerId);
        cur = { color, size: eraser ? size * 2.5 : size, eraser, pts: [pos(e)] };
        strokes.push(cur);
        seg(cur, 0);
        compose();
      });
      canvas.addEventListener("pointermove", (e) => {
        if (!cur) return;
        let evs = e.getCoalescedEvents ? e.getCoalescedEvents() : [];
        if (!evs.length) evs = [e];
        for (const ev of evs) {
          cur.pts.push(pos(ev));
          seg(cur, cur.pts.length - 1);
        }
        compose();
      });
      const end = () => {
        cur = null;
        lg.globalCompositeOperation = "source-over";
      };
      canvas.addEventListener("pointerup", end);
      canvas.addEventListener("pointercancel", end);

      function setTool() {
        ov.querySelectorAll("[data-color]").forEach((b) => b.classList.toggle("on", !eraser && b.dataset.color === color));
        ov.querySelector('[data-act="eraser"]').classList.toggle("on", eraser);
        ov.querySelectorAll("[data-size]").forEach((b) => b.classList.toggle("on", Number(b.dataset.size) === size));
      }
      async function finish(send) {
        window.removeEventListener("resize", fit);
        window.removeEventListener("keydown", keys, true);
        if (!send) {
          ov.remove();
          return done(null);
        }
        if (!strokes.length && !bg) {
          ov.remove();
          return done(null);
        }
        compose();
        const blob = await canvasBlob(canvas, bg ? "image/jpeg" : "image/png", 0.88);
        ov.remove();
        done(blob);
      }
      function keys(e) {
        if (e.key === "Escape") {
          e.stopPropagation();
          finish(false);
        } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
          e.preventDefault();
          strokes.pop();
          redraw();
        } else if ((e.ctrlKey || e.metaKey) && e.key === "Enter") finish(true);
      }
      ov.addEventListener("click", (e) => {
        const b = e.target.closest("button");
        if (!b) return;
        if (b.dataset.color) {
          color = b.dataset.color;
          eraser = false;
        } else if (b.dataset.size) size = Number(b.dataset.size);
        else if (b.dataset.act === "eraser") eraser = !eraser;
        else if (b.dataset.act === "undo") {
          strokes.pop();
          redraw();
        } else if (b.dataset.act === "clear") {
          strokes.length = 0;
          redraw();
        } else if (b.dataset.act === "cancel") return finish(false);
        else if (b.dataset.act === "send") return finish(true);
        setTool();
      });
      window.addEventListener("resize", fit);
      window.addEventListener("keydown", keys, true);
      compose();
      fit();
      requestAnimationFrame(fit);
    });
  }

  /** Affiche une image en grand, avec « Annoter ». */
  function lightbox(m, { onAnnotate } = {}) {
    const ov = document.createElement("div");
    ov.className = "lightbox";
    ov.innerHTML = `<img alt=""><div class="lb-bar">
      ${onAnnotate ? `<button class="pill-btn" data-act="annotate">${icon("draw")} Dessiner dessus</button>` : ""}
      <button class="pill-btn ghost" data-act="close">Fermer</button></div>`;
    ov.querySelector("img").src = mediaUrl(m.image);
    const close = () => {
      ov.remove();
      window.removeEventListener("keydown", esc, true);
    };
    const esc = (e) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        close();
      }
    };
    ov.addEventListener("click", (e) => {
      const act = e.target.closest("button")?.dataset.act;
      if (act === "annotate") {
        close();
        onAnnotate(m);
      } else if (act === "close" || e.target === ov) close();
    });
    window.addEventListener("keydown", esc, true);
    document.body.append(ov);
  }

  return { invoke, listen, icon, md, mediaUrl, toast, Conversation, Composer, draw, lightbox };
})();
