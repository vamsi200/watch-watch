#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="/opt/kafka_2.13-4.2.0"
CONFIG="$ROOT_DIR/config/server.properties"
CLUSTER_ID="wy4VTF0oTCq_In1D4VsJ0Q"

log() {
  echo "[INFO] $1"
}

start_kafka() {
  log "Starting Kafka..."

  if [ ! -f "/tmp/kraft-combined-logs/meta.properties" ]; then
    log "Formatting Kafka storage..."
    "$ROOT_DIR/bin/kafka-storage.sh" format \
      -t "$CLUSTER_ID" \
      --standalone \
      -c "$CONFIG"
  else
    log "Kafka already formatted, skipping..."
  fi

  log "Launching Kafka server..."
  "$ROOT_DIR/bin/kafka-server-start.sh" "$CONFIG"
}

start_container() {
  local name="$1"

  if docker ps --format '{{.Names}}' | grep -q "^$name$"; then
    log "$name already running"
  else
    log "Starting $name..."
    docker start "$name" >/dev/null
  fi
}

start_ui() {
  log "Starting UI stack..."

  start_container "thirsty_williamson"
  start_container "kafka-connect"
  start_container "elastic"
  start_container "kibana"
}

main() {
  start_ui
  start_kafka
}

main "$@"
