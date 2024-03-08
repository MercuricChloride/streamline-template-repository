with import <nixpkgs> {};

stdenv.mkDerivation {
    name = "streamline-build-environment";
    buildInputs = [ protobuf rustup ];
}
