# Starling Server — build & run helpers
#
# Usage:
#   just build              # build the library and binary
#   just create NAME        # create a new roost
#   just open NAME          # start a headless roost server
#   just check              # cargo check

build:
    cargo build

create name:
    cargo run -- roost create {{name}}

open name:
    cargo run -- roost open {{name}}

check:
    cargo check
