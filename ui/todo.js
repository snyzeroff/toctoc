// Listes partagées : éditeur de création et carte vivante dans la conversation.
(() => {
  const { invoke, icon, toast } = Toc;
  const rid = () => [...crypto.getRandomValues(new Uint8Array(6))].map((b) => b.toString(16).padStart(2, "0")).join("");
  const inline = (t) => DOMPurify.sanitize(marked.parseInline(t || ""), { FORBID_TAGS: ["img", "style", "iframe", "form", "input", "button", "svg"], FORBID_ATTR: ["style", "class", "id"] });
  const esc = (t) => String(t).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const MAX_LEVEL = 3;

  /** Arbre ordonné des éléments vivants. */
  function tree(list) {
    const live = list.items.filter((i) => !i.deleted);
    const kids = (p) => live.filter((i) => (i.parent || null) === p).sort((a, b) => a.order - b.order);
    const walk = (p, depth) => kids(p).flatMap((i) => [{ item: i, depth }, ...(depth < 5 ? walk(i.id, depth + 1) : [])]);
    return walk(null, 0);
  }

  function plain(list) {
    const lines = tree(list).map(({ item, depth }) => "  ".repeat(depth) + (item.task ? (item.done ? "☑ " : "☐ ") : "• ") + item.text);
    return (list.title ? list.title + "\n" : "") + lines.join("\n");
  }

  // ───────── éditeur de création ─────────
  /** Ouvre l'éditeur ; renvoie { title, items } ou null. */
  function editor() {
    return new Promise((done) => {
      const ov = document.createElement("div");
      ov.className = "draw-overlay";
      ov.innerHTML = `
        <div class="draw-panel todo-editor">
          <div class="draw-top">
            <button class="link" data-act="cancel">Annuler</button>
            <strong>Nouvelle liste</strong>
            <button class="pill-btn" data-act="send">Envoyer</button>
          </div>
          <div class="te-body">
            <input class="te-title" maxlength="120" placeholder="Titre (facultatif)" spellcheck="true">
            <div class="te-rows"></div>
            <button class="link te-add" data-act="add">+ Ajouter une ligne</button>
            <p class="hint te-hint">Entrée : nouvelle ligne · Tab : en dessous (sous-élément) · Maj + Tab : remonter ·
            clic sur la case : case à cocher ↔ simple point</p>
          </div>
        </div>`;
      document.body.append(ov);
      const rowsEl = ov.querySelector(".te-rows");
      let rows = [];

      function render(focusIdx, caret) {
        rowsEl.innerHTML = "";
        rows.forEach((r, i) => {
          const row = document.createElement("div");
          row.className = "te-row";
          row.style.paddingLeft = r.level * 22 + "px";
          row.innerHTML = `<button class="te-kind ${r.task ? "task" : "point"}" title="Case à cocher ou simple point"></button>
            <input value="${esc(r.text)}" maxlength="500" placeholder="${r.level ? "Sous-élément" : "Élément"}" spellcheck="true">
            <button class="te-del" title="Supprimer la ligne">${icon("close")}</button>`;
          const inp = row.querySelector("input");
          inp.oninput = () => (r.text = inp.value);
          inp.onkeydown = (e) => key(e, i, inp);
          row.querySelector(".te-kind").onclick = () => {
            r.task = !r.task;
            render(i);
          };
          row.querySelector(".te-del").onclick = () => {
            rows.splice(i, 1);
            if (!rows.length) rows.push({ level: 0, task: true, text: "" });
            render(Math.max(0, i - 1));
          };
          rowsEl.append(row);
        });
        if (focusIdx != null) {
          const inp = rowsEl.querySelectorAll("input")[focusIdx];
          if (inp) {
            inp.focus();
            const pos = caret ?? inp.value.length;
            inp.setSelectionRange(pos, pos);
          }
        }
      }
      function key(e, i, inp) {
        const r = rows[i];
        if (e.key === "Enter") {
          e.preventDefault();
          if (e.ctrlKey || e.metaKey) return finish(true);
          rows.splice(i + 1, 0, { level: r.level, task: r.task, text: "" });
          render(i + 1);
        } else if (e.key === "Tab") {
          e.preventDefault();
          const max = i === 0 ? 0 : Math.min(MAX_LEVEL, rows[i - 1].level + 1);
          r.level = e.shiftKey ? Math.max(0, r.level - 1) : Math.min(max, r.level + 1);
          // les lignes en dessous ne doivent pas « flotter »
          for (let j = i + 1; j < rows.length && rows[j].level > rows[j - 1].level + 1; j++) rows[j].level = rows[j - 1].level + 1;
          render(i, inp.selectionStart);
        } else if (e.key === "Backspace" && !inp.value && rows.length > 1) {
          e.preventDefault();
          rows.splice(i, 1);
          render(Math.max(0, i - 1));
        } else if (e.key === "ArrowUp" && i > 0) {
          e.preventDefault();
          render(i - 1);
        } else if (e.key === "ArrowDown" && i < rows.length - 1) {
          e.preventDefault();
          render(i + 1);
        }
      }
      function finish(send) {
        window.removeEventListener("keydown", esc_, true);
        if (!send) {
          ov.remove();
          return done(null);
        }
        const items = [];
        const stack = []; // dernier id vu à chaque niveau
        let order = 0;
        for (const r of rows) {
          const text = r.text.trim();
          if (!text) continue;
          const level = Math.min(r.level, stack.length);
          const id = rid();
          items.push({ id, parent: level ? stack[level - 1] : null, text, task: r.task, done: false, order: ++order, ts: 0 });
          stack.length = level;
          stack[level] = id;
        }
        if (!items.length) return toast("Écrivez au moins un élément.");
        ov.remove();
        const title = ov.querySelector(".te-title").value.trim();
        done({ title: title || null, items });
      }
      function esc_(e) {
        if (e.key === "Escape") {
          e.stopPropagation();
          finish(false);
        }
      }
      ov.addEventListener("click", (e) => {
        const act = e.target.closest("button")?.dataset.act;
        if (act === "cancel") finish(false);
        else if (act === "send") finish(true);
        else if (act === "add") {
          const last = rows[rows.length - 1];
          rows.push({ level: last ? last.level : 0, task: last ? last.task : true, text: "" });
          render(rows.length - 1);
        }
      });
      ov.querySelector(".te-title").onkeydown = (e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          rowsEl.querySelector("input")?.focus();
        }
      };
      window.addEventListener("keydown", esc_, true);
      rows = [{ level: 0, task: true, text: "" }];
      render();
      ov.querySelector(".te-title").focus();
    });
  }

  async function create() {
    const todo = await editor();
    if (!todo) return;
    try {
      await invoke("send_todo", { todo });
    } catch (err) {
      toast(String(err));
    }
  }

  // ───────── carte vivante dans la conversation ─────────
  const op = (id, o) => invoke("todo_op", { id, op: o }).catch((err) => toast(String(err)));
  /** Ce qu'on était en train de faire dans une liste, pour le retrouver après mise à jour. */
  let keep = null;

  function card(m) {
    const list = m.todo;
    const el = document.createElement("div");
    el.className = "todo";
    const [done, all] = list.items.filter((i) => !i.deleted && i.task).reduce(([d, a], i) => [d + (i.done ? 1 : 0), a + 1], [0, 0]);
    el.innerHTML = `<div class="td-head"><span class="td-ico">${icon("checklist")}</span>
      <span class="td-title">${esc(list.title || "Liste")}</span>
      ${all ? `<span class="td-count ${done === all ? "full" : ""}">${done}/${all}</span>` : ""}</div>
      ${all ? `<div class="td-bar"><i style="width:${(100 * done) / all}%"></i></div>` : ""}
      <ul class="td-items"></ul>
      <div class="td-new"><input placeholder="Ajouter un élément…" maxlength="500"></div>`;
    const ul = el.querySelector(".td-items");
    for (const { item, depth } of tree(list)) {
      const li = document.createElement("li");
      li.className = "td-item" + (item.task ? "" : " point") + (item.done ? " done" : "");
      li.style.paddingLeft = depth * 20 + "px";
      li.dataset.item = item.id;
      li.__text = item.text;
      li.innerHTML = `${item.task ? `<button class="td-check" title="${item.done ? "Décocher" : "Cocher"}" role="checkbox" aria-checked="${item.done}">${item.done ? icon("check") : ""}</button>` : '<span class="td-dot">•</span>'}
        <span class="td-text" title="Double-clic pour modifier">${inline(item.text)}</span>
        <span class="td-tools">
          <button data-act="sub" title="Ajouter dessous">+</button>
          <button data-act="kind" title="${item.task ? "Transformer en simple point" : "Transformer en case à cocher"}">${item.task ? "•" : "☐"}</button>
          <button data-act="del" title="Supprimer">${icon("close")}</button>
        </span>`;
      ul.append(li);
    }

    el.addEventListener("click", (e) => {
      const li = e.target.closest(".td-item");
      if (!li) return;
      const item = li.dataset.item;
      if (e.target.closest(".td-check")) return op(m.id, { op: "toggle", item });
      const act = e.target.closest("button")?.dataset.act;
      if (act === "del") op(m.id, { op: "remove", item });
      else if (act === "kind") op(m.id, { op: "kind", item });
      else if (act === "sub") subInput(li, item);
    });
    el.addEventListener("dblclick", (e) => {
      const t = e.target.closest(".td-text");
      if (t) editText(t, m.id, t.closest(".td-item").dataset.item);
    });
    const addInput = el.querySelector(".td-new input");
    addInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && addInput.value.trim()) {
        keep = { id: m.id, parent: null };
        op(m.id, { op: "add", parent: null, text: addInput.value, task: true });
        addInput.value = "";
      } else if (e.key === "Escape") addInput.blur();
    });
    function subInput(li, parent) {
      el.querySelector(".td-sub")?.remove();
      const box = document.createElement("li");
      box.className = "td-sub";
      box.style.paddingLeft = parseInt(li.style.paddingLeft || 0) + 20 + "px";
      box.innerHTML = '<input placeholder="Sous-élément…" maxlength="500">';
      li.after(box);
      const inp = box.querySelector("input");
      inp.focus();
      inp.onkeydown = (e) => {
        if (e.key === "Enter" && inp.value.trim()) {
          keep = { id: m.id, parent };
          op(m.id, { op: "add", parent, text: inp.value, task: true });
          inp.value = "";
        } else if (e.key === "Escape") box.remove();
      };
      inp.onblur = () => setTimeout(() => !inp.value && box.remove(), 150);
    }
    // après une mise à jour, on remet le curseur là où on écrivait
    if (keep && keep.id === m.id) {
      const k = keep;
      keep = null;
      requestAnimationFrame(() => {
        if (!k.parent) el.querySelector(".td-new input")?.focus();
        else {
          const li = el.querySelector(`[data-item="${k.parent}"]`);
          if (li) subInput(li, k.parent);
        }
      });
    }
    return el;
  }

  function editText(span, id, item) {
    const li = span.closest(".td-item");
    const m = span.closest(".todo");
    const cur = li.__text ?? span.textContent;
    const inp = document.createElement("input");
    inp.className = "td-edit";
    inp.value = cur;
    inp.maxLength = 500;
    span.replaceWith(inp);
    inp.focus();
    inp.select();
    let finished = false;
    const end = (save) => {
      if (finished) return;
      finished = true;
      if (save && inp.value.trim() && inp.value.trim() !== cur) op(id, { op: "rename", item, text: inp.value });
      else inp.replaceWith(span);
    };
    inp.onkeydown = (e) => {
      if (e.key === "Enter") end(true);
      else if (e.key === "Escape") end(false);
    };
    inp.onblur = () => end(true);
  }

  Object.assign(Toc, { todoCard: card, todoPlain: plain, newTodo: create });
})();
