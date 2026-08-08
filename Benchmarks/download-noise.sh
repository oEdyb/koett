#!/bin/sh

set -eu

script_directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
noise_directory="$script_directory/Noise"
mkdir -p "$noise_directory"

download() {
    filename=$1
    curl --fail --location --retry 3 \
        --output "$noise_directory/$filename" \
        "https://raw.githubusercontent.com/microsoft/MS-SNSD/master/noise_test/$filename"
}

download AirConditioner_1.wav
download Babble_1.wav
download Neighbor_1.wav
download Typing_1.wav

cd "$noise_directory"
shasum -a 256 -c <<'CHECKSUMS'
3fe55d7035d7ef2599841738f818dc4463f5ef983eb4b5fefeee153d8caa2110  AirConditioner_1.wav
d7a64dd995a92f4790308dbff196332585eec502734ac737bfc9e2ac317dacd6  Babble_1.wav
61021e8be14dd19d6a7ae54f8b9572597cb1e9a5258aaeb6ba8b1ef6ce26cda0  Neighbor_1.wav
47980660888977fb45e5cdfd4a14d6585139e90267c0e61a01eeadb4a5891222  Typing_1.wav
CHECKSUMS
