# Frozen BBS, a message board for Meshtastic

Frozen intends to be a radio BBS optimized for slow connections. This is the very beginning of the project.

The current status is that Frozen has a working message board, admin tools to manage data, a terminal client to interact with the BBS, and a server that supports serial and TCP connections to Meshtastic radios.

It has a [Justfile](https://just.systems) to execute some common tasks like running DB migrations, backing up and restoring, and creating sample objects to poke at. Even if you don't use `just` -- and you should! you should! -- it's easy to read to see how those routine operations look.

## Building

Clone this repo. Run `cargo build`. Ta-da: now it's built.

# Getting started

Install `diesel_cli`.

```shell
$ just setup
Installed diesel.
```

Now, run the database migrations to create the tables:

```shell
$ just db_migrate
```

You can optionally make some demo objects:

```shell
$ just db_init
```

# Configuring

`just default_config` will create a sample config file:

```shell
$ just default_config
mkdir -p $(dirname "`cargo run config config-path`")
cargo run config dump > "`cargo run config config-path`"
```

Run `cargo run config config-path` to get the path to the config file. Edit it to at least set `my_id` to the node ID of your radio. `frozenbbs` will pester you until you do this.

```toml
my_id = "!cafeb33d"
db_path = "/Users/myname/.local/share/frozenbbs/frozen.db"
serial_device = "/dev/ttyUSB0" # *or* tcp_address = "myradio.example.com:4403"
sysops = ["!cafeb33d"]
public_channel = 0
ad_text = "I'm running a BBS on this node. DM me to get started!"
```

- `my_id` should be the hex name of the Meshtastic node you'll be running the BBS on. It will only process messages which are addressed to that ID.
- `db_path` is probably usable as-is.
- `serial_device` may also be OK, although I had to use `/dev/ttyACM0` on my Raspberry Pi.
- `tcp_address` is the radio's hostname:port to connect to.
  - If you configure both of these, the server will use `tcp_address`.
- `sysops` lists usrs who can execute administration commands.
- `public_channel` sets the channel number to send BBS ads to.
- `ad_text` is the message that the sysop_advertise command sends to the public channel.

# Running the program

The BBS program is called `frozenbbs` with several sub-commands. For example:

**Listing users**

```shell
$ frozenbbs user list
# BBS users

| Created at          | Last seen at        | Node ID   | Name | Long name                                |
| ------------------- | ------------------- | --------- | ---- | ---------------------------------------- |
| 2024-11-25@04:34:47 | 2024-11-25@04:34:47 | !cafebead | FRZB | Frozen BBS                               |
```

**Adding a user**

```shell
$ frozenbbs user observe --id !1234abcd
Created user #2, '!1234abcd'
```

**Listing message boards**

```shell
$ frozenbbs board list
# BBS boards

| Created at          | Num | Name                           | Description |
| ------------------- | --- | ------------------------------ | ----------- |
| 2024-11-25@04:34:47 |   1 | Board Talk                     | Discussing this BBS itself. |
| 2024-11-25@04:34:47 |   2 | Meshtastic                     | How did we get here? |
| 2024-11-25@04:34:47 |   3 | Local                          | Things happening nearby. |
```

**Adding a message board**

```shell
$ frozenbbs board add --name 'GitHub Stuff' --description 'Grousing about corporate monoculture'
Created board #4, 'GitHub Stuff'
```

**Reading posts**

```shell
$ frozenbbs post read -b1
# Posts in 'Board Talk'

| Created at          | Node ID   | Body |
| ------------------- | --------- | ---- |
| 2024-11-25@04:34:47 | !cafebead | First post. |
```

**Writing posts**

```shell
$ frozenbbs post add -b 1 -n !1234abcd -c 'Hello, GitHub readers!'
Created post #2
```

**Running a local session**

```shell
$ frozenbbs client terminal -n !abcd1234
Connected. ^D to quit.

Command: ?

Welcome to Frozen BBS!

Commands:

B  : Board list
Bn : Enter board #n
?  : Tell me where I am
H  : This help
You are not in a board.

Command: b

Boards:

#1 Board Talk: Discussing this BBS itself.
#2 Meshtastic: How did we get here?
#3 Local: Things happening nearby.

Command: b1

Entering board 1

Command: n

From: !cafebead/FRZB:Frozen BBS
At  : 2024-11-28T17:43:13
Msg : First post.

Command: ^D
Disconnected.
```

**Running a server**

```shell
$ frozenbbs server
Observed node at 1732851007: !f5f5a1c1/HPT1:Depeche Node
[...]
```

**Other commands**

There are several commands and subcommands, each with a handful of arguments. I tried to make the UI reasonably discoverable. If you run `frozenbbs`, it'll show you the available commands. `frozenbbs foo` will show the subcommands under `foo`. `frozenbbs foo bar` will show any required arguments for `bar`. Use `-h` and `--help` to explore!

# Design notes

The `frozenbbs` program doesn't cache any state in RAM. When the event loop processes a command, it calls the related command function with information about the user running it and any arguments they sent. That command function is responsible for gathering any additional information needed to fulfill the request. For example, the command to read the next post in the user's current message board reads the necessary information from the database. This has a few nice effects:

1. It greatly simplifies program code because we don't have to pass massive balls of state around.
2. There's no cache to get out of date.
3. The program can be killed and restarted at any time without losing state stored in RAM.
4. Conceptually, we could run multiple copies on different nodes at the same time, served from the same database.

The downside is that we're fetching more database information than strictly necessary instead of remembering values. Honestly, SQLite is so astonishingly fast and the radio broadcasts so inherently slow that the tradeoff is well worth it. For example, running `frozenbbs -vv client command --node-id !abcd1234 n` takes about 2.5ms in a loop. You will not be processing more than 400 requests per second over Meshtastic, guaranteed.

# Hardware setup

I have a RAK4631 radio with a RAK13800 Ethernet module, with the BBS code running on a Raspberry Pi 4 next to it. I went through a lot of false starts making this work.

First try: The server connects to the radio via Ethernet. I was _so_ close to having this work, but it turns out that the Meshtastic Rust library only supports sending via serial, not Ethernet. It doesn't support Bluetooth at all. At one point I had a horrid scheme set up where I'd receive a packet via Ethernet, process it, then shell out to the Python Meshtastic command to send the response via serial (because you couldn't connect both programs to the radio's Ethernet-based service at once). That actually _worked_. Once. Then it hung the Ethernet jack so that you'd never get other packets without power-cycling the radio. Nope. I'm only willing to entertain so much hackery at once.

Second try: Use MQTT. I was _so_ close to having this work, but it turns out that I couldn't get the radio to actual transmit the Protobufs-wrapped reply message I was sending it. The RAK4631 firmware doesn't support the JSON API. Really, though, even if that did work, it's not obvious to me that this was a good long-term solution. The Mesthtastic firmware is adding privacy features that allow nodes to set a bit saying "please don't forward my packets to MQTT", which is great in general, but super inconvenient here. There's some online talk about making exceptions for where the MQTT server is running on a private network, so we could run something like mosquito on the Raspberry Pi and configure the radio to connect to it. But then it'd still be up to the whims of the firmware whether to support that exception, and it also means that you can't configure the radio to connect to _another_ MQTT server for other needs. That would be a bummer.

Third try: OK, fine. We'll make serial work. More people have serial connections than Ethernet anyway, so that's likely a good idea anyway. But here's a fun fact you might not know or appreciate -- I certainly didnt!: the Raspberry Pi's USB port doesn't reliably put out enough power to support a radio, its transmitter, its Ethernet module, and lots of serial output all at the same time. Oh look, corrupted packets! Transmitter hangs! Ethernet freezing! Fun for the whole family!

Today: Using the serial port, but with a powered USB hub between the Raspberry Pi and the RAK4631 so that the hub is providing the necessary wattage. Finally, after all this time, I can fully operate all the features of the radio as documented _and_ running Frozen BBS without the whole thing locking up ever half an hour. Success! I have this setup running live today.

# Releases

- **v1.3.2, 2024-12-28:** _Important fix:_ Fixed a loop where the BBS tried to message itself.
- **v1.3.1, 2024-12-28:** Fixed a bug where configuring `tcp_address` would conflict with the new default `serial_device`. 🤦‍♂️
- **v1.3.0, 2024-12-28:** Handle multiple replies from the same command. Direct messages. Robustness cleanups. Observe more packet types. User bios. Default configuration.
- **v1.2.0, 2024-12-22:** Supports TCP, too!
- **v1.1.0, 2024-12-20:** Lots of cleanups and quality-of-life enhancements. A menu system. Spam!
- **v1.0.0, 2024-12-18:** First official release.

# License

Copyright (C) 2024 Kirk Strauser

This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with this program. If not, see <http://www.gnu.org/licenses/>.
