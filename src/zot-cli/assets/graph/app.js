/* Interactive viewer for a zot knowledge graph.
 * Loads the one-shot /graph.json snapshot and renders it with Cytoscape.
 * All filtering happens client-side; the server never re-queries. */
(async function () {
  "use strict";
  const $ = (id) => document.getElementById(id);
  const element = (tag, className, text) => {
    const value = document.createElement(tag);
    if (className) value.className = className;
    if (text != null) value.textContent = String(text);
    return value;
  };
  const PALETTE = [
    "#2f81f7", "#3fb950", "#e3b341", "#f85149", "#bc8cff", "#39c5cf",
    "#ff7b72", "#d2a8ff", "#7ee787", "#ffa657", "#a5d6ff", "#ff9bce",
  ];
  const colorFor = (c) => PALETTE[((c % PALETTE.length) + PALETTE.length) % PALETTE.length];
  function safeWebUrl(value) {
    try {
      const url = new URL(String(value));
      return url.protocol === "http:" || url.protocol === "https:" ? url.href : null;
    } catch (_) {
      return null;
    }
  }

  function externalLink(label, href) {
    const link = element("a", "", label);
    link.href = href;
    link.target = "_blank";
    link.rel = "noopener noreferrer";
    return link;
  }

  function appendField(parent, label, value) {
    if (value == null || value === "") return;
    const field = element("div", "field");
    field.append(element("span", "k", label + ":"), " ");
    field.append(value instanceof Node ? value : document.createTextNode(String(value)));
    parent.append(field);
  }

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
    const detail = $("detail");
    detail.replaceChildren();
    if (!node) {
      detail.append(element("div", "empty", "Click a node to inspect it."));
      return;
    }
    detail.append(element("div", "title", node.title || node.key));
    appendField(detail, "Type", node.item_type);
    appendField(detail, "Year", node.year);
    appendField(detail, "First author", node.first_author);
    appendField(detail, "Connections", node.degree + " (weighted " + node.weighted_degree + ")");
    appendField(detail, "Community", "#" + node.community);
    if (node.doi) {
      const doiUrl = new URL("https://doi.org/");
      doiUrl.pathname = String(node.doi);
      appendField(detail, "DOI", externalLink(node.doi, doiUrl.href));
    }
    if (node.url) {
      const href = safeWebUrl(node.url);
      appendField(detail, "URL", href ? externalLink(node.url, href) : node.url);
    }
    const zoteroLink = element("a", "", "open in Zotero");
    zoteroLink.href = "zotero://select/library/items/" + encodeURIComponent(String(node.key));
    appendField(detail, "Zotero", zoteroLink);
    if (node.tags && node.tags.length) {
      const tags = element("div", "field");
      tags.append(element("span", "k", "Tags:"), document.createElement("br"));
      node.tags.forEach((tag) => tags.append(element("span", "tag", tag)));
      detail.append(tags);
    }
  }

  function renderLegend() {
    const list = (metrics.communities || []).slice(0, 10);
    const legend = $("legend");
    legend.replaceChildren();
    if (!list.length) {
      legend.append(element("div", "empty", "No communities."));
      return;
    }
    list.forEach((community) => {
      const item = element("div", "item");
      const swatch = element("span", "swatch");
      swatch.style.background = colorFor(community.id);
      const tags = community.top_tags && community.top_tags.length
        ? " · " + community.top_tags.join(", ")
        : "";
      item.append(
        swatch,
        element("span", "", "#" + community.id + " · " + community.size + " papers" + tags)
      );
      legend.append(item);
    });
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
