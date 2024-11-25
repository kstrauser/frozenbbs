bbscmd := "target/debug/frozenbbs"
dbfile := `cut -d= -f2 .env 2>/dev/null || echo`

# Create the .env file pointing to the database file
db_env path_to_dbfile:
    echo 'DATABASE_URL="{{ path_to_dbfile }}"' > .env

# Connect to the database
db_shell:
    sqlite3 "{{ dbfile }}"

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
