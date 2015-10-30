
all:
	cargo build --release

test:
	cargo test


clean:
	cargo clean

.PHONY: all clean

watch:
	while find src/ -print0 | xargs -0 inotifywait -e delete_self -e modify ; do \
		echo "============ at `date` ==========" ; \
		make ; \
	done

