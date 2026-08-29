{
  description = "A classic Windows-7-style start menu for the COSMIC panel";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    # libcosmic's `master` branch currently declares a newer MSRV than
    # nixpkgs' own rustc satisfies (verified while putting this flake
    # together: `cargo +stable check` refused with "requires rustc 1.93").
    # fenix gives us a rolling nightly toolchain instead. Swap this for
    # `pkgs.rustPlatform` once libcosmic settles on a stable-compatible MSRV.
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self, nixpkgs, flake-utils, fenix }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        toolchain = fenix.packages.${system}.latest.toolchain;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };

        appId = "io.github.MyNameIs-13.CosmicAppletStartMenu";

        runtimeLibs = with pkgs; [
          wayland
          wayland-protocols
          libxkbcommon
          libGL
          vulkan-loader
          fontconfig
          freetype
          expat
          libx11
          libxcursor
          libxrandr
          libxi
        ];
      in
      {
        packages.default = rustPlatform.buildRustPackage {
          pname = "cosmic-applet-startmenu";
          version = "0.1.0";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;

            # One entry per distinct git dependency source (repo + rev),
            # keyed by "<name>-<version>" of any one crate it produces —
            # not one per crate. `nix build` fails on the first missing/
            # wrong hash with the correct one in the error message; paste
            # it in and re-run until it builds. Standard nixpkgs workflow
            # for Cargo git dependencies, see the nixpkgs manual's "Rust"
            # chapter.
            outputHashes = {
              "accesskit-0.22.0" = "sha256-pP9CyiV1zIONQ7vbl5MkMtilemSPrHaZ0c/SyR+lb0k=";
              "atomicwrites-0.4.2" = "sha256-QZSuGPrJXh+svMeFWqAXoqZQxLq/WfIiamqvjJNVhxA=";
              "libcosmic-1.0.0" = "sha256-43r6smzg4zIZn/HarkZkchgnXGNnrIvOaf49I9CauOA=";
              "window_clipboard-0.4.1" = "sha256-WO3JFbE+6ESRAfkxrnEFeZyGuhUHLOKOVHcGQyHwoK0=";
              "cosmic-protocols-0.2.0" = "sha256-LUAmB+3+doRZOJbVURaIInaQuV/LXCKfoWHA28ihAMo=";
              "cosmic-freedesktop-icons-0.4.0" = "sha256-tPriTi5L0mFMHjo5xpF5cmKGHqlX3WUO7EZAgVdBpS4=";
              "cosmic-panel-config-0.1.0" = "sha256-dx+k+A5ZXo9MXuUxjdEd4xEqscuaNdVoojQzCWUNy/g=";
              "cosmic-settings-daemon-0.1.0" = "sha256-LYIR+qK+hCBVV+bfVWz2jvH5fGvfNTcryKqfe5n8Gog=";
              "cryoglyph-0.1.0" = "sha256-10JUHl1ktbqLaReuiU3HPa4r2KvsoryyJoF3BFoge3U=";
              "winit-0.31.0-beta.2" = "sha256-8r9O5RgVa8vxkPPYvr2aQiRdZ4isg7Jdnk8O5gQIr9k=";
              "smithay-clipboard-0.8.0" = "sha256-GojAFRbhJcP0Rpr+v9WOivgW9x38PZdeBWTbMhkDB3A=";
              "softbuffer-0.4.1" = "sha256-9Ret/nfieBFl4yJ9TddyWsSuS7sI4QAza/TZrxYMb+I=";
            };
          };

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = runtimeLibs;

          # `cargo-auditable`/build.rs already writes desktop/metainfo files
          # to target/xdgen/ during the build; install them alongside the
          # binary the same way `justfile`'s `install` recipe does.
          postInstall = ''
            install -Dm0644 target/xdgen/app.desktop \
              $out/share/applications/${appId}.desktop
            install -Dm0644 target/xdgen/app.metainfo.xml \
              $out/share/metainfo/${appId}.metainfo.xml
            install -Dm0644 resources/icons/hicolor/scalable/apps/${appId}.svg \
              $out/share/icons/hicolor/scalable/apps/${appId}.svg
          '';

          meta = {
            description = "A classic Windows-7-style start menu for the COSMIC panel";
            license = pkgs.lib.licenses.gpl3Only;
            platforms = pkgs.lib.platforms.linux;
            mainProgram = "cosmic-applet-startmenu";
          };
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            toolchain
            pkgs.pkg-config
          ];
          buildInputs = runtimeLibs;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
        };
      }
    );
}
