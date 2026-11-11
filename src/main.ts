import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";
import { initAccent } from "./lib/accent";
import { initTheme } from "./lib/theme";

initTheme();
initAccent();

const target = document.getElementById("app")!;
mount(App, { target });
