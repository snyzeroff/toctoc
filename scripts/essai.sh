#!/usr/bin/env bash
# Lance deux copies de test (profils a et b) avec l'inspection activée sur les ports 9331 et 9332.
cd "$(dirname "$0")/.."
for p in a b; do
  port=$([ "$p" = a ] && echo 9331 || echo 9332)
  TOCTOC_PROFILE=$p WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=$port ./src-tauri/target/debug/toctoc.exe "$@" &
done
