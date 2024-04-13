{ pkgs, ... }:
pkgs.stdenvNoCC.mkDerivation {
  name = "resources";
  src = ./.;
  
  buildPhase = "cp -r $src/res $out";
}