import { mount } from "svelte";
import "./app.css";
import "@fontsource/inter/400.css";
import "@fontsource/inter/500.css";
import "@fontsource/inter/600.css";
import "@fontsource/inter/700.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/600.css";
import App from "./App.svelte";

const target = document.getElementById("app");
if (!target) {
  throw new Error("AnimaBooter: #app mount point missing from index.html");
}

const app = mount(App, { target });

export default app;
