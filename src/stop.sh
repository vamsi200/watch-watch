#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="/opt/kafka_2.13-4.2.0"
CONFIG="$ROOT_DIR/config/server.properties"

log() {
  echo "[INFO] $1"
}

stop_kafka() {
  log "Stopping Kafka..."

  if pgrep -f "kafka.Kafka" >/dev/null; then
    "$ROOT_DIR/bin/kafka-server-stop.sh" "$CONFIG" || true

    for i in {1..10}; do
      if ! pgrep -f "kafka.Kafka" >/dev/null; then
        log "Kafka stopped gracefully"
        return
      fi
      sleep 1
    done

    log "Kafka did not stop gracefully, killing..."
    pkill -f "kafka.Kafka" || true
  else
    log "Kafka not running"
  fi
}

stop_container() {
  local name="$1"

  if docker ps --format '{{.Names}}' | grep -q "^$name$"; then
    log "Stopping $name..."
    docker stop "$name" >/dev/null
  else
    log "$name already stopped"
  fi
}

stop_ui() {
  log "Stopping UI stack..."

  stop_container "thirsty_williamson"
  stop_container "kafka-connect"
  stop_container "elastic"
  stop_container "kibana"
}

main() {
  stop_kafka
  stop_ui
}

main "$@"
