
all:
	cargo build

test:
	cargo test


clean:
	cargo clean

watch:
	while find src/ -print0 | xargs -0 inotifywait -e delete_self -e modify ; do \
		echo "============ at `date` ==========" ; \
		make ; \
	done

run:
	cargo run --bin bender

run_hello:
	cargo run --bin hello

.PHONY: all clean test watch

