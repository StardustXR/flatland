{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
    }:
    let
      supportedSystems = [
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      nixpkgsFor = forAllSystems (system: import nixpkgs { inherit system; });
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgsFor.${system};
          craneLib = crane.mkLib pkgs;
        in
        {
          default = craneLib.buildPackage {
            src = ./.;
            cargoLock = ./Cargo.lock;
            doCheck = false;

            nativeBuildInputs = [ pkgs.just ];
            postInstall = "just prefix=$out install-data";
            STARDUST_RES_PREFIXES = "${builtins.placeholder "out"}/share";
          };
        }
      );
      formatter = forAllSystems (system: nixpkgsFor.${system}.nixfmt-tree);

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgsFor.${system};
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              cargo
              rustc
              just
            ];
          };
        }
      );
    };
}
