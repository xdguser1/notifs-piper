{
  description = "A notification daemon for multiprocess piping.";

  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = inputs: {
    apps = builtins.mapAttrs (system: pkgs: {
      notifs-piper = {
        type = "app";
        program = "${inputs.self.packages.${system}.notifs-piper}/bin/notifs-piper";
      };

      default = inputs.self.apps.${system}.notifs-piper;
    }) inputs.nixpkgs.legacyPackages;

    packages = builtins.mapAttrs (system: pkgs:
    let
      inherit (pkgs) makeRustPlatform lib fetchFromGitHub;
    in
    {
      notifs-piper = (makeRustPlatform rec {
        inherit (inputs.fenix.packages.${system}.minimal) toolchain;
        cargo = toolchain;
        rustc = toolchain;
      }).buildRustPackage (finalAttrs: {
        pname = "notifs-piper";
        version = "0.1.2";

        src = fetchFromGitHub {
          owner = "xdguser1";
          repo = "notifs-piper";
          rev = "v0.1.2";
          hash = "sha256-UEKItmzdCk1uc0MmEyMF5YAdvqu5srtBISs8tEchdG0=";
        };

        cargoHash = "";

        cargoLock.lockFile = ./Cargo.lock;
      });

      default = inputs.self.packages.${system}.notifs-piper;
    }) inputs.nixpkgs.legacyPackages;

    nixosModules.default = { config, lib, pkgs, ... }:
    let
      inherit (lib) types;
      cfg = config.services.notifs-piper;
    in
    {
      options.services.notifs-piper = {
        enable = lib.mkEnableOption "notifs-piper";

        package = lib.mkPackageOption inputs.self.packages.${pkgs.system} "notifs-piper" {};

        file = lib.mkOption {
          type = types.nullOr types.path;
          example = "/home/admin/logs/my-logs.json";
          description = "The path where the notifications are stored.";
          default = null;
        };

        max = lib.mkOption {
          type = types.nullOr types.ints.u16;
          example = "100";
          description = "The maximum amount of logs to be stored.";
          default = null;
        };

        auto-close = lib.mkOption {
          type = types.bool;
          example = "true";
          description = "Whether notifications are closed automatically.";
          default = false;
        };

        timeout = lib.mkOption {
          type = types.nullOr types.ints.u16;
          example = "5000";
          description = "This corresponds to the number of milliseconds before automatically closing a notification when given a timeout of -1.";
          default = null;
        };

        options = lib.mkOption {
            type = types.listOf (types.oneOf [
                "action-icons"
                "actions"
                "body"
                "body-hyperlinks"
                "body-images"
                "body-markup"
                "persistence"
                "sound"
            ]);
            example = "[\"body-images\" \"body\"]";
            description = "Basic notification daemon server capabilities.";
            default = [];
        };

        icon = lib.mkOption {
            type = types.uniq (types.nullOr (types.either "icon-multi" "icon-static"));
            example = "icon-static";
            description = ''
            Chooses the type of icons to be shown.
            This is another option, but icon-multi and icon-static are mutually exclusive.
            '';
            default = null;
        };
      };

      config = lib.mkIf cfg.enable {
        systemd.user.services.notifs-piper = {
          wantedBy = [ "default.target" ];
          description = "Starts notifs-piper notification server.";
          serviceConfig = {
            Type = "simple";
            ExecStart = let
              makeOption = opt: optn: if opt != null then "-${optn} ${builtins.toString opt}" else "";
            in
              ''
                ${cfg.package}/bin/notifs-piper daemon ${
                  makeOption cfg.file "f"
                } ${
                  makeOption cfg.max "m"
                } ${
                  if cfg.auto-close then "-c" else ""
                } ${
                  makeOption cfg.timeout "t"
                } ${
                  builtins.foldl' (acc: el: "${acc} ${el}") "" (builtins.map (x: "-o ${x}") cfg.options)
                } ${
                  makeOption cfg.icon "o"
                }
              '';
            StandardOutput = "journal";
            StandardError = "journal";
          };
        };
      };
    };
  };
}
