{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  inputs.fenix.url = "github:nix-community/fenix";
  inputs.fenix.inputs.nixpkgs.follows = "nixpkgs";

  outputs = { self, nixpkgs, fenix }:
    let
      name = "flatland";
      pkgs = system: import nixpkgs {
        inherit system;
      };
      shell = pkgs: pkgs.mkShell {
        inputsFrom = [ self.packages.${pkgs.system}.default ];
      };
      package = pkgs:
        let
          toolchain = fenix.packages.${pkgs.system}.minimal.toolchain;
        in
          (pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          }).buildRustPackage rec {
            pname = name;
            src = ./.;

            # ---- START package specific settings ----
            version = "0.8.0";
            cargoLock = {
              lockFile = ./Cargo.lock;
              allowBuiltinFetchGit = true;
            };

            STARDUST_RES_PREFIXES = ./res;
            # ---- END package specific settings ----
          };
    in
    {
      overlays.default = final: prev: {
        stardust-xr = (prev.stardust-xr or {}) // {
          ${name} = package final;
        };
      };

      packages."x86_64-linux".default = package (pkgs "x86_64-linux");
      packages."aarch64-linux".default = package (pkgs "aarch64-linux");

      devShells."x86_64-linux".default = shell (pkgs "x86_64-linux");
      devShells."aarch64-linux".default = shell (pkgs "aarch64-linux");
    };
}
