.PHONY: help searxng-up searxng-down searxng-logs searxng-restart searxng-init

SEARXNG_CONTAINER := searxng
SEARXNG_PORT := 8888

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
	@echo "Starting SearXNG on port $(SEARXNG_PORT)..."
	docker run --name $(SEARXNG_CONTAINER) -d \
		-p $(SEARXNG_PORT):8080 \
		-v "./searxng/config/:/etc/searxng/" \
		-v "./searxng/data/:/var/cache/searxng/" \
		docker.io/searxng/searxng:latest

searxng-up:
	@echo "Starting SearXNG on port $(SEARXNG_PORT)..."
	docker start $(SEARXNG_CONTAINER) 2>/dev/null || \
	(docker run --name $(SEARXNG_CONTAINER) -d \
		-p $(SEARXNG_PORT):8080 \
		-v "./searxng/config/:/etc/searxng/" \
		-v "./searxng/data/:/var/cache/searxng/" \
		docker.io/searxng/searxng:latest)

searxng-down:
	docker stop $(SEARXNG_CONTAINER) 2>/dev/null || true
	docker rm $(SEARXNG_CONTAINER) 2>/dev/null || true

searxng-logs:
	docker logs -f $(SEARXNG_CONTAINER)

searxng-restart: searxng-down searxng-up
