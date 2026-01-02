# Start from the official latest Ubuntu image
FROM ubuntu:latest

# Update the package index and install the desired package(s)
RUN apt-get update && apt-get install -y rustup \
    dpkg-dev

ARG USER_NAME="ubuntu"

USER ${USER_NAME}

RUN rustup default stable
RUN cargo install cargo-deb
RUN mkdir /home/${USER_NAME}/build
WORKDIR /home/${USER_NAME}/build

# Optional: Set the default command for when the container starts
CMD ["/bin/bash"]
