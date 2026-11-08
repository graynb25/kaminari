(function () {
  try {
    var mode = localStorage.getItem("kaminari.theme") || "system";
    var light =
      mode === "light" ||
      (mode === "system" &&
        window.matchMedia("(prefers-color-scheme: light)").matches);
    document.documentElement.dataset.theme = light ? "light" : "dark";
  } catch (error) {
    document.documentElement.dataset.theme = "dark";
  }
})();
