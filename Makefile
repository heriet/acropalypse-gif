bash:
	docker compose run --rm dev bash

build:
	docker compose run --rm dev cargo build --release

build-dev:
	docker compose run --rm dev cargo build

clippy:
	docker compose run --rm dev cargo clippy
