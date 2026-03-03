default: run

.PHONY : default download run watch

download: download_entries.generated.sh
	./download.sh

download_entries.generated.sh :
	cargo run -- `readlink -f data.md`

watch :
	cargo run -- --watch `readlink -f data.md`
