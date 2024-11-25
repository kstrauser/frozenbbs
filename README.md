# Frozen BBS, a message board for Meshtastic

_NOTE: This doesn't do anything yet. I'm not being modest. It doesn't even have code to connect to Meshtastic yet. It will!_

Frozen intends to be a radio BBS optimized for slow connections. This is the very beginning of the project.

The current status is that Frozen has a SQLite backend with tables for users, message boards, and posts in those boards. It also has an admin tool to read those objects and create new ones. If you're super bored, you can use it to create users and pretend you're having a conversation with yourself. I have not done this. _Ahem._

It has a [Justfile](https://just.systems) to execute some common tasks like running DB migrations, backing up and restoring, and creating sample objects to poke at. Even if you don't use `just` -- and you should! you should! -- it's easy to read to see how those routine operations look.

## Building

Clone this repo. Run `cargo build`. Ta-da: now it's built.

## Getting started

Install `diesel_cli` and create a `.env` file telling Frozen BBS where to put its database file. You can do these with:

```shell
$ just setup
Installed diesel.
Configured .env.
```

Now, run the database migrations to create the tables:

```shell
$ just db_migrate
```

You can make some demo objects:

```shell
$ just db_init
```

## The admin tool

The BBS program is called `frozenbbs`. Right now it has one command, `admin`, with several subcommands:

**Listing users**

```shell
$ frozenbbs admin user list
# BBS users

| Created at          | Last seen at        | Node ID   | Name | Long name                                |
| ------------------- | ------------------- | --------- | ---- | ---------------------------------------- |
| 2024-11-25@04:34:47 | 2024-11-25@04:34:47 | !cafebead | FRZB | Frozen BBS                               |
```

**Adding a user**

```shell
$ frozenbbs admin user add --id !1234abcd --short abcd --long "ABC'ing you later!"
Created user #2, '!1234abcd'
```

**Listing message boards**

```shell
$ frozenbbs admin board list
# BBS boards

| Created at          | Num | Name                           | Description |
| ------------------- | --- | ------------------------------ | ----------- |
| 2024-11-25@04:34:47 |   1 | Board Talk                     | Discussing this BBS itself. |
| 2024-11-25@04:34:47 |   2 | Meshtastic                     | How did we get here? |
| 2024-11-25@04:34:47 |   3 | Local                          | Things happening nearby. |
```

**Adding a message board**

```shell
$ frozenbbs admin board add --name 'GitHub Stuff' --description 'Grousing about corporate monoculture'
Created board #4, 'GitHub Stuff'
```

**Reading posts**

```shell
$ target/debug/frozenbbs admin post read -b1
# Posts in 'Board Talk'

| Created at          | Node ID   | Body |
| ------------------- | --------- | ---- |
| 2024-11-25@04:34:47 | !cafebead | First post. |
```

**Writing posts**

```shell
$ frozenbbs admin post add -b 1 -n !1234abcd -c 'Hello, GitHub readers!'
Created post #2
```

## Notes

See, there's really not much there. The bones are in place, though. The next step will be writing a `frozenbbs client` command to let you interact with the BBS locally like a remote user would. And following that will be the real fun: a `frozenbbs server` command processing network traffic. That's probably a lot closer to fruition than the current state of things might appear. After that will be a menu system and other non-message-board features.

## License

Copyright (C) 2024 Kirk Strauser

This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with this program. If not, see <http://www.gnu.org/licenses/>.
