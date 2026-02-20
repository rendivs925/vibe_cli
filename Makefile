.PHONY: help searxng-up searxng-down searxng-logs searxng-restart

help:
	@echo "SearXNG Setup Commands:"
	@echo "  make searxng-up       - Start SearXNG via Docker"
	@echo "  make searxng-down     - Stop SearXNG container"
	@echo "  make searxng-logs     - View SearXNG logs"
	@echo "  make searxng-restart  - Restart SearXNG container"

SEARXNG_CONTAINER := vibe-searxng
SEARXNG_PORT := 8085

searxng-up:
	@echo "Starting SearXNG on port $(SEARXNG_PORT)..."
	docker run -d \
		--name $(SEARXNG_CONTAINER) \
		-p $(SEARXNG_PORT):8085 \
		-e SEARXNG_BASE_URL=http://localhost:$(SEARXNG_PORT) \
		-e SEARXNG_SECRET=$(shell openssl rand -hex 32) \
		-v searxng-data:/etc/searxng \
		searxng/searxng:latest

searxng-down:
	docker stop $(SEARXNG_CONTAINER) 2>/dev/null || true
	docker rm $(SEARXNG_CONTAINER) 2>/dev/null || true

searxng-logs:
	docker logs -f $(SEARXNG_CONTAINER)

searxng-restart: searxng-down searxng-up

searxng-config:
	@echo "SearXNG config directory: ~/.config/searxng/"
	@echo ""
	@echo "To use with vibe_cli, set environment variable:"
	@echo "  export SEARXNG_URL=http://localhost:$(SEARXNG_PORT)"
