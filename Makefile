.PHONY: build release test doctor chat daemon fleet docker-up

build:
	cargo build

release:
	cargo build --release

test:
	cargo test -- --nocapture

doctor:
	cargo run -- doctor

chat:
	cargo run -- chat --agent hermes-main

daemon:
	cargo run -- daemon --bind 0.0.0.0:8080

fleet:
	cargo run -- fleet --benchmark-count 50

docker-up:
	docker compose up --build -d
