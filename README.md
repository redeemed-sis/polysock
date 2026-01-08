# General information
`polysock` is a command-line utility for socket management. It serves as an alternative to `socat`, written on rust. This time `polysock` supports ***udp***, ***stdio***, ***tcp-client***, ***tcp-server*** types of sockets. This utility is supported on Linux (Arch, Ubuntu), Windows and macOs.
# Building and installation
It is possible to build it for `ubuntu` and `arch-linux`:
```sh
# Ubuntu in docker
docker build -t rust-ubuntu .
docker run -it -v $(pwd):/home/ubuntu/build rust-ubuntu
cargo deb
sudo dpkg -i target/debian/*.deb

# On arch linux
makepkg
sudo pacman -U *.tar.zst
```
# Some examples
## Different socket types
Here are a few common examples of how to use `polysock`:
- UDP examples
```sh
# Bind STDIO to UDP socket
polysock oneliner -f stdio -t udp --to-params '{ "port_dst": "5150", "ip_dst": "127.0.0.1" }'

# Bind two UDP sockets in bidirectional mode
polysock oneliner -e bidir \ 
    -f udp --from-params  '{ "port_local": "5150" }' \
    -t udp --to-params '{ "port_dst": "5151", "ip_dst":"127.0.0.1" }'
```
- TCP examples
```sh
# Bind STDIO to TCP connection
polysock oneliner -f stdio \
    -t tcp-client --to-params '{ "port_dst": "5150", "ip_dst": "127.0.0.1" }'

# Bind UDP socket to all connection on TCP server. Messages 
# received on udp will be redirected to TCP clients
polysock oneliner -f udp --from-params '{ "port_local": "5150" }' \
    -t tcp-server --to-params '{ "port_local": "1234" }'
```
## Tracing decorators
```sh
# Trace every message on "from" and "to" sockets by
# printing socket information
polysock oneliner -f tcp-server --from-params '{ "port_local": "5150" }' \
    -t stdio --trace-info

# Output:
#
# Socket is opened: tcp-server0
# Socket is opened: stdio0
# Data is received from: tcp-server0, connected clients:
# Client 127.0.0.1:42700
# Hello
# Data is transered to: stdio0
# Data is received from: tcp-server0, connected clients:
# Client 127.0.0.1:42700
# World
# Data is transered to: stdio0

# Trace every message on "from" socket by
# printing socket information and data in
# raw and canonical format
polysock oneliner -f tcp-server --from-params '{ "port_local": "5150" }' \
    -t stdio --trace-info --trace-raw --trace-canon --trace-to-off

# Output:
#
# Socket is opened: tcp-server0
# Data is received from: tcp-server0, connected clients:
# Client 127.0.0.1:44052
# Data is received: [72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 10]
# Received data (canonical format):
#  Length: 12 (0xc) bytes
# 0000:   48 65 6c 6c  6f 20 77 6f  72 6c 64 0a                Hello world.
# Hello world
```
