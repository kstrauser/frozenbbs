bbscmd := "target/debug/frozenbbs"
dbfile := `cut -d= -f2 .env 2>/dev/null || echo \"\"`

setup:
    #!/bin/bash

    if which -s diesel; then
        echo Diesel is already installed.
    else
        cargo install diesel_cli
        echo Installed diesel.
    fi

    if [ '{{ dbfile }}' != '""' ]; then
        echo .env is already configured.
    else
        if [ {{ os() }} = macos ]; then
            newdbdir="{{ home_dir() }}/Library/Application Support/frozenbbs"
        elif [ {{ os() }} = linux ]; then
            newdbdir="{{ home_dir() }}/.local/share/frozenbbs"
        else
            echo Implement me.
            exit 1
        fi
        mkdir -p "$newdbdir"
        echo 'DATABASE_URL="'$newdbdir/frozen.db'"' > .env
        echo Configured .env.
    fi

# Connect to the database
db_shell:
    sqlite3 {{ dbfile }}

# Delete the database
[confirm]
db_nuke:
    rm -f {{ dbfile }}

# Apply migrations
db_migrate:
    diesel migration run

# Export the database to a text file
db_dump:
    sqlite3 {{ dbfile }} .dump > backup.sql

# Restore the database from backup
db_restore: db_nuke
    cat backup.sql | sqlite3 {{ dbfile }}

db_init:
    {{ bbscmd }} admin user add --id !cafebead --short FRZB --long "Frozen BBS"
    {{ bbscmd }} admin board add --name "Board Talk" --description "Discussing this BBS itself."
    {{ bbscmd }} admin board add --name "Meshtastic" --description "How did we get here?"
    {{ bbscmd }} admin board add --name "Local" --description "Things happening nearby."
    {{ bbscmd }} admin post add --board-id 1 --node-id !cafebead --content "First post."
    {{ bbscmd }} admin user add -n !1234abcd -s 1234 -l 'OK person'
    {{ bbscmd }} admin user add -n !1234fedc -s 4567 -l 'Jerk' -j
