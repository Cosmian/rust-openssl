#!/bin/bash

if [ "$#" -ne 1 ]; then
    echo -e "Usage: ./setup.sh <OPENSSL_DIR>"
    exit 1
fi


echo "Setup for OpenSSL version 3.1.0 with FIPS 140 module"
echo "Installing OpenSSL to "${1}" ..."

if [[ ! -d "${1}" ]]; then
    echo "ERROR: Specified directory does not exist"
    exit 2
fi

cd "${1}"
export OPENSSL_DIR="$(pwd)"
cd -

# Creating ssl config files directory.
mkdir "${OPENSSL_DIR}/ssl"
export OPENSSL_CONF="${OPENSSL_DIR}"/ssl/openssl.cnf

path_save="$(pwd)"

# Downloading and installing OpenSSL 3.1.0.
cd "$(mktemp -d)"
wget https://github.com/openssl/openssl/releases/download/openssl-3.1.0/openssl-3.1.0.tar.gz
tar -xvf openssl-3.1.0.tar.gz

cd openssl-3.1.0/
./Configure --prefix="${OPENSSL_DIR}" --openssldir="${OPENSSL_DIR}/ssl" enable-fips no-shared \
&& make -j "$(nproc)" && make install -j "$(nproc)"

echo -e "\nOpenSSL successfully installed at "${OPENSSL_DIR}""

cd "${path_save}"
