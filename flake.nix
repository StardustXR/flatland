{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, crane }:
  let supportedSystems = [ "aarch64-linux" "x86_64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      nixpkgsFor = forAllSystems (system: import nixpkgs { inherit system; });
  in {
    packages = forAllSystems (system: let pkgs = nixpkgsFor.${system}; craneLib = crane.mkLib pkgs; in {
      default = craneLib.buildPackage {
        doCheck = false;
        src = ./.;
        
        STARDUST_RES_PREFIXES = pkgs.stdenvNoCC.mkDerivation {
          name = "resources";
          src = ./.;
  
          buildPhase = "cp -r $src/res $out";
        };
      };
    });

    devShells = forAllSystems (system: let pkgs = nixpkgsFor.${system}; craneLib = crane.mkLib pkgs; in {
      default = craneLib.devShell {
      };
    });
  };
}
