{ lib, pkgs, ... }:

let
  rOracleVersion = "4.6.1";
in
{
  packages = with pkgs; [
    cargo-deny
    pkg-config
    git
    gzip
    xz
    bzip2
    zstd
  ];

  languages = {
    rust = {
      enable = true;
      toolchainFile = ./rust-toolchain.toml;
    };

    c.enable = true;
    python.enable = true;

    r = {
      enable = true;
      lsp.enable = false;
      # Keep input updates aligned with the fixture generators and oracle CI.
      package =
        assert lib.assertMsg (pkgs.R.version == rOracleVersion)
          "The fixture oracle requires R ${rOracleVersion}, but devenv's locked nixpkgs provides R ${pkgs.R.version}. Select a matching nixpkgs revision in devenv.yaml.";
        pkgs.rWrapper.override {
          packages = with pkgs.rPackages; [
            RcppTOML
          ];
        };
    };
  };
  git-hooks = {
    hooks = {
      clippy = {
        enable = true;

        settings = {
          allFeatures = true;
        };
      };

      rustfmt = {
        enable = true;
      };
    };
  };
}
