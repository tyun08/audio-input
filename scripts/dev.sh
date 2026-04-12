#!/bin/sh
PORT=${PORT:-1420}
exec npx tauri dev --config "{\"build\":{\"devUrl\":\"http://localhost:${PORT}\",\"beforeDevCommand\":\"PORT=${PORT} npm run dev\"}}"
