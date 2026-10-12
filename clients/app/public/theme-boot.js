// Applies the saved theme, accent and density to <html> before first paint,
// so a reload never flashes the wrong theme. A classic (non-module) script
// loaded from index.html: the web CSP allows `script-src 'self'` only, so this
// cannot be inline. Keep in step with src/lib/ui/theme/theme.ts and
// src/lib/storage/prefs.ts (theme-boot.test.ts checks both).
(function () {
  var prefix = "campbooks.pref.";
  var allowed = {
    theme: ["system", "light", "dark"],
    accent: ["business", "personal", "forest"],
    density: ["comfortable", "compact"],
  };
  function read(key) {
    try {
      return globalThis.localStorage.getItem(prefix + key);
    } catch {
      return null;
    }
  }
  function pick(name, value) {
    return allowed[name].includes(value) ? value : allowed[name][0];
  }
  var root = document.documentElement;
  var workspace = read("workspace") || "default";
  root.dataset.theme = pick("theme", read("theme"));
  root.dataset.accent = pick("accent", read("accent." + workspace));
  root.dataset.density = pick("density", read("density"));
})();
