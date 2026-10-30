#!/bin/sh
# No snap, XDG_RUNTIME_DIR é /run/user/<uid>/snap.feathersheet; o socket do Wayland
# fica um nível acima. O libwayland aceita caminho absoluto no WAYLAND_DISPLAY.
mkdir -p "$XDG_RUNTIME_DIR"
case "$WAYLAND_DISPLAY" in
  ""|/*) ;;
  *) export WAYLAND_DISPLAY="$(dirname "$XDG_RUNTIME_DIR")/$WAYLAND_DISPLAY" ;;
esac
exec "$@"
