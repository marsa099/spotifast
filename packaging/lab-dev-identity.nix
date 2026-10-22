# Current Spotifast sources, with the existing local Lab launch/profile identity.
# The pinned legacy checkpoint in lab.nix deliberately keeps its old transform.
''
  substituteInPlace src/paths.rs \
    --replace-fail '"spotifast"' '"fastpotify-lab"' \
    --replace-fail '"spotifast-config"' '"fastpotify-lab-config"' \
    --replace-fail '"spotifast-state"' '"fastpotify-lab-state"' \
    --replace-fail '"spotifast-cache"' '"fastpotify-lab-cache"'
  substituteInPlace src/single_instance.rs \
    --replace-fail 'const NAME: &str = "spotifast";' 'const NAME: &str = "fastpotify-lab";'
  substituteInPlace src/app.rs \
    --replace-fail 'fastframe_now_playing::App::new("spotifast", "Spotifast");' 'fastframe_now_playing::App::new("fastpotify_lab", "Fastpotify Lab"); app.desktop_entry = "fastpotify-lab".into();' \
    --replace-fail 'id: "spotifast"' 'id: "fastpotify-lab"' \
    --replace-fail '"Spotifast"' '"Fastpotify Lab"' \
    --replace-fail '{} - Spotifast' '{} - Fastpotify Lab'
  substituteInPlace src/entrypoint.rs \
    --replace-fail 'desktop_entry("spotifast")' 'desktop_entry("fastpotify-lab")' \
    --replace-fail '"Spotifast"' '"Fastpotify Lab"'
  substituteInPlace src/backend.rs \
    --replace-fail 'Autostart::new("spotifast", "Spotifast")' 'Autostart::new("fastpotify-lab", "Fastpotify Lab")'
  substituteInPlace src/settings.rs \
    --replace-fail 'device_name: "Spotifast".to_string()' 'device_name: "Fastpotify Lab".to_string()'
  substituteInPlace packaging/icons/spotifast.svg \
    --replace-fail '#18c055' '#8b5cf6' \
    --replace-fail '#0c8a3a' '#6d28d9' \
    --replace-fail '#5cf095' '#c4b5fd' \
    --replace-fail '#1ed760' '#a78bfa' \
    --replace-fail '#15b650' '#8b5cf6' \
    --replace-fail '#d9ffe8' '#ede9fe' \
    --replace-fail '#7bf0aa' '#c4b5fd' \
    --replace-fail '#0a6e2e' '#5b21b6'
''
