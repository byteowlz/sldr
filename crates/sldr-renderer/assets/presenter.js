/**
 * sldr Presenter Engine
 *
 * Vanilla JS presentation controller embedded into self-contained HTML output.
 * Features: keyboard navigation, CSS transitions, speaker notes, overview grid,
 * touch gestures, fullscreen, progress bar, URL hash routing, dark mode toggle,
 * click-through builds (data-click-step).
 *
 * No external dependencies. Single IIFE, ~400 lines.
 */
(function () {
  "use strict";

  // ---------------------------------------------------------------------------
  // DOM references
  // ---------------------------------------------------------------------------
  var deck = document.querySelector(".sldr-deck");
  if (!deck) return;

  var slides = Array.from(deck.querySelectorAll(".sldr-slide"));
  var progress = document.querySelector(".sldr-progress");
  var pageNum = document.querySelector(".sldr-page-num");
  var overlay = null; // overview grid, created lazily
  var notesWin = null; // speaker notes window

  var total = slides.length;
  if (total === 0) return;

  // ---------------------------------------------------------------------------
  // State
  // ---------------------------------------------------------------------------
  var current = 0;
  var clickStep = 0; // current build step within a slide
  var overviewOpen = false;
  var transition = deck.dataset.transition || "fade";

  // ---------------------------------------------------------------------------
  // Initialisation
  // ---------------------------------------------------------------------------
  function init() {
    // Read initial slide from URL hash
    var hash = parseHash();
    if (hash >= 0 && hash < total) {
      current = hash;
    }

    // Mark first slide active
    showSlide(current, "none");

    // Event listeners
    document.addEventListener("keydown", onKey);
    window.addEventListener("hashchange", onHashChange);
    window.addEventListener("resize", onResize);
    initTouch();
  }

  // ---------------------------------------------------------------------------
  // Navigation
  // ---------------------------------------------------------------------------
  function next() {
    if (overviewOpen) return;

    // Check for click-through build steps
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
    if (overviewOpen) return;

    // Step backwards through click steps first
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
  function showSlide(index, dir, prevIndex) {
    for (var i = 0; i < total; i++) {
      var slide = slides[i];
      if (i === index) {
        slide.classList.add("active");
        slide.removeAttribute("aria-hidden");

        // Apply enter transition
        if (dir !== "none") {
          var enterClass = getTransitionClass(dir, "enter");
          slide.classList.add(enterClass);
          slide.addEventListener(
            "animationend",
            function handler() {
              slide.classList.remove(enterClass);
              slide.removeEventListener("animationend", handler);
            },
            { once: true }
          );
        }

        // Reset click steps to 0 (hide all build elements)
        applyClickSteps(slide, 0);
      } else {
        // Apply exit transition to previous slide
        if (dir !== "none" && i === prevIndex) {
          var exitClass = getTransitionClass(dir, "exit");
          slide.classList.add(exitClass);
          slide.addEventListener(
            "animationend",
            function exitHandler() {
              slide.classList.remove("active", exitClass);
              slide.setAttribute("aria-hidden", "true");
              slide.removeEventListener("animationend", exitHandler);
            },
            { once: true }
          );
        } else {
          slide.classList.remove("active");
          slide.setAttribute("aria-hidden", "true");
        }
      }
    }
  }

  function getTransitionClass(dir, phase) {
    // phase: "enter" or "exit"
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
    if (m) return parseInt(m[1], 10) - 1; // 1-indexed in URL, 0-indexed internally
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
    // Ignore if user is typing in an input/textarea/contenteditable
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
        next();
        break;

      case "ArrowLeft":
      case "ArrowUp":
      case "Backspace":
        e.preventDefault();
        if (overviewOpen) return;
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

      case "Escape":
        if (overviewOpen) {
          e.preventDefault();
          toggleOverview();
        }
        break;

      default:
        // Number key + Enter: go to slide N
        // We handle just the digit accumulation here
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

        // Only handle horizontal swipes (ignore vertical scroll-like gestures)
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

      // Clone slide content as thumbnail
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
    if (!notesWin) return; // popup blocked

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
  // Dark mode
  // ---------------------------------------------------------------------------
  function toggleDarkMode() {
    document.documentElement.classList.toggle("dark");
  }

  // ---------------------------------------------------------------------------
  // Resize handling (maintains aspect ratio)
  // ---------------------------------------------------------------------------
  function onResize() {
    // The CSS handles scaling via aspect-ratio + viewport units.
    // This hook is available for future enhancements.
  }

  // ---------------------------------------------------------------------------
  // Boot
  // ---------------------------------------------------------------------------
  updateProgress();
  init();
})();
