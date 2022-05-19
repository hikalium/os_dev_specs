default:
	cargo build

.PHONY : default download run watch

download:
	./download.sh

run :
	cargo run -- `readlink -f data.md`
	open index.html

watch :
	cargo run -- `readlink -f data.md`
