release tag:
    git tag "{{tag}}" HEAD
    git push origin "refs/tags/{{tag}}"

# Dev — single command: builds Docker image, starts postgres + app, seeds test data
dev:
    docker compose -f docker-compose.dev.yml up -d --build
    @echo "Waiting for app to be ready..."
    @until curl -sf http://localhost:3000 > /dev/null 2>&1; do sleep 1; done
    ./scripts/seed-dev.sh
    @echo "Ready at http://localhost:3000"

# Stop dev containers
dev-down:
    docker compose -f docker-compose.dev.yml down

# Quick local run (requires postgres already running)
run:
    source .envrc && cargo run

# Start postgres only for native dev
dev-db:
    docker compose -f docker-compose.dev.yml up -d postgres

# Run app natively with auto-reload (postgres must be running separately)
dev-native:
    source .envrc && cargo watch -x run