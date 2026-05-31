/* @refresh reload */
import { render } from "solid-js/web";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import SettingsApp from "./SettingsApp";

const windowLabel = getCurrentWindow().label;

render(
  () => (windowLabel === "settings" ? <SettingsApp /> : <App />),
  document.getElementById("root") as HTMLElement,
);
