# arecibo

> [!WARNING]
> arecibo uses the bittorrent protocol to download metadata about torrents from peers. this *shouldnt* trigger angry letters from your ISP but you should use a VPN as backup as overzealous monitoring might think you are downloading copyrighted material.

A simple service that wraps [rqbit](https://github.com/ikatson/rqbit) to get information about a torrent from its hash.

Uses DHT and a hard-coded list of trackers to find peers.

## installation

`docker run -v arecibo-data:/data --rm -it -p 3080:3080 sylver/arecibo`

> [!TIP]
> - Setting `DHT_QUERIES_PER_SECOND` to a higher number (~1000) will let you scan more torrents at once but may have other negative impacts.
> - Using `network_mode: host` may let you talk to more peers

## usage

### `GET localhost:3080/torrent/<hash>/metadata`

Gets metadata about the torrent.

<details>

<summary>Multi-file response</summary>

```json
{
    "name": "Big Buck Bunny",
    "size": 276445467,
    "created_at": null,
    "files": [
        {
            "path": ["Big Buck Bunny/Big Buck Bunny.en.srt"],
            "size": 140
        },
        {
            "path": ["Big Buck Bunny/Big Buck Bunny.mp4"],
            "size": 276134947
        },
        {
            "path": ["Big Buck Bunny/poster.jpg"],
            "size": 310380
        }
    ]
}
```

</details>

<details>

<summary>Single-file response</summary>

```json
{
    "name": "ubuntu-25.04-desktop-amd64.iso",
    "size": 6278520832,
    "created_at": null,
    "files": [
        {
            "path": ["ubuntu-25.04-desktop-amd64.iso"],
            "size": 6278520832
        }
    ]
}
```
</details>


### `GET localhost:3080/torrent/<hash>/file`

Download the .torrent file