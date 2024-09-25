# cheru.dev v4



to build:
```shell
curl https://download.geonames.org/export/dump/allCountries.zip -o allCountries.zip
unzip allCountries.zip -d rust-site/src/include/
cargo build --release
```

to run:
```shell
./target/release/v4
```

