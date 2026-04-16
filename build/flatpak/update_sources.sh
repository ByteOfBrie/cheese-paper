#!/bin/bash
# script to automate updating cargo sources. this should be done whenever we update the cargo packages

script_dir=$(cd "$(dirname "$0")" && pwd)

clone_path="${script_dir}/flatpak-builder-tools"

rm -rf "${clone_path}"

git clone https://github.com/flatpak/flatpak-builder-tools.git "$clone_path"

venv_path="${clone_path}/cargo/venv"

python -m venv --clear "$venv_path"

cargo_toml_file=$(realpath "${script_dir}/../../Cargo.lock")

echo $cargo_toml_file

(. "${venv_path}/bin/activate" && python -m pip install "${clone_path}/cargo/" && python "${clone_path}/cargo/flatpak-cargo-generator.py" "$cargo_toml_file" -o "$script_dir/cargo-sources.json" )
