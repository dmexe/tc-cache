#!/bin/bash

set -ex

apt-get update -qy                             
apt-get install musl-tools upx -qq             
rustup target add x86_64-unknown-linux-musl    
