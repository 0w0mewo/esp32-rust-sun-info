#!/usr/bin/env bash
set -e

ESP="esp32c3"

declare -A TARGETS 
TARGETS["esp32"]="xtensa-esp32-none-elf"
TARGETS["esp32c3"]="riscv32imc-unknown-none-elf"

WIFI_SSID="SSID HERE" \
WIFI_PSWD="WIFI PASSWORD HERE" \
UPDATE_SEC="2" \
cargo build \
  --release \
  --no-default-features \
  --features "ws2812b ${ESP}" \
  --target "${TARGETS[${ESP}]}"