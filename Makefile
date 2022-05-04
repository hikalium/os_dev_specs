

default: download
	cargo build
	cargo run > index.html

.PHONY : download

download:
	./download.sh

run : default
	open index.html
