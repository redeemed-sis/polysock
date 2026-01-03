# General information
`polysock` is a command-line utility for socket management. It serves as an alternative to `socat` while offering more flexible features, such as bidirectional exchange, a REPL mode, and script execution.
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
Here are a few common examples of how to use `polysock`:
```sh
# Bind STDIO and UDP socket
polysock oneliner -f stdio -t udp --to-params '{ "port_dst": "5150", "ip_dst": "127.0.0.1" }'
# Bind two UDP sockets in bidirectional mode
polysock oneliner -e bidir \ 
    -f udp --from-params  '{ "port_local": "5150" }' \
    -t udp --to-params '{ "port_dst": "5151", "ip_dst":"127.0.0.1" }'
```
