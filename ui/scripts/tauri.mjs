// Runs the Tauri CLI against ../app (the Tauri project lives next to ui/, not inside it).
// Used instead of `TAURI_APP_PATH=../app tauri`, which does not work on Windows.
import process from "node:process";
import { URL, fileURLToPath } from "node:url";
import cli from "@tauri-apps/cli";

process.env.TAURI_APP_PATH ??= fileURLToPath(new URL("../../app", import.meta.url));

cli.run(process.argv.slice(2), "tauri").catch((err) => {
  cli.logError(err.message);
  process.exit(1);
});
