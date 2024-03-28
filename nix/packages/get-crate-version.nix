{
  lib,
  writeShellApplication,
  git-upstream,
}:
writeShellApplication {
  name = "get-crate-version";

  text = ''
    echo ${lib.escapeShellArg git-upstream.version}
  '';
}
