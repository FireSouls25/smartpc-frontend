import "./lib/theme.css";
import { mount } from "svelte";
import App from "./app/App.svelte";

const el = document.getElementById("app");
if (!el) throw new Error("#app element missing");
mount(App, { target: el });
