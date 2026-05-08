# BuddyCast

## Command Line Interface

### `buddy_cast getinfo`
Fetch the remote contents list and print filtered entries.

```bash
buddy_cast getinfo
```

Options:
- `--timeout <SECONDS>`: HTTP timeout for the contents request. Default: `60`
- `--save <PATH>`: Save the fetched `contents.json` to a local file
- `--limit <COUNT|all>`: Limit how many entries are printed. Default: `50`

Examples:

```bash
buddy_cast getinfo --limit 20
buddy_cast getinfo --limit all
buddy_cast getinfo --save downloads/contents.json
```

### `buddy_cast fetch <ASSET_ID>`
Fetch one package by asset id, decrypt it, extract it, and generate derived outputs.

```bash
buddy_cast fetch <ASSET_ID>
```

Options:
- `--timeout <SECONDS>`: HTTP timeout for the download request. Default: `60`
- `--out <PATH>`: Output directory. Default: `downloads`
- `--keep-encrypted-zip`: Keep the encrypted zip copy in the output directory
- `--srt`: Also render `.srt` subtitle files
- `--offline`: Read `downloads/contents.json` and `downloads/<ASSET_ID>.encrypted.zip` instead of using the network

Examples:

```bash
buddy_cast fetch <ASSET_ID>
buddy_cast fetch <ASSET_ID> --srt
buddy_cast fetch <ASSET_ID> --out ./downloads
buddy_cast fetch <ASSET_ID> --keep-encrypted-zip
buddy_cast fetch <ASSET_ID> --offline
```



## Typical Flow

```bash
buddy_cast getinfo --limit 50
buddy_cast fetch <ASSET_ID>
```

For offline usage:

```bash
buddy_cast getinfo --save downloads/contents.json
buddy_cast fetch <ASSET_ID> --offline
```
