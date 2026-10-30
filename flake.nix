{
  description = "Chores shared household app";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = {self,nixpkgs}: let
    systems = ["x86_64-linux" "aarch64-linux"];
    each = nixpkgs.lib.genAttrs systems;
    build = system: let
      pkgs = import nixpkgs {inherit system;};
      frontend = pkgs.buildNpmPackage {
        pname = "chores-ui"; version = "0.1.0"; nodejs = pkgs.nodejs_26;
        src = pkgs.lib.cleanSourceWith {src = ./.; filter = path: type: !(builtins.elem (baseNameOf path) ["node_modules" "target" "dist" ".git" "data" "test-results" "result" "backend" "docs" "deploy" ".github"]);};
        npmDepsHash = "sha256-c0WQTjLLnWC5gCOKQm0AMxS+tOgvBFqe6gYRjX6bkBc=";
        installPhase = ''mkdir -p $out/share/chores; cp -r dist/. $out/share/chores/'';
      };
      backend = pkgs.rustPlatform.buildRustPackage {
        pname = "chores-api"; version = "0.1.0";
        src = pkgs.lib.cleanSourceWith {src = ./backend; filter = path: type: baseNameOf path != "target";};
        cargoLock.lockFile = ./backend/Cargo.lock;
      };
    in {inherit pkgs frontend backend;};
  in {
    packages = each (system: let inherit (build system) pkgs frontend backend; in {
      inherit frontend backend; default = backend;
      container = pkgs.dockerTools.buildLayeredImage {
        name="ghcr.io/fruitiex/chores"; tag="main";
        contents=[backend frontend pkgs.cacert pkgs.tzdata pkgs.coreutils pkgs.gnutar];
        extraCommands=''mkdir -p data tmp; chmod 1777 tmp'';
        fakeRootCommands=''chown 10001:10001 data'';
        config={User="10001:10001"; Entrypoint=["${backend}/bin/chores-api"]; Env=["CHORES_BIND=0.0.0.0:8080" "CHORES_DATA_DIR=/data" "CHORES_STATIC_DIR=${frontend}/share/chores"]; ExposedPorts."8080/tcp"={}; Volumes."/data"={};};
      };
    });
    devShells = each (system: let pkgs=(build system).pkgs; in {default=pkgs.mkShell {
      packages=with pkgs; [nodejs_26 python3 cargo rustc rustfmt clippy gcc pkg-config chromium kubectl fluxcd kustomize sops];
      shellHook=''export PLAYWRIGHT_CHROMIUM_EXECUTABLE=${pkgs.chromium}/bin/chromium'';
    };});
  };
}
