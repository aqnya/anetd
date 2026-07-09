/**
 * anetd WebUI — Svelte 5 entry point.
 * Mounts the App component into #app.
 */
import { mount } from "svelte";
import App from "./App.svelte";

mount(App, { target: document.getElementById("app")! });
