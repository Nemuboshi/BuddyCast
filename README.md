# BuddyCast

Go command-line tool for listing, downloading, decrypting, and converting UDCast subtitle packages.

```console
go run ./cmd/buddycast getinfo --limit 20
go run ./cmd/buddycast getinfo --save downloads/contents.json
go run ./cmd/buddycast fetch <ASSET_ID> --srt
go run ./cmd/buddycast fetch <ASSET_ID> --offline
```

`fetch` shows dependency-free terminal progress bars while downloading, decrypting, and processing files. Build the standalone binary with:

```console
go build -trimpath -ldflags "-s -w" -o buddycast ./cmd/buddycast
```
