# Preserve Jackson @JsonValue annotations and the enums that rely on them
# (Importance, Visibility). Without these rules R8 strips the annotation in
# release builds and Jackson falls back to serializing enums as `name()`
# (e.g. "Default") instead of the integer value the Rust side expects.
-keepattributes RuntimeVisibleAnnotations

-keep enum app.tauri.notification.Importance { *; }
-keep enum app.tauri.notification.Visibility { *; }

# Apps name their PushDataHandler in the manifest and the service loads it
# with Class.forName — keep the interface and every implementation (with
# its no-argument constructor) through R8, or release builds silently lose
# the handler.
-keep interface app.tauri.notification.PushDataHandler
-keep class * implements app.tauri.notification.PushDataHandler { <init>(); }
