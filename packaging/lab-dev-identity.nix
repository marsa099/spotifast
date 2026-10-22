# Isolate Spotifast Lab from upstream. The legacy keyring service is intentional:
# an existing user's recorded credential-profile keeps protected sign-in intact.
''
  substituteInPlace src/paths.rs \
    --replace-fail '"spotifast"' '"spotifast-lab"' \
    --replace-fail '"spotifast-config"' '"spotifast-lab-config"' \
    --replace-fail '"spotifast-state"' '"spotifast-lab-state"' \
    --replace-fail '"spotifast-cache"' '"spotifast-lab-cache"'
  substituteInPlace src/single_instance.rs \
    --replace-fail 'const NAME: &str = "spotifast";' 'const NAME: &str = "spotifast-lab";'
  substituteInPlace src/credentials.rs \
    --replace-fail '"rocks.spotifast.Spotifast"' '"rocks.fastpotify.Fastpotify"'
  substituteInPlace src/app.rs \
    --replace-fail 'fastframe_now_playing::App::new("spotifast", "Spotifast");' 'fastframe_now_playing::App::new("spotifast_lab", "Spotifast Lab"); app.desktop_entry = "spotifast-lab".into();' \
    --replace-fail 'id: "spotifast"' 'id: "spotifast-lab"' \
    --replace-fail '"org.mpris.MediaPlayer2.spotifast"' '"org.mpris.MediaPlayer2.spotifast_lab"' \
    --replace-fail '"Spotifast"' '"Spotifast Lab"' \
    --replace-fail '{} - Spotifast' '{} - Spotifast Lab'
  substituteInPlace src/entrypoint.rs \
    --replace-fail '"spotifast"' '"spotifast-lab"' \
    --replace-fail '"Spotifast"' '"Spotifast Lab"'
  substituteInPlace src/sink.rs \
    --replace-fail '"Spotifast"' '"Spotifast Lab"' \
    --replace-fail '"spotifast"' '"spotifast-lab"'
  substituteInPlace src/backend.rs \
    --replace-fail 'Autostart::new("spotifast", "Spotifast")' 'Autostart::new("spotifast-lab", "Spotifast Lab")'
  substituteInPlace src/settings.rs \
    --replace-fail 'device_name: "Spotifast".to_string()' 'device_name: "Spotifast Lab".to_string()'
  substituteInPlace tests/branding.rs \
    --replace-fail 'Settings::default().device_name, "Spotifast"' 'Settings::default().device_name, "Spotifast Lab"'
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
