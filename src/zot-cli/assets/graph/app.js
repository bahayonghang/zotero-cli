/* Interactive viewer for a zot knowledge graph.
 * Loads the one-shot /graph.json snapshot and renders it with Cytoscape.
 * All filtering happens client-side; the server never re-queries. */
(async function () {
  "use strict";
  const $ = (id) => document.getElementById(id);
  const PALETTE = [
    "#2f81f7", "#3fb950", "#e3b341", "#f85149", "#bc8cff", "#39c5cf",
    "#ff7b72", "#d2a8ff", "#7ee787", "#ffa657", "#a5d6ff", "#ff9bce",
  ];
  const colorFor = (c) => PALETTE[((c % PALETTE.length) + PALETTE.length) % PALETTE.length];
  const esc = (s) => (s == null ? "" : String(s).replace(/[&<>"]/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c])));

  let raw;
  try {
    const res = await fetch("/graph.json");
    raw = await res.json();
  } catch (err) {
    $("loading").textContent = "Failed to load graph.json: " + err;
    return;
  }
  const data = raw && raw.nodes ? raw : (raw && raw.data) || { nodes: [], edges: [], metrics: {} };
  const nodes = data.nodes || [];
  const edges = data.edges || [];
  const metrics = data.metrics || {};

  const nodeByKey = new Map(nodes.map((n) => [n.key, n]));
  const maxWeight = edges.reduce((m, e) => Math.max(m, e.weight), 1);
  const maxDegree = nodes.reduce((m, n) => Math.max(m, n.degree), 1);

  $("scope").textContent = "Scope: " + (data.scope || "—");
  $("counts").textContent = nodes.length + " nodes · " + edges.length + " edges · " +
    (metrics.connected_components ?? "?") + " components · " +
    (metrics.communities ? metrics.communities.length : "?") + " communities";

  const minWeight = $("minWeight");
  const minDegree = $("minDegree");
  minWeight.max = String(Math.min(maxWeight, 200));
  minDegree.max = String(Math.min(maxDegree, 60));

  const cy = cytoscape({
    container: $("cy"),
    wheelSensitivity: 0.25,
    style: [
      {
        selector: "node",
        style: {
          "background-color": (ele) => colorFor(ele.data("community")),
          label: "data(short)",
          color: "#c9d1d9",
          "font-size": "7px",
          width: (ele) => 8 + 38 * (ele.data("degree") / maxDegree),
          height: (ele) => 8 + 38 * (ele.data("degree") / maxDegree),
          "text-wrap": "ellipsis",
          "text-max-width": "90px",
          "min-zoomed-font-size": 7,
        },
      },
      {
        selector: "edge",
        style: {
          width: (ele) => 0.4 + 2.6 * (ele.data("weight") / maxWeight),
          "line-color": "#30363d",
          "curve-style": "haystack",
          opacity: 0.55,
        },
      },
      { selector: "node.dim", style: { opacity: 0.1, "text-opacity": 0 } },
      { selector: "node.sel", style: { "border-width": 2, "border-color": "#ffffff" } },
    ],
  });

  function enabledRelations() {
    const set = new Set();
    document.querySelectorAll(".rel").forEach((c) => { if (c.checked) set.add(c.value); });
    return set;
  }

  function rebuild(runLayout) {
    const minW = Number(minWeight.value);
    const minD = Number(minDegree.value);
    const rels = enabledRelations();

    const visibleEdges = edges.filter(
      (e) => e.weight >= minW && e.relations.some((r) => rels.has(r))
    );
    const vdeg = new Map();
    for (const e of visibleEdges) {
      vdeg.set(e.source, (vdeg.get(e.source) || 0) + 1);
      vdeg.set(e.target, (vdeg.get(e.target) || 0) + 1);
    }
    const visibleKeys = new Set();
    for (const n of nodes) {
      if ((vdeg.get(n.key) || 0) >= minD) visibleKeys.add(n.key);
    }
    const els = [];
    for (const n of nodes) {
      if (!visibleKeys.has(n.key)) continue;
      els.push({
        data: {
          id: n.key,
          short: n.title && n.title.length > 42 ? n.title.slice(0, 41) + "…" : n.title || n.key,
          degree: n.degree, community: n.community,
        },
      });
    }
    for (const e of visibleEdges) {
      if (visibleKeys.has(e.source) && visibleKeys.has(e.target)) {
        els.push({ data: { source: e.source, target: e.target, weight: e.weight } });
      }
    }
    cy.elements().remove();
    cy.add(els);
    $("counts").textContent = els.filter((x) => !x.data.source).length + " / " + nodes.length +
      " nodes shown · " + visibleEdges.length + " edges";
    if (runLayout) layout();
    applySearch();
  }

  function layout() {
    const n = cy.nodes().length;
    cy.layout({
      name: "cose",
      animate: false,
      randomize: true,
      componentSpacing: 90,
      nodeRepulsion: 9000,
      numIter: n > 900 ? 400 : 1000,
    }).run();
  }

  function applySearch() {
    const q = $("search").value.trim().toLowerCase();
    if (!q) { cy.nodes().removeClass("dim"); return; }
    cy.nodes().forEach((ele) => {
      const node = nodeByKey.get(ele.id());
      const hay = ((node && node.title) || "") + " " + ((node && node.first_author) || "");
      ele.toggleClass("dim", !hay.toLowerCase().includes(q));
    });
  }

  function showDetail(node) {
    if (!node) { $("detail").innerHTML = '<div class="empty">Click a node to inspect it.</div>'; return; }
    const rows = [];
    rows.push('<div class="title">' + esc(node.title || node.key) + "</div>");
    const field = (k, v) => v ? '<div class="field"><span class="k">' + k + ":</span> " + v + "</div>" : "";
    rows.push(field("Type", esc(node.item_type)));
    rows.push(field("Year", esc(node.year)));
    rows.push(field("First author", esc(node.first_author)));
    rows.push(field("Connections", node.degree + " (weighted " + node.weighted_degree + ")"));
    rows.push(field("Community", "#" + node.community));
    if (node.doi) rows.push(field("DOI", '<a href="https://doi.org/' + esc(node.doi) + '" target="_blank">' + esc(node.doi) + "</a>"));
    if (node.url) rows.push(field("URL", '<a href="' + esc(node.url) + '" target="_blank">' + esc(node.url) + "</a>"));
    rows.push(field("Zotero", '<a href="zotero://select/library/items/' + esc(node.key) + '">open in Zotero</a>'));
    if (node.tags && node.tags.length) {
      rows.push('<div class="field"><span class="k">Tags:</span><br>' +
        node.tags.map((t) => '<span class="tag">' + esc(t) + "</span>").join("") + "</div>");
    }
    $("detail").innerHTML = rows.join("");
  }

  function renderLegend() {
    const list = (metrics.communities || []).slice(0, 10);
    $("legend").innerHTML = list.map((c) =>
      '<div class="item"><span class="swatch" style="background:' + colorFor(c.id) + '"></span>' +
      "<span>#" + c.id + " · " + c.size + " papers" +
      (c.top_tags && c.top_tags.length ? " · " + esc(c.top_tags.join(", ")) : "") + "</span></div>"
    ).join("") || '<div class="empty">No communities.</div>';
  }

  cy.on("tap", "node", (evt) => {
    cy.nodes().removeClass("sel");
    evt.target.addClass("sel");
    showDetail(nodeByKey.get(evt.target.id()));
  });
  cy.on("tap", (evt) => { if (evt.target === cy) { cy.nodes().removeClass("sel"); showDetail(null); } });

  let searchTimer;
  $("search").addEventListener("input", () => { clearTimeout(searchTimer); searchTimer = setTimeout(applySearch, 150); });
  const onFilter = (run) => () => {
    $("minWeightVal").textContent = minWeight.value;
    $("minDegreeVal").textContent = minDegree.value;
    rebuild(run);
  };
  minWeight.addEventListener("change", onFilter(true));
  minDegree.addEventListener("change", onFilter(true));
  minWeight.addEventListener("input", () => { $("minWeightVal").textContent = minWeight.value; });
  minDegree.addEventListener("input", () => { $("minDegreeVal").textContent = minDegree.value; });
  document.querySelectorAll(".rel").forEach((c) => c.addEventListener("change", onFilter(true)));
  $("relayout").addEventListener("click", () => layout());

  renderLegend();
  rebuild(true);
  $("loading").style.display = "none";
})();
