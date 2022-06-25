# os_dev_specs

Index of OS dev related specifications / datasheets.

[data.md](./data.md) contains all the info needed to retrieve the pdfs and indexes.

[index.html](./index.html) is generated from `data.md`, which contains links to jump specific pages in the specs.

[spec/index.txt](spec/index.txt) is a list of hashes of the pdf to detect updates (automatically generated via `make download`).


## How to add entries
1. Edit `data.md`
2. Run `make run`

Also, you can `make watch` to monitor the changes on data.md and update the html automagically ;)

## How to replicate the library
1. Clone this repo
1. Run `make download`

# License
MIT
