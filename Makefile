.PHONY: help searxng-up searxng-down searxng-logs searxng-restart searxng-init

SEARXNG_CONTAINER := searxng

help:
	@echo "SearXNG Setup Commands:"
	@echo "  make searxng-init     - Create config/data directories and start SearXNG"
	@echo "  make searxng-up      - Start SearXNG via Docker"
	@echo "  make searxng-down    - Stop SearXNG container"
	@echo "  make searxng-logs    - View SearXNG logs"
	@echo "  make searxng-restart - Restart SearXNG container"

searxng-init:
	@echo "Creating SearXNG directories..."
	mkdir -p ./searxng/config/ ./searxng/data/
	@echo "Starting SearXNG with host network..."
	docker run --name $(SEARXNG_CONTAINER) -d \
		--network host \
		-v "./searxng/config:/etc/searxng" \
		-v "./searxng/data:/var/cache/searxng" \
		-e SEARXNG_SECRET=changeme \
		docker.io/searxng/searxng:latest

searxng-up:
	@echo "Starting SearXNG with host network..."
	docker start $(SEARXNG_CONTAINER) 2>/dev/null || \
	(docker run --name $(SEARXNG_CONTAINER) -d \
		--network host \
		-v "./searxng/config:/etc/searxng" \
		-v "./searxng/data:/var/cache/searxng" \
		-e SEARXNG_SECRET=changeme \
		docker.io/searxng/searxng:latest)

searxng-down:
	docker stop $(SEARXNG_CONTAINER) 2>/dev/null || true
	docker rm $(SEARXNG_CONTAINER) 2>/dev/null || true

searxng-logs:
	docker logs -f $(SEARXNG_CONTAINER)

searxng-restart: searxng-down searxng-up
