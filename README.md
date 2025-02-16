# arecibo

A simple service that wraps [rqbit](https://github.com/ikatson/rqbit) to get information about a torrent from its hash.

Uses DHT and a hard-coded list of trackers to find peers.

## installation

`docker run --rm --it -p 3080:3080 sylver/arecibo`

> [!TIP]
> - Setting `DHT_QUERIES_PER_SECOND` to a higher number (~1000) will let you scan more torrents at once but may have other negative impacts.
> - Setting `ARECIBO_DHT_FILE` to a persistent location may result in better DHT results
> - Using `network_mode: host` may let you talk to more peers

## usage

`GET localhost:3080/info/<hash>`


<details>

<summary>Response</summary>

```json
{
    "name": "Big Buck Bunny",
    "size": 276445467,
    "files": [
        {
            "path": ["Big Buck Bunny.en.srt"],
            "size": 140
        },
        {
            "path": ["Big Buck Bunny.mp4"],
            "size": 276134947
        },
        {
            "path": ["poster.jpg"],
            "size": 310380
        }
    ]
}
```

</details>
