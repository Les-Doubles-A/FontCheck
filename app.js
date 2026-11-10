(() => {
  const LICENSE_ORDER = ["inconnu", "personnel", "commercial", "libre"];
  const LICENSES = {
    inconnu:    { label: "Inconnu",     code: "?",    color: "#75716b" },
    personnel:  { label: "Personnel",   code: "PERS", color: "#c9463a" },
    commercial: { label: "Commercial",  code: "COMM", color: "#2f8f57" },
    libre:      { label: "Libre (OFL)", code: "OFL",  color: "#3a6fc9" },
  };

  const ROW_HEIGHT = 105;
  const POOL_SIZE = 18;

  // Initial state injected by Rust backend
  const initData = window.__FONTCHECK_INIT__ || {
    fonts: ["Arial", "Calibri", "Cambria", "Consolas", "Courier New", "Georgia", "Segoe UI", "Times New Roman", "Verdana"],
    licenses: {},
    favorites: [],
    version: "1.0.0",
  };

  let allFonts = Array.isArray(initData.fonts) ? initData.fonts : [];
  let filteredFonts = [...allFonts];
  let licenses = { ...(initData.licenses || {}) };
  let favorites = new Set(Array.isArray(initData.favorites) ? initData.favorites : []);
  let activeFilter = "toutes";
  let previewSize = 27;
  let latestUpdateInfo = null;

  // DOM Elements
  const totalLabel = document.getElementById("totalLabel");
  const versionLabel = document.getElementById("versionLabel");
  const checkUpdateBtn = document.getElementById("checkUpdateBtn");
  const updateBanner = document.getElementById("updateBanner");
  const updateMessage = document.getElementById("updateMessage");
  const updateInstallBtn = document.getElementById("updateInstallBtn");
  const updateDismissBtn = document.getElementById("updateDismissBtn");
  const licenseFilters = document.getElementById("licenseFilters");
  const refreshBtn = document.getElementById("refreshBtn");
  const searchInput = document.getElementById("searchInput");
  const sizeBtn = document.getElementById("sizeBtn");
  const sampleInput = document.getElementById("sampleInput");
  const statusLabel = document.getElementById("statusLabel");
  const statusToast = document.getElementById("statusToast");
  const viewport = document.getElementById("viewport");
  const virtualSpacer = document.getElementById("virtualSpacer");
  const emptyState = document.getElementById("emptyState");

  versionLabel.textContent = `v${initData.version || "1.0.0"}`;

  function sendIpc(payload) {
    if (window.ipc && typeof window.ipc.postMessage === "function") {
      window.ipc.postMessage(JSON.stringify(payload));
    }
  }

  let toastTimer = null;
  function showToast(msg, duration = 3500) {
    if (!statusToast) return;
    statusToast.textContent = msg;
    statusToast.hidden = false;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      statusToast.hidden = true;
    }, duration);
  }

  function licenseOf(family) {
    const status = licenses[family];
    return LICENSES[status] ? status : "inconnu";
  }

  // Create fixed pool of virtualized row DOM nodes
  const rowPool = [];
  for (let i = 0; i < POOL_SIZE; i++) {
    const row = document.createElement("div");
    row.className = "font-row";
    row.style.display = "none";

    const top = document.createElement("div");
    top.className = "font-row-top";

    const nameEl = document.createElement("span");
    nameEl.className = "font-row-name";

    const actions = document.createElement("div");
    actions.className = "font-row-actions";

    const licBtn = document.createElement("button");
    licBtn.type = "button";
    licBtn.className = "license-badge";
    licBtn.title = "Changer la catégorie de licence";

    const favBtn = document.createElement("button");
    favBtn.type = "button";
    favBtn.className = "fav-btn";
    favBtn.title = "Ajouter / retirer des favoris";

    actions.append(licBtn, favBtn);
    top.append(nameEl, actions);

    const previewEl = document.createElement("div");
    previewEl.className = "font-row-preview";

    row.append(top, previewEl);
    virtualSpacer.appendChild(row);

    const view = {
      row,
      nameEl,
      licBtn,
      favBtn,
      previewEl,
      boundFamily: null,
    };

    licBtn.addEventListener("click", () => {
      if (view.boundFamily) cycleLicense(view.boundFamily);
    });

    favBtn.addEventListener("click", () => {
      if (view.boundFamily) toggleFavorite(view.boundFamily);
    });

    rowPool.push(view);
  }

  function updateSidebarCounts() {
    totalLabel.textContent = `${allFonts.length} POLICES\nINSTALLÉES`;
    const counts = {
      toutes: allFonts.length,
      favoris: 0,
      inconnu: 0,
      personnel: 0,
      commercial: 0,
      libre: 0,
    };
    for (let i = 0; i < allFonts.length; i++) {
      const f = allFonts[i];
      if (favorites.has(f)) counts.favoris++;
      counts[licenseOf(f)]++;
    }
    for (const [key, val] of Object.entries(counts)) {
      const badge = document.getElementById(`count-${key}`);
      if (badge) badge.textContent = String(val);
    }
  }

  function applyFilter(resetScroll = true) {
    const term = searchInput.value.trim().toLowerCase();
    filteredFonts = allFonts.filter((family) => {
      if (term && !family.toLowerCase().includes(term)) return false;
      if (activeFilter === "toutes") return true;
      if (activeFilter === "favoris") return favorites.has(family);
      return licenseOf(family) === activeFilter;
    });

    const total = filteredFonts.length;
    virtualSpacer.style.height = `${total * ROW_HEIGHT}px`;
    if (resetScroll) {
      viewport.scrollTop = 0;
    }

    statusLabel.textContent = `${total} police(s) affichée(s)`;
    emptyState.hidden = total > 0;
    updateSidebarCounts();
    renderVisible();
  }

  function renderVisible() {
    const scrollTop = viewport.scrollTop;
    const start = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - 1);
    const sampleText = sampleInput.value || "Votre texte ici";

    for (let slot = 0; slot < POOL_SIZE; slot++) {
      const view = rowPool[slot];
      const index = start + slot;

      if (index >= filteredFonts.length) {
        view.row.style.display = "none";
        view.boundFamily = null;
        continue;
      }

      const family = filteredFonts[index];
      view.boundFamily = family;
      view.row.style.display = "flex";
      view.row.style.transform = `translateY(${index * ROW_HEIGHT}px)`;

      view.nameEl.textContent = family;

      const status = licenseOf(family);
      const meta = LICENSES[status];
      view.licBtn.textContent = meta.code;
      view.licBtn.style.color = meta.color;
      view.licBtn.style.borderColor = meta.color;

      const isFav = favorites.has(family);
      view.favBtn.textContent = isFav ? "[x]" : "[ ]";
      view.favBtn.classList.toggle("active", isFav);

      const safeFamily = family.replace(/"/g, '\\"');
      view.previewEl.style.fontFamily = `"${safeFamily}", "Segoe UI", sans-serif`;
      view.previewEl.style.fontSize = `${previewSize}px`;
      view.previewEl.textContent = sampleText;
    }
  }

  function cycleLicense(family) {
    const current = licenseOf(family);
    const nextIndex = (LICENSE_ORDER.indexOf(current) + 1) % LICENSE_ORDER.length;
    const nextStatus = LICENSE_ORDER[nextIndex];

    if (nextStatus === "inconnu") {
      delete licenses[family];
    } else {
      licenses[family] = nextStatus;
    }

    sendIpc({ cmd: "set_license", family, status: nextStatus });

    if (activeFilter !== "toutes" && activeFilter !== "favoris") {
      applyFilter(false);
    } else {
      updateSidebarCounts();
      renderVisible();
    }
  }

  function toggleFavorite(family) {
    if (favorites.has(family)) {
      favorites.delete(family);
    } else {
      favorites.add(family);
    }
    sendIpc({ cmd: "toggle_favorite", family });

    if (activeFilter === "favoris") {
      applyFilter(false);
    } else {
      updateSidebarCounts();
      renderVisible();
    }
  }

  // Events
  licenseFilters.addEventListener("click", (e) => {
    const btn = e.target.closest(".filter-btn");
    if (!btn) return;
    activeFilter = btn.dataset.filter || "toutes";
    for (const b of licenseFilters.querySelectorAll(".filter-btn")) {
      b.classList.toggle("active", b === btn);
    }
    applyFilter(true);
  });

  let filterRaf = null;
  searchInput.addEventListener("input", () => {
    if (filterRaf) cancelAnimationFrame(filterRaf);
    filterRaf = requestAnimationFrame(() => applyFilter(true));
  });

  let previewRaf = null;
  sampleInput.addEventListener("input", () => {
    if (previewRaf) cancelAnimationFrame(previewRaf);
    previewRaf = requestAnimationFrame(renderVisible);
  });

  let scrollRaf = null;
  viewport.addEventListener(
    "scroll",
    () => {
      if (scrollRaf) return;
      scrollRaf = requestAnimationFrame(() => {
        scrollRaf = null;
        renderVisible();
      });
    },
    { passive: true }
  );

  window.addEventListener("resize", renderVisible);

  sizeBtn.addEventListener("click", () => {
    previewSize = previewSize === 27 ? 34 : previewSize === 34 ? 21 : 27;
    renderVisible();
  });

  refreshBtn.addEventListener("click", () => {
    refreshBtn.textContent = "Actualisation…";
    sendIpc({ cmd: "refresh_fonts" });
  });

  document.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      searchInput.focus();
      searchInput.select();
    }
  });

  // --- Update System Handlers ---
  checkUpdateBtn.addEventListener("click", () => {
    checkUpdateBtn.textContent = "Vérification…";
    checkUpdateBtn.disabled = true;
    sendIpc({ cmd: "check_update", manual: true });
  });

  updateDismissBtn.addEventListener("click", () => {
    updateBanner.hidden = true;
  });

  updateInstallBtn.addEventListener("click", () => {
    if (!latestUpdateInfo) return;
    updateInstallBtn.disabled = true;
    updateInstallBtn.textContent = "Téléchargement en cours…";
    sendIpc({
      cmd: "install_update",
      download_url: latestUpdateInfo.download_url || "",
      release_url: latestUpdateInfo.release_url || "",
    });
  });

  window.__onFontsRefreshed = (newFonts) => {
    refreshBtn.textContent = "Actualiser";
    if (Array.isArray(newFonts)) {
      allFonts = newFonts;
      applyFilter(false);
      showToast(`${allFonts.length} polices synchronisées`);
    }
  };

  window.__onUpdateChecked = (info, manual) => {
    checkUpdateBtn.textContent = "↻ Vérifier MAJ";
    checkUpdateBtn.disabled = false;
    if (!info) return;

    latestUpdateInfo = info;

    if (info.available) {
      updateMessage.textContent = `Nouvelle version v${info.latest_version} disponible (actuelle : v${info.current_version})`;
      updateInstallBtn.disabled = false;
      updateInstallBtn.textContent = info.download_url
        ? "Mettre à jour automatiquement"
        : "Télécharger la mise à jour";
      updateBanner.hidden = false;
    } else if (manual) {
      if (info.error) {
        showToast(info.error);
      } else {
        showToast(`Vous utilisez déjà la dernière version (v${info.current_version})`);
      }
    }
  };

  window.__onUpdateError = (errMsg) => {
    updateInstallBtn.disabled = false;
    updateInstallBtn.textContent = "Réessayer la mise à jour";
    showToast(errMsg || "Erreur lors de la mise à jour", 5000);
  };

  // Initial render
  searchInput.focus();
  applyFilter(true);
})();
