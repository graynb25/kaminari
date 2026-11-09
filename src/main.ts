import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";
import { initAccent } from "./lib/accent";
import { initTheme } from "./lib/theme";

initTheme();
initAccent();

const target = document.getElementById("app")!;
const params = new URLSearchParams(location.search);
const harness = params.get("mpvHarness") === "1";
const perfHarness = params.get("perfHarness") === "1";

if (perfHarness) {
  (window as unknown as { __perfStart?: number }).__perfStart = performance.now();
  void import("./lib/PerfHarness.svelte").then(({ default: PerfHarness }) => {
    mount(PerfHarness, { target });
  });
} else if (harness) {
  void import("./lib/MpvHarness.svelte").then(({ default: MpvHarness }) => {
    mount(MpvHarness, { target });
  });
} else {
  mount(App, { target });
}
