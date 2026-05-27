{
  description = "Fuzzy-search Claude Code conversation history from the terminal.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "ccmanager";
            version = "1.1.2";

            src = ./.;

            # On first Nix build Nix will print the correct sha256 — drop
            # it in here, replacing fakeHash.
            cargoHash = pkgs.lib.fakeHash;

            # Some tests require filesystem access not available in Nix sandbox
            doCheck = false;

            meta = with pkgs.lib; {
              description = "Fuzzy-search Claude Code conversation history from the terminal.";
              homepage = "https://github.com/NormalUhr/ccmanager";
              license = licenses.mit;
              mainProgram = "ccmanager";
            };
          };
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/ccmanager";
        };
      });

      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              cargo
              rustc
              rust-analyzer
              rustfmt
              clippy
            ];

            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          };
        }
      );
    };
}
