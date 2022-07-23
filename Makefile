default: run

.PHONY : default download run watch

download:
	./download.sh

run :
	cargo run -- `readlink -f data.md`

watch :
	cargo run -- --watch `readlink -f data.md`
