// Appels au cœur Rust de l'appli. Toute erreur est écrite dans le journal de l'appli
// (le même fichier que le cœur), jamais seulement dans la console du webview.
import { invoke } from "@tauri-apps/api/core";
import { error as logError, info as logInfo } from "@tauri-apps/plugin-log";

async function call(cmd, args) {
  try {
    return await invoke(cmd, args);
  } catch (e) {
    logError(`interface : ${cmd} a échoué : ${e}`);
    throw e;
  }
}

export const log = {
  info: (msg) => logInfo(`interface : ${msg}`),
  error: (msg) => logError(`interface : ${msg}`),
};

export const api = {
  state: () => call("get_state"),
  devices: () => call("list_devices"),
  setDevice: (device) => call("set_device", { device }),
  setLatency: (latencyMs) => call("set_latency", { latencyMs }),
  setMaster: (gain, muted) => call("set_master", { gain, muted }),
  setSource: (name, settings) => call("set_source", { name, settings }),
  setPrefs: (prefs) => call("set_prefs", { prefs }),
  getAutostart: () => call("get_autostart"),
  setAutostart: (enabled) => call("set_autostart", { enabled }),
  openLogs: () => call("open_logs"),
  openRepo: () => call("open_repo"),
  updateCheck: () => call("update_check"),
  updateInstall: () => call("update_install"),
};
