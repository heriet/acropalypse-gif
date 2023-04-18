# aCropalypse gif

aCropalypse（CVE-2023-21036）related GIF PoC. The aCropalypse reported affects PNG, but a similar exploit exists in GIF images.

## PoC

This is original GIF Image `case02.gif`.

![Original GIF](asset/case02.gif)

And crop image `case02_crop.gif` using aCropalypse affected software.

![Crop GIF Image](asset/case02_crop.gif)

Execute `acropalypse-gif restore`

```
$ acropalypse-gif restore asset/case02_crop.gif 688 634
asset/case02_crop.gif-restored.gif
```

![Restore GIF Image](asset/case02_crop.gif-restored.gif)


## Build

```sh
cargo build --release
```

## Usage

### detect

Detect aCropalypse gif image.

```sh
acropalypse-gif detect <filepath>
```

```sh
# undetected
$ acropalypse-gif detect asset/case01.gif

# detected
$ acropalypse-gif detect asset/case01_crop.gif
asset/case01_crop.gif
```

### restore

Restore partical gif image from aCropalypse affected gif image.

```sh
acropalypse-gif restore <filepath> <width> <height>
```

```sh
# restore case01.gif
$ acropalypse-gif restore asset/case01_crop.gif 619 232
asset/case01_crop.gif-restored.gif
```

## Run on docker

```sh
docker compose run --rm acropalypse-gif detect asset/case01_crop.gif
docker compose run --rm acropalypse-gif restore asset/case01_crop.gif 619 232
```

## Exploit Detail

- [Exploit Detail](doc/exploit_detail.md)
  - [Exploit Detail (Japanese)](doc/exploit_detail_ja.md)

### Algorithm

- Search Image Data Sub-block. It may be start `0xFF` and verify GIF Image Data format.
- Search 12bit Clear Code of LZW compression(GIF). It may be `0b000100000000`.
- Decompress LZW after Clear Code.
- Encode image using cropped image palette.
