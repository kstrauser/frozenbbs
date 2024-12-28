bbscmd := "target/debug/frozenbbs"
cfgfile := "cargo run config config-path"
dbfile := "cargo run config db-path"

# Install dev requirements
setup:
    #!/bin/bash

    if which -s diesel; then
        echo Diesel is already installed.
    else
        cargo install diesel_cli --no-default-features --features sqlite
        echo Installed diesel.
    fi

# Build the debug version
build:
    cargo build

# Build the release version
build_release:
    cargo build --release

# Install the release version
install: build_release
    cargo install --path .

# Create the default configuration file
default_config:
    mkdir -p $(dirname "`{{ cfgfile }}`")
    cargo run config dump > "`{{ cfgfile }}`"

# Connect to the database
db_shell:
    sqlite3 "`{{ dbfile }}`"

# Delete the database
[confirm]
db_nuke:
    rm -f "`{{ dbfile }}`"

# Apply migrations
db_migrate:
    mkdir -p $(dirname "`{{ dbfile }}`")
    diesel --database-url "`{{ dbfile }}`" migration run

# Format migrations with pg_format
db_format:
    pg_format --type-case 2 --function-case 2 --keyword-case 2 -i migrations/*/*.sql

# Export the database to a text file
db_dump:
    sqlite3 "`{{ dbfile }}`" .dump > backup.sql

# Restore the database from backup
db_restore: db_nuke
    cat backup.sql | sqlite3 "`{{ dbfile }}`"

# Create some test data
db_fixture: db_migrate
    {{ bbscmd }} user observe --node-id !cafebead --short-name FRZB --long-name "Frozen BBS"
    {{ bbscmd }} user observe -n !1234fedc -s 1234 -l 'Jerk'
    {{ bbscmd }} user ban -n !1234fedc
    {{ bbscmd }} user observe -n !1234abcd -s 4567 -l 'OK person'
    {{ bbscmd }} board add --name "Board Talk" --description "Discussing this BBS itself."
    {{ bbscmd }} board add --name "Meshtastic" --description "How did we get here?"
    {{ bbscmd }} board add --name "Local" --description "Things happening nearby."
    {{ bbscmd }} post add --board-id 1 --node-id !cafebead --content "First post."
    {{ bbscmd }} post add --board-id 1 --node-id !1234fedc --content "LOL I'm a jerk look at me!"
    {{ bbscmd }} post add --board-id 1 --node-id !1234abcd --content "Third post."
