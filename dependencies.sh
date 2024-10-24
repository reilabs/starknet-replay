#!/bin/bash

# LLVM installation script adapted from `starkware-libs/sequencer`.

set -e

[[ ${UID} == "0" ]] || SUDO="sudo"

function install_essential_deps_linux() {
    $SUDO bash -c '
        apt update && apt install -y \
            ca-certificates \
            curl \
            git \
            gnupg \
            jq \
            libssl-dev \
            lsb-release \
            pkg-config \
            ripgrep \
            software-properties-common \
            zstd \
            wget
  '
}

function setup_llvm_deps() {
    case "$(uname)" in
    Darwin)
        brew update
        brew install llvm@18

        llvm_prefix=$(brew --prefix llvm@18)
        echo TABLEGEN_180_PREFIX=$llvm_prefix >> $GITHUB_ENV
        echo PATH=$llvm_prefix/bin:$PATH >> $GITHUB_ENV
        echo LIBRARY_PATH=$(brew --prefix)/lib:$LIBRARY_PATH >> $GITHUB_ENV
        echo LD_LIBRARY_PATH=$(brew --prefix)/lib:$LD_LIBRARY_PATH >> $GITHUB_ENV
        echo MLIR_SYS_180_PREFIX=$llvm_prefix >> $GITHUB_ENV
        ;;
    Linux)
        $SUDO bash -c 'curl https://apt.llvm.org/llvm.sh -Lo llvm.sh
        bash ./llvm.sh 18 all
        apt update && apt install -y \
            libgmp3-dev \
            libmlir-18-dev \
            libpolly-18-dev \
            libzstd-dev \
            mlir-18-tools
        '
        ;;
    *)
        echo "Error: Unsupported operating system"
        exit 1
        ;;
    esac
}

function main() {
    [ "$(uname)" = "Linux" ] && install_essential_deps_linux
    setup_llvm_deps
    echo "LLVM dependencies installed successfully."
}

main "$@"
