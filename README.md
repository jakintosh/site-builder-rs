# Site Builder

This is a tiny personal static site generator. It started as a bash script, but I quickly realized that I hate bash, and that all the command line utils I was using were rust binaries anyway, so I just ported (and refined) the script in Rust. It mostly became a project for me to build a command line binary project in Rust, and to also practice Rust in a straightforward project in general.

This binary will be used to generate https://jakintosh.com and probably also eventually https://coalescent.computer.

This project relies on:
- [Tera](https://github.com/Keats/tera) — for templating
- [Pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) — for parsing markdown -> html
- [Clap](https://github.com/clap-rs/clap) — for easy CLI setup
- [Anyhow](https://github.com/dtolnay/anyhow)/[Thiserror](https://github.com/dtolnay/thiserror) — for nice error management
- [Blake2](https://github.com/RustCrypto/hashes/tree/master/blake2) — to create content hashes for permalinks