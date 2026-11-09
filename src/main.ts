import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";
import { initTheme } from "./lib/theme";

initTheme();

const target = document.getElementById("app")!;
const harness = new URLSearchParams(location.search).get("mpvHarness") === "1";

if (harness) {
  void import("./lib/MpvHarness.svelte").then(({ default: MpvHarness }) => {
    mount(MpvHarness, { target });
  });
} else {
  mount(App, { target });
}
