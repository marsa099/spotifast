# One identity recipe for Nix packages and the non-Nix portable updater.
let
  rules = builtins.fromJSON (builtins.readFile ./fork-update/lab-identity.json);
  quote = value: "'" + builtins.replaceStrings [ "'" ] [ "'\\''" ] value + "'";
  command = rule:
    builtins.concatStringsSep " " (
      [ "substituteInPlace" (quote rule.path) ]
      ++ builtins.concatLists (
        builtins.map (pair: [
          "--replace-fail"
          (quote (builtins.elemAt pair 0))
          (quote (builtins.elemAt pair 1))
        ]) rule.replacements
      )
    );
in
builtins.concatStringsSep "\n" (builtins.map command rules) + "\n"
