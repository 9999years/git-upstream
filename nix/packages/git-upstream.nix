{
  system,
  lib,
  stdenv,
  libiconv,
  darwin,
  inputs,
  rustPlatform,
  rust-analyzer,
  cargo-release,
}: let
  inherit (inputs) crane advisory-db;
  craneLib = crane.lib.${system};
  src = lib.cleanSourceWith {
    src = craneLib.path ../../.;
    # Keep test data.
    filter = path: type:
      lib.hasInfix "/data" path
      || (craneLib.filterCargoSources path type);
  };

  commonArgs' = {
    inherit src;

    nativeBuildInputs = lib.optionals stdenv.isDarwin [
      (libiconv.override {
        enableStatic = true;
        enableShared = false;
      })
      darwin.apple_sdk.frameworks.CoreServices
    ];
  };

  # Build *just* the cargo dependencies, so we can reuse
  # all of that work (e.g. via cachix) when running in CI
  cargoArtifacts = craneLib.buildDepsOnly commonArgs';

  commonArgs =
    commonArgs'
    // {
      inherit cargoArtifacts;
    };

  checks = {
    git-upstream-nextest = craneLib.cargoNextest (commonArgs
      // {
        NEXTEST_HIDE_PROGRESS_BAR = "true";
      });
    git-upstream-doctest = craneLib.cargoTest (commonArgs
      // {
        cargoTestArgs = "--doc";
      });
    git-upstream-clippy = craneLib.cargoClippy (commonArgs
      // {
        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      });
    git-upstream-rustdoc = craneLib.cargoDoc (commonArgs
      // {
        cargoDocExtraArgs = "--document-private-items";
        RUSTDOCFLAGS = "-D warnings";
      });
    git-upstream-fmt = craneLib.cargoFmt commonArgs;
    git-upstream-audit = craneLib.cargoAudit (commonArgs
      // {
        inherit advisory-db;
      });
  };

  devShell = craneLib.devShell {
    inherit checks;

    # Make rust-analyzer work
    RUST_SRC_PATH = rustPlatform.rustLibSrc;

    # Extra development tools (cargo and rustc are included by default).
    packages = [
      rust-analyzer
      cargo-release
    ];
  };
in
  # Build the actual crate itself, reusing the dependency
  # artifacts from above.
  craneLib.buildPackage (commonArgs
    // {
      # Don't run tests; we'll do that in a separate derivation.
      doCheck = false;

      passthru = {
        inherit
          checks
          devShell
          commonArgs
          craneLib
          ;
      };
    })
