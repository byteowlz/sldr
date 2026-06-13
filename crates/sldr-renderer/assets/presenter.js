/**
 * sldr Presenter Engine
 *
 * Vanilla JS presentation controller embedded into self-contained HTML output.
 * Features: keyboard navigation, CSS transitions, speaker notes, overview grid,
 * touch gestures, fullscreen, progress bar, URL hash routing, dark/light mode
 * toggle, live flavor switching, click-through builds (data-click-step).
 *
 * No external dependencies. Single IIFE.
 *
 * FLAVOR EMBEDDING CONTRACT
 * -------------------------
 * The renderer embeds flavors as <style data-flavor="Name"> blocks in <head>.
 * The active flavor has no `disabled` attribute; all others have `disabled`.
 *
 *   Single flavor (default):
 *     <style data-flavor="Acme">:root { ... } html.dark { ... }</style>
 *     -> Toolbar shows dark/light toggle only. No flavor selector.
 *
 *   Multi-flavor (--flavors flag):
 *     <style data-flavor="Acme">...</style>
 *     <style data-flavor="Dark" disabled>...</style>
 *     <style data-flavor="Mono" disabled>...</style>
 *     -> Toolbar shows dark/light toggle + flavor dropdown (T key).
 *
 * Dark/light mode works in both modes via html.dark class + CSS variables.
 */
(function () {
  "use strict";

  // ---------------------------------------------------------------------------
  // DOM references
  // ---------------------------------------------------------------------------
  var deck = document.querySelector(".sldr-deck");
  if (!deck) return;

  var allSlides = Array.from(deck.querySelectorAll(".sldr-slide"));

  // Language axis (ADR-0007): multi-language decks duplicate each slide
  // per embedded language, tagged with data-lang. Only the active
  // language's variants take part in navigation; "L" cycles languages.
  var langNames = (deck.dataset.langs || "")
    .split(",")
    .map(function (l) { return l.trim(); })
    .filter(function (l) { return l.length > 0; });
  var activeLang = null;
  if (langNames.length > 1) {
    try {
      var savedLang = localStorage.getItem("sldr-lang");
      if (savedLang && langNames.indexOf(savedLang) !== -1) activeLang = savedLang;
    } catch (err) { /* file:// or private mode */ }
    if (!activeLang) activeLang = langNames[0];
  }

  function slidesForLang(lang) {
    return allSlides.filter(function (s) {
      return !s.dataset.lang || s.dataset.lang === lang;
    });
  }

  var slides = langNames.length > 1 ? slidesForLang(activeLang) : allSlides;
  var progress = document.querySelector(".sldr-progress");
  var pageNum = document.querySelector(".sldr-page-num");
  var overlay = null; // overview grid, created lazily
  var notesWin = null; // speaker notes window

  var total = slides.length;
  if (total === 0) return;

  function setLanguage(lang) {
    if (langNames.indexOf(lang) === -1 || lang === activeLang) return;
    activeLang = lang;
    try { localStorage.setItem("sldr-lang", lang); } catch (err) { /* ignore */ }
    // Hide everything, recompute the active set, stay on the same position.
    for (var i = 0; i < allSlides.length; i++) forceHide(allSlides[i]);
    slides = slidesForLang(activeLang);
    total = slides.length;
    if (current >= total) current = total - 1;
    showSlide(current, 0, current);
    updateProgress();
  }

  // --- Effect diagnostics (?fxdebug) -------------------------------------
  // Self-reporting overlay: tells us whether ANY css animation runs on this
  // machine (control bar) and whether the decoration effect is ticking.
  if (location.search.indexOf("fxdebug") !== -1) {
    var box = document.createElement("div");
    box.style.cssText =
      "position:fixed;top:12px;left:12px;z-index:99999;background:#000;color:#0f0;" +
      "font:12px/1.5 monospace;padding:12px 14px;border:1px solid #0f0;max-width:48ch;" +
      "pointer-events:none";
    var readout = document.createElement("div");
    readout.style.whiteSpace = "pre";
    var ctrl = document.createElement("div");
    ctrl.style.cssText =
      "width:40px;height:12px;margin-top:8px;background:#0f0;" +
      "animation:sldr-diag-slide 1s linear infinite";
    var kf = document.createElement("style");
    kf.textContent =
      "@keyframes sldr-diag-slide{from{transform:translateX(0)}to{transform:translateX(200px)}}";
    document.head.appendChild(kf);
    box.appendChild(readout);
    box.appendChild(ctrl);
    document.body.appendChild(box);
    var fxEl = document.querySelector(".sldr-fx");

    function sample() {
      var reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
      // Reliable tick detection: getBoundingClientRect reflects the actual
      // animated render position; getComputedStyle does not (it can return
      // the base value mid-animation).
      var ctrlX1 = ctrl.getBoundingClientRect().left;
      var fxT1 = fxEl ? getComputedStyle(fxEl, "::before").transform : "-";
      setTimeout(function () {
        var ctrlX2 = ctrl.getBoundingClientRect().left;
        var fxT2 = fxEl ? getComputedStyle(fxEl, "::before").transform : "-";
        var ps = fxEl ? getComputedStyle(fxEl, "::before") : null;
        readout.textContent =
          "FX DIAGNOSTIC\n" +
          "prefers-reduced-motion: " + reduced + "\n" +
          "data-sldr-motion: " + (document.documentElement.getAttribute("data-sldr-motion") || "(unset)") + "\n" +
          "active flavor: " + (activeFlavor || "?") + "\n" +
          ".sldr-fx present/display: " + (!!fxEl) + " / " + (fxEl ? getComputedStyle(fxEl).display : "-") + "\n" +
          "::before anim-name: " + (ps ? ps.animationName : "-") + "\n" +
          "::before play-state: " + (ps ? ps.animationPlayState : "-") + "\n" +
          "CONTROL BAR MOVING: " + (Math.abs(ctrlX1 - ctrlX2) > 0.5 ? "YES" : "NO  <- ALL anim disabled") + "\n" +
          "EFFECT TRANSFORM CHANGING: " + (fxT1 !== fxT2 ? "YES" : "NO") + "\n" +
          "(does the green bar below slide right?)";
      }, 350);
    }
    sample();
    setInterval(sample, 1500);
  }

  // Motion toggle (M): forces decoration animation on/off regardless of
  // the OS prefers-reduced-motion setting. Persisted per browser.
  try {
    var savedMotion = localStorage.getItem("sldr-motion");
    if (savedMotion === "on" || savedMotion === "off") {
      document.documentElement.setAttribute("data-sldr-motion", savedMotion);
    }
  } catch (err) { /* file:// or private mode */ }

  function toggleMotion() {
    var el = document.documentElement;
    var next;
    if (el.getAttribute("data-sldr-motion") === "on") {
      next = "off";
    } else if (el.getAttribute("data-sldr-motion") === "off") {
      next = "on";
    } else {
      // No explicit choice yet: flip away from whatever the OS does.
      next = matchMedia("(prefers-reduced-motion: reduce)").matches ? "on" : "off";
    }
    el.setAttribute("data-sldr-motion", next);
    try { localStorage.setItem("sldr-motion", next); } catch (err) { /* ignore */ }
    if (hintEl) { hintEl.remove(); hintEl = null; }
  }

  // Discoverability: if the system requests reduced motion AND this deck has
  // a decoration effect, the effect is frozen by the accessibility reset.
  // Most people never knowingly enabled reduced motion — surface the M-key
  // opt-in once instead of leaving them staring at a static background.
  var hintEl = null;
  if (
    document.querySelector(".sldr-fx") &&
    matchMedia("(prefers-reduced-motion: reduce)").matches &&
    document.documentElement.getAttribute("data-sldr-motion") !== "on" &&
    document.documentElement.getAttribute("data-sldr-motion") !== "off"
  ) {
    hintEl = document.createElement("div");
    hintEl.textContent = "Your system has reduced motion on — press M for animated background";
    hintEl.style.cssText =
      "position:fixed;bottom:18px;left:50%;transform:translateX(-50%);z-index:200;" +
      "background:var(--sldr-surface,#222);color:var(--sldr-text,#eee);" +
      "border:1px solid var(--sldr-border-bright,#555);border-radius:999px;" +
      "padding:8px 18px;font:13px/1.4 var(--sldr-code-font,monospace);" +
      "box-shadow:var(--sldr-shadow-md,0 4px 12px rgba(0,0,0,.3));opacity:0.94;" +
      "pointer-events:none";
    var showHint = function () {
      if (hintEl) document.body.appendChild(hintEl);
    };
    if (document.body) { showHint(); } else { window.addEventListener("DOMContentLoaded", showHint); }
    setTimeout(function () { if (hintEl) { hintEl.remove(); hintEl = null; } }, 8000);
  }

  function cycleLanguage() {
    if (langNames.length < 2) return;
    var next = langNames[(langNames.indexOf(activeLang) + 1) % langNames.length];
    setLanguage(next);
  }

  // ---------------------------------------------------------------------------
  // State
  // ---------------------------------------------------------------------------
  var current = 0;
  var clickStep = 0;
  var overviewOpen = false;
  var flavorPanelOpen = false;
  var transition = deck.dataset.transition || "fade";
  var isDark = document.documentElement.classList.contains("dark");

  // ---------------------------------------------------------------------------
  // Flavor system
  // ---------------------------------------------------------------------------
  var flavorStyles = Array.from(document.querySelectorAll("style[data-flavor]"));
  var flavorNames = flavorStyles.map(function (s) { return s.dataset.flavor; });
  var activeFlavor = "";

  function initFlavors() {
    if (flavorStyles.length === 0) return;
    // The HTML `disabled` attribute on <style> is markup-inert — it is a
    // DOM-only property, so at load EVERY flavor block applies and the
    // last one in the document wins. Enforce single-active here: the
    // first flavor (the build's resolved active) wins unless the markup
    // genuinely carries the attribute for a later one.
    activeFlavor = flavorStyles[0].dataset.flavor;
    for (var i = 0; i < flavorStyles.length; i++) {
      if (!flavorStyles[i].hasAttribute("disabled")) {
        activeFlavor = flavorStyles[i].dataset.flavor;
        break;
      }
    }
    for (var j = 0; j < flavorStyles.length; j++) {
      flavorStyles[j].disabled = flavorStyles[j].dataset.flavor !== activeFlavor;
    }
  }

  function switchFlavor(name) {
    if (name === activeFlavor) return;

    for (var i = 0; i < flavorStyles.length; i++) {
      if (flavorStyles[i].dataset.flavor === name) {
        flavorStyles[i].disabled = false;
      } else {
        flavorStyles[i].disabled = true;
      }
    }

    activeFlavor = name;
    updateToolbarFlavor();

    // Persist choice in sessionStorage
    try { sessionStorage.setItem("sldr-flavor", name); } catch (e) { /* noop */ }
  }

  function restoreFlavor() {
    try {
      var saved = sessionStorage.getItem("sldr-flavor");
      if (saved && flavorNames.indexOf(saved) !== -1) {
        switchFlavor(saved);
      }
    } catch (e) { /* noop */ }
  }

  // ---------------------------------------------------------------------------
  // Dark / Light mode
  // ---------------------------------------------------------------------------
  function toggleDarkMode() {
    isDark = !isDark;
    document.documentElement.classList.toggle("dark", isDark);
    updateToolbarDark();

    // Persist choice
    try { sessionStorage.setItem("sldr-dark", isDark ? "1" : "0"); } catch (e) { /* noop */ }
  }

  function restoreDarkMode() {
    try {
      var saved = sessionStorage.getItem("sldr-dark");
      if (saved === "1" && !isDark) {
        toggleDarkMode();
      } else if (saved === "0" && isDark) {
        toggleDarkMode();
      }
    } catch (e) { /* noop */ }
  }

  // ---------------------------------------------------------------------------
  // Toolbar (bottom bar with dark mode toggle + flavor selector)
  // ---------------------------------------------------------------------------
  var toolbar = null;
  var darkBtn = null;
  var flavorBtn = null;
  var flavorPanel = null;

  function createToolbar() {
    toolbar = document.createElement("div");
    toolbar.className = "sldr-toolbar";

    // Dark mode toggle button
    darkBtn = document.createElement("button");
    darkBtn.className = "sldr-toolbar-btn";
    darkBtn.setAttribute("aria-label", "Toggle dark/light mode (D)");
    darkBtn.setAttribute("title", "Toggle dark/light mode (D)");
    darkBtn.innerHTML = isDark ? getSunIcon() : getMoonIcon();
    darkBtn.addEventListener("click", function (e) {
      e.stopPropagation();
      toggleDarkMode();
    });

    toolbar.appendChild(darkBtn);

    // Flavor selector - only show if there are multiple flavors
    if (flavorNames.length > 1) {
      // Flavor button
      flavorBtn = document.createElement("button");
      flavorBtn.className = "sldr-toolbar-btn sldr-flavor-btn";
      flavorBtn.setAttribute("aria-label", "Switch flavor (T)");
      flavorBtn.setAttribute("title", "Switch flavor (T)");
      flavorBtn.innerHTML = getPaletteIcon() + '<span class="sldr-flavor-label">' + escapeHtml(activeFlavor) + "</span>";
      flavorBtn.addEventListener("click", function (e) {
        e.stopPropagation();
        toggleFlavorPanel();
      });

      toolbar.appendChild(flavorBtn);

      // Flavor panel (dropdown)
      flavorPanel = document.createElement("div");
      flavorPanel.className = "sldr-flavor-panel";
      flavorPanel.setAttribute("aria-hidden", "true");

      for (var i = 0; i < flavorNames.length; i++) {
        var item = document.createElement("button");
        item.className = "sldr-flavor-item";
        if (flavorNames[i] === activeFlavor) item.classList.add("sldr-flavor-active");
        item.textContent = flavorNames[i];
        item.dataset.flavor = flavorNames[i];
        item.addEventListener("click", onFlavorItemClick);
        flavorPanel.appendChild(item);
      }

      toolbar.appendChild(flavorPanel);
    }

    document.body.appendChild(toolbar);
  }

  function onFlavorItemClick(e) {
    e.stopPropagation();
    var name = e.currentTarget.dataset.flavor;
    switchFlavor(name);
    closeFlavorPanel();
  }

  function toggleFlavorPanel() {
    if (!flavorPanel) return;
    flavorPanelOpen = !flavorPanelOpen;

    if (flavorPanelOpen) {
      flavorPanel.classList.add("sldr-flavor-panel-open");
      flavorPanel.setAttribute("aria-hidden", "false");
    } else {
      closeFlavorPanel();
    }
  }

  function closeFlavorPanel() {
    if (!flavorPanel) return;
    flavorPanelOpen = false;
    flavorPanel.classList.remove("sldr-flavor-panel-open");
    flavorPanel.setAttribute("aria-hidden", "true");
  }

  function updateToolbarDark() {
    if (darkBtn) {
      darkBtn.innerHTML = isDark ? getSunIcon() : getMoonIcon();
    }
  }

  function updateToolbarFlavor() {
    if (flavorBtn) {
      flavorBtn.innerHTML = getPaletteIcon() + '<span class="sldr-flavor-label">' + escapeHtml(activeFlavor) + "</span>";
    }
    if (flavorPanel) {
      var items = flavorPanel.querySelectorAll(".sldr-flavor-item");
      for (var i = 0; i < items.length; i++) {
        items[i].classList.toggle("sldr-flavor-active", items[i].dataset.flavor === activeFlavor);
      }
    }
  }

  // Close flavor panel on outside click
  function onDocumentClick() {
    if (flavorPanelOpen) closeFlavorPanel();
  }

  // ---------------------------------------------------------------------------
  // SVG icons (inline, no external deps)
  // ---------------------------------------------------------------------------
  function getSunIcon() {
    return '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>';
  }

  function getMoonIcon() {
    return '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>';
  }

  function getPaletteIcon() {
    return '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r="0.5" fill="currentColor"/><circle cx="17.5" cy="10.5" r="0.5" fill="currentColor"/><circle cx="8.5" cy="7.5" r="0.5" fill="currentColor"/><circle cx="6.5" cy="12" r="0.5" fill="currentColor"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.93 0 1.5-.67 1.5-1.5 0-.39-.15-.74-.39-1.04-.23-.29-.38-.63-.38-1.02 0-.83.67-1.5 1.5-1.5H16c3.31 0 6-2.69 6-6 0-5.17-4.36-8.94-10-8.94z"/></svg>';
  }

  function escapeHtml(s) {
    var div = document.createElement("div");
    div.textContent = s;
    return div.innerHTML;
  }

  // ---------------------------------------------------------------------------
  // Initialisation
  // ---------------------------------------------------------------------------
  function init() {
    // Init flavors and restore preferences
    initFlavors();
    restoreFlavor();
    restoreDarkMode();

    // Read initial slide from URL hash
    var hash = parseHash();
    if (hash >= 0 && hash < total) {
      current = hash;
    }

    // Mark first slide active
    showSlide(current, "none");

    // Create toolbar
    createToolbar();

    // Event listeners
    document.addEventListener("keydown", onKey);
    document.addEventListener("click", onDocumentClick);
    window.addEventListener("hashchange", onHashChange);
    window.addEventListener("resize", onResize);
    initTouch();
  }

  // ---------------------------------------------------------------------------
  // Navigation
  // ---------------------------------------------------------------------------
  function next() {
    if (overviewOpen || flavorPanelOpen) return;

    var maxSteps = getMaxClickStep(slides[current]);
    if (clickStep < maxSteps) {
      clickStep++;
      applyClickSteps(slides[current], clickStep);
      syncNotes();
      return;
    }

    if (current < total - 1) {
      goTo(current + 1);
    }
  }

  function prev() {
    if (overviewOpen || flavorPanelOpen) return;

    if (clickStep > 0) {
      clickStep--;
      applyClickSteps(slides[current], clickStep);
      syncNotes();
      return;
    }

    if (current > 0) {
      goTo(current - 1);
    }
  }

  function goTo(index) {
    if (index < 0 || index >= total || index === current) return;

    var dir = index > current ? "forward" : "backward";
    var prevIndex = current;
    current = index;
    clickStep = 0;

    showSlide(current, dir, prevIndex);
    updateHash();
    updateProgress();
    syncNotes();
  }

  // ---------------------------------------------------------------------------
  // Slide display
  // ---------------------------------------------------------------------------
  //
  // Animation cleanup uses setTimeout (not animationend) because rapid
  // navigation interrupts CSS animations, and `animationend` doesn't fire
  // on interrupted animations. The timeout is the source of truth; cleanups
  // are idempotent and safe to run multiple times.

  var TRANSITION_MS = 400; // matches base.css animation duration
  var pendingCleanups = [];
  var pendingTimers = [];

  function cleanupAllAnimations() {
    // Force-finish any pending animations before starting new ones.
    // This catches the case where rapid navigation outpaces the
    // transition duration: each new showSlide call resets all state.
    for (var i = 0; i < pendingCleanups.length; i++) {
      pendingCleanups[i]();
    }
    pendingCleanups = [];
    for (var j = 0; j < pendingTimers.length; j++) {
      clearTimeout(pendingTimers[j]);
    }
    pendingTimers = [];
  }

  function removeTransitionClasses(el) {
    el.classList.remove(
      "tr-fade-enter", "tr-fade-exit",
      "tr-slide-left-enter", "tr-slide-left-exit",
      "tr-slide-right-enter", "tr-slide-right-exit",
      "tr-none"
    );
  }

  function scheduleCleanup(fn) {
    pendingCleanups.push(fn);
    var timer = setTimeout(function () {
      fn();
      var ci = pendingCleanups.indexOf(fn);
      if (ci !== -1) pendingCleanups.splice(ci, 1);
      var ti = pendingTimers.indexOf(timer);
      if (ti !== -1) pendingTimers.splice(ti, 1);
    }, TRANSITION_MS + 50);
    pendingTimers.push(timer);
  }

  function forceHide(slide) {
    removeTransitionClasses(slide);
    slide.classList.remove("active");
    slide.setAttribute("aria-hidden", "true");
  }

  function showSlide(index, dir, prevIndex) {
    // Clean up any in-flight animations first to prevent stuck states.
    cleanupAllAnimations();

    // Hide every slide except the one entering (and the prev slide if
    // it's being animated out). cleanupAllAnimations above guarantees
    // that any previously-mid-animation slides are now in clean state,
    // so this loop won't re-trigger animations.
    for (var i = 0; i < total; i++) {
      if (i === index) continue;
      if (dir !== "none" && i === prevIndex && prevIndex !== index) continue;
      forceHide(slides[i]);
    }

    // Enter
    var enterSlide = slides[index];
    removeTransitionClasses(enterSlide);
    enterSlide.classList.add("active");
    enterSlide.removeAttribute("aria-hidden");

    if (dir !== "none") {
      var enterClass = getTransitionClass(dir, "enter");
      enterSlide.classList.add(enterClass);
      scheduleCleanup(function () {
        removeTransitionClasses(enterSlide);
      });
    }

    applyClickSteps(enterSlide, 0);

    // Exit (cross-fade with enter)
    if (dir !== "none" && prevIndex !== undefined && prevIndex !== index) {
      var exitSlide = slides[prevIndex];
      removeTransitionClasses(exitSlide);
      // exitSlide must remain visible (active, no aria-hidden) for the
      // duration of the exit animation. The timeout below restores its
      // hidden state after the animation duration.
      exitSlide.classList.add("active");
      exitSlide.removeAttribute("aria-hidden");
      var exitClass = getTransitionClass(dir, "exit");
      exitSlide.classList.add(exitClass);
      scheduleCleanup(function () {
        forceHide(exitSlide);
      });
    }
  }

  function getTransitionClass(dir, phase) {
    if (transition === "none") return "tr-none";
    if (transition === "fade") return "tr-fade-" + phase;
    if (transition === "slide-left") {
      if (dir === "forward") return "tr-slide-left-" + phase;
      return "tr-slide-right-" + phase;
    }
    if (transition === "slide-right") {
      if (dir === "forward") return "tr-slide-right-" + phase;
      return "tr-slide-left-" + phase;
    }
    return "tr-fade-" + phase;
  }

  // ---------------------------------------------------------------------------
  // Click-through builds (data-click-step)
  // ---------------------------------------------------------------------------
  function getMaxClickStep(slide) {
    var max = 0;
    var els = slide.querySelectorAll("[data-click-step]");
    for (var i = 0; i < els.length; i++) {
      var step = parseInt(els[i].dataset.clickStep, 10);
      if (step > max) max = step;
    }
    return max;
  }

  function applyClickSteps(slide, step) {
    var els = slide.querySelectorAll("[data-click-step]");
    for (var i = 0; i < els.length; i++) {
      var elStep = parseInt(els[i].dataset.clickStep, 10);
      if (elStep <= step) {
        els[i].classList.add("sldr-visible");
        els[i].classList.remove("sldr-hidden");
      } else {
        els[i].classList.remove("sldr-visible");
        els[i].classList.add("sldr-hidden");
      }
    }
  }

  // ---------------------------------------------------------------------------
  // Progress bar & page number
  // ---------------------------------------------------------------------------
  function updateProgress() {
    if (progress) {
      var pct = total > 1 ? (current / (total - 1)) * 100 : 100;
      progress.style.width = pct + "%";
    }
    if (pageNum) {
      pageNum.textContent = current + 1 + " / " + total;
    }
  }

  // ---------------------------------------------------------------------------
  // URL hash routing
  // ---------------------------------------------------------------------------
  function parseHash() {
    var m = window.location.hash.match(/^#\/?(\d+)$/);
    if (m) return parseInt(m[1], 10) - 1;
    return -1;
  }

  function updateHash() {
    var newHash = "#/" + (current + 1);
    if (window.location.hash !== newHash) {
      history.replaceState(null, "", newHash);
    }
  }

  function onHashChange() {
    var hash = parseHash();
    if (hash >= 0 && hash < total && hash !== current) {
      goTo(hash);
    }
  }

  // ---------------------------------------------------------------------------
  // Keyboard handling
  // ---------------------------------------------------------------------------
  function onKey(e) {
    var tag = e.target.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || e.target.isContentEditable) {
      return;
    }

    switch (e.key) {
      case "ArrowRight":
      case "ArrowDown":
      case " ":
      case "Enter":
        e.preventDefault();
        if (overviewOpen) return;
        if (flavorPanelOpen) { closeFlavorPanel(); return; }
        next();
        break;

      case "ArrowLeft":
      case "ArrowUp":
      case "Backspace":
        e.preventDefault();
        if (overviewOpen) return;
        if (flavorPanelOpen) { closeFlavorPanel(); return; }
        prev();
        break;

      case "Home":
        e.preventDefault();
        goTo(0);
        break;

      case "End":
        e.preventDefault();
        goTo(total - 1);
        break;

      case "o":
      case "O":
        e.preventDefault();
        toggleOverview();
        break;

      case "s":
      case "S":
        e.preventDefault();
        openSpeakerNotes();
        break;

      case "f":
      case "F":
        e.preventDefault();
        toggleFullscreen();
        break;

      case "d":
      case "D":
        e.preventDefault();
        toggleDarkMode();
        break;

      case "e":
      case "E":
        if (!e.ctrlKey && !e.metaKey) {
          e.preventDefault();
          toggleEditMode();
        }
        break;

      case "t":
      case "T":
        e.preventDefault();
        if (flavorNames.length > 1) toggleFlavorPanel();
        break;

      case "l":
      case "L":
        e.preventDefault();
        cycleLanguage();
        break;

      case "m":
      case "M":
        e.preventDefault();
        toggleMotion();
        break;

      case "Escape":
        if (flavorPanelOpen) {
          e.preventDefault();
          closeFlavorPanel();
        } else if (overviewOpen) {
          e.preventDefault();
          toggleOverview();
        }
        break;

      default:
        break;
    }
  }

  // ---------------------------------------------------------------------------
  // Touch gestures
  // ---------------------------------------------------------------------------
  function initTouch() {
    var startX = 0;
    var startY = 0;
    var threshold = 50;

    deck.addEventListener(
      "touchstart",
      function (e) {
        if (overviewOpen) return;
        var touch = e.touches[0];
        startX = touch.clientX;
        startY = touch.clientY;
      },
      { passive: true }
    );

    deck.addEventListener(
      "touchend",
      function (e) {
        if (overviewOpen) return;
        var touch = e.changedTouches[0];
        var dx = touch.clientX - startX;
        var dy = touch.clientY - startY;

        if (Math.abs(dx) > Math.abs(dy) && Math.abs(dx) > threshold) {
          if (dx < 0) {
            next();
          } else {
            prev();
          }
        }
      },
      { passive: true }
    );
  }

  // ---------------------------------------------------------------------------
  // Overview grid
  // ---------------------------------------------------------------------------
  function toggleOverview() {
    if (!overlay) {
      overlay = createOverlay();
      document.body.appendChild(overlay);
    }

    overviewOpen = !overviewOpen;

    if (overviewOpen) {
      populateOverview();
      overlay.classList.add("sldr-overview-open");
      overlay.setAttribute("aria-hidden", "false");
    } else {
      overlay.classList.remove("sldr-overview-open");
      overlay.setAttribute("aria-hidden", "true");
    }
  }

  function createOverlay() {
    var el = document.createElement("div");
    el.className = "sldr-overview";
    el.setAttribute("aria-hidden", "true");
    el.setAttribute("role", "dialog");
    el.setAttribute("aria-label", "Slide overview");
    return el;
  }

  function populateOverview() {
    overlay.innerHTML = "";

    var grid = document.createElement("div");
    grid.className = "sldr-overview-grid";

    for (var i = 0; i < total; i++) {
      var thumb = document.createElement("button");
      thumb.className = "sldr-overview-thumb";
      if (i === current) thumb.classList.add("sldr-overview-current");
      thumb.dataset.index = i;
      thumb.setAttribute("aria-label", "Go to slide " + (i + 1));

      var clone = slides[i].cloneNode(true);
      clone.classList.remove("active");
      clone.removeAttribute("aria-hidden");
      clone.style.position = "relative";
      clone.style.display = "flex";

      var wrapper = document.createElement("div");
      wrapper.className = "sldr-overview-content";
      wrapper.appendChild(clone);

      var label = document.createElement("span");
      label.className = "sldr-overview-label";
      label.textContent = i + 1;

      thumb.appendChild(wrapper);
      thumb.appendChild(label);

      thumb.addEventListener("click", onThumbClick);
      grid.appendChild(thumb);
    }

    overlay.appendChild(grid);
  }

  function onThumbClick(e) {
    var btn = e.currentTarget;
    var index = parseInt(btn.dataset.index, 10);
    toggleOverview();
    goTo(index);
  }

  // ---------------------------------------------------------------------------
  // Speaker notes
  // ---------------------------------------------------------------------------
  function openSpeakerNotes() {
    if (notesWin && !notesWin.closed) {
      notesWin.focus();
      syncNotes();
      return;
    }

    notesWin = window.open("", "sldr-notes", "width=500,height=400");
    if (!notesWin) return;

    notesWin.document.write(
      "<!DOCTYPE html><html><head>" +
        '<meta charset="UTF-8">' +
        "<title>Speaker Notes</title>" +
        "<style>" +
        "body { font-family: system-ui, sans-serif; padding: 24px; " +
        "background: #1a1a2e; color: #e0e0e0; line-height: 1.6; }" +
        "h2 { color: #6366f1; margin: 0 0 8px; font-size: 14px; }" +
        "#notes { font-size: 16px; white-space: pre-wrap; }" +
        "#timer { position: fixed; top: 12px; right: 16px; " +
        "font-size: 24px; font-variant-numeric: tabular-nums; color: #888; }" +
        "</style></head><body>" +
        '<div id="timer">00:00</div>' +
        '<h2 id="slide-info"></h2>' +
        '<div id="notes"></div>' +
        "<script>" +
        "var start = Date.now();" +
        "setInterval(function() {" +
        "  var s = Math.floor((Date.now() - start) / 1000);" +
        "  var m = Math.floor(s / 60); s = s % 60;" +
        '  document.getElementById("timer").textContent = ' +
        '    String(m).padStart(2, "0") + ":" + String(s).padStart(2, "0");' +
        "}, 1000);" +
        "</" +
        "script></body></html>"
    );
    notesWin.document.close();

    syncNotes();
  }

  function syncNotes() {
    if (!notesWin || notesWin.closed) return;

    var slide = slides[current];
    var notesEl = slide.querySelector(".sldr-notes");
    var text = notesEl ? notesEl.innerHTML : "<em>No notes for this slide.</em>";

    var infoEl = notesWin.document.getElementById("slide-info");
    var notesDiv = notesWin.document.getElementById("notes");
    if (infoEl) infoEl.textContent = "Slide " + (current + 1) + " / " + total;
    if (notesDiv) notesDiv.innerHTML = text;
  }

  // ---------------------------------------------------------------------------
  // Fullscreen
  // ---------------------------------------------------------------------------
  function toggleFullscreen() {
    if (!document.fullscreenElement) {
      (deck.requestFullscreen || deck.webkitRequestFullscreen || function () {}).call(deck);
    } else {
      (document.exitFullscreen || document.webkitExitFullscreen || function () {}).call(document);
    }
  }

  // ---------------------------------------------------------------------------
  // Resize handling
  // ---------------------------------------------------------------------------
  function onResize() {
    // CSS handles scaling. Hook for future enhancements.
  }

  // ---------------------------------------------------------------------------
  // Edit mode (contenteditable inline editing)
  // ---------------------------------------------------------------------------
  var editMode = false;
  var editToolbar = null;

  function toggleEditMode() {
    editMode = !editMode;

    // Toggle contenteditable on all slide content areas
    for (var i = 0; i < slides.length; i++) {
      var contentAreas = slides[i].querySelectorAll(
        ".sldr-content, .sldr-left, .sldr-right, .sldr-heading"
      );
      for (var j = 0; j < contentAreas.length; j++) {
        contentAreas[j].contentEditable = editMode ? "true" : "false";
        contentAreas[j].classList.toggle("sldr-editable", editMode);
      }
    }

    if (editMode) {
      document.body.classList.add("sldr-edit-mode");
      if (!editToolbar) editToolbar = createEditToolbar();
      editToolbar.classList.add("sldr-edit-toolbar-visible");
    } else {
      document.body.classList.remove("sldr-edit-mode");
      if (editToolbar) editToolbar.classList.remove("sldr-edit-toolbar-visible");
    }
  }

  function createEditToolbar() {
    var bar = document.createElement("div");
    bar.className = "sldr-edit-toolbar";

    var label = document.createElement("span");
    label.className = "sldr-edit-label";
    label.textContent = "EDIT MODE";
    bar.appendChild(label);

    var actions = [
      { cmd: "bold", icon: "B", title: "Bold (Ctrl+B)" },
      { cmd: "italic", icon: "I", title: "Italic (Ctrl+I)" },
      { cmd: "underline", icon: "U", title: "Underline (Ctrl+U)" },
    ];

    for (var i = 0; i < actions.length; i++) {
      var btn = document.createElement("button");
      btn.className = "sldr-edit-btn";
      btn.setAttribute("title", actions[i].title);
      btn.textContent = actions[i].icon;
      btn.dataset.cmd = actions[i].cmd;
      btn.addEventListener("mousedown", function (ev) {
        ev.preventDefault(); // Keep focus on editable area
        document.execCommand(ev.currentTarget.dataset.cmd, false, null);
      });
      bar.appendChild(btn);
    }

    // Separator
    var sep = document.createElement("span");
    sep.className = "sldr-edit-sep";
    bar.appendChild(sep);

    // Save/Download button
    var saveBtn = document.createElement("button");
    saveBtn.className = "sldr-edit-btn sldr-edit-save";
    saveBtn.setAttribute("title", "Download modified HTML (Ctrl+S)");
    saveBtn.textContent = "Save";
    saveBtn.addEventListener("click", function (e) {
      e.preventDefault();
      downloadModifiedHtml();
    });
    bar.appendChild(saveBtn);

    // Close edit mode button
    var closeBtn = document.createElement("button");
    closeBtn.className = "sldr-edit-btn sldr-edit-close";
    closeBtn.setAttribute("title", "Exit edit mode (E)");
    closeBtn.textContent = "Exit";
    closeBtn.addEventListener("click", function (e) {
      e.preventDefault();
      toggleEditMode();
    });
    bar.appendChild(closeBtn);

    document.body.appendChild(bar);
    return bar;
  }

  function downloadModifiedHtml() {
    // Turn off edit mode temporarily to get clean HTML
    var wasEditing = editMode;
    if (wasEditing) {
      // Remove contenteditable attributes for export
      for (var i = 0; i < slides.length; i++) {
        var areas = slides[i].querySelectorAll("[contenteditable]");
        for (var j = 0; j < areas.length; j++) {
          areas[j].removeAttribute("contenteditable");
          areas[j].classList.remove("sldr-editable");
        }
      }
    }

    // Remove edit toolbar and mode class temporarily
    document.body.classList.remove("sldr-edit-mode");
    if (editToolbar) editToolbar.classList.remove("sldr-edit-toolbar-visible");

    // Get the full document HTML
    var html = "<!DOCTYPE html>\n" + document.documentElement.outerHTML;

    // Restore edit mode
    if (wasEditing) {
      document.body.classList.add("sldr-edit-mode");
      if (editToolbar) editToolbar.classList.add("sldr-edit-toolbar-visible");
      for (var k = 0; k < slides.length; k++) {
        var contentAreas = slides[k].querySelectorAll(
          ".sldr-content, .sldr-left, .sldr-right, .sldr-heading"
        );
        for (var l = 0; l < contentAreas.length; l++) {
          contentAreas[l].contentEditable = "true";
          contentAreas[l].classList.add("sldr-editable");
        }
      }
    }

    // Download
    var blob = new Blob([html], { type: "text/html" });
    var url = URL.createObjectURL(blob);
    var a = document.createElement("a");
    a.href = url;
    a.download = document.title.replace(/[^a-zA-Z0-9_-]/g, "_") + ".html";
    a.click();
    URL.revokeObjectURL(url);
  }

  // Intercept Ctrl+S in edit mode for save
  document.addEventListener("keydown", function (e) {
    if (editMode && (e.ctrlKey || e.metaKey) && e.key === "s") {
      e.preventDefault();
      downloadModifiedHtml();
    }
  });

  // ---------------------------------------------------------------------------
  // Boot
  // ---------------------------------------------------------------------------
  updateProgress();
  init();
})();
