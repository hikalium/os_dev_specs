

default: download
	cargo build
	cargo run -- `readlink -f data.md`

.PHONY : download

download:
	./download.sh

run : default
	open index.html
